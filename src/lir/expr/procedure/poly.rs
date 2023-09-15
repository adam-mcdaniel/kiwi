//! # Poly-Procedure
//!
//! A polymorphic procedure of LIR code which can be applied to a list of arguments with type arguments.
//! This is mono-morphed into a `Procedure` when it is called with a list of type arguments.
//! A procedure is compiled down to a label in the assembly code.
use crate::lir::{ConstExpr, Env, Error, Expr, GetType, Mutability, Type, TypeCheck};
use core::fmt;
use std::{collections::HashMap, rc::Rc, sync::Mutex};

use log::{debug, trace, error};

use super::Procedure;

/// A polymorphic procedure of LIR code which can be applied to a list of arguments with type arguments.
/// This is mono-morphed into a `Procedure` when it is called with a list of type arguments.
/// A procedure is compiled down to a label in the assembly code.
#[derive(Clone, Debug)]
pub struct PolyProcedure {
    /// The name of the procedure.
    name: String,
    /// The type parameters of the procedure.
    ty_params: Vec<String>,
    /// The arguments of the procedure.
    args: Vec<(String, Mutability, Type)>,
    /// The return type of the procedure.
    ret: Type,
    /// The body of the procedure.
    body: Box<Expr>,
    /// The monomorphs of the procedure.
    monomorphs: Rc<Mutex<HashMap<String, Procedure>>>,
}

impl PartialEq for PolyProcedure {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.ty_params == other.ty_params
            && self.args == other.args
            && self.ret == other.ret
            && self.body == other.body
    }
}

impl PolyProcedure {
    /// Construct a new polymorphic procedure with type parameters, a list of arguments + their types,
    /// a return type, and the body of the procedure.
    pub fn new(
        name: String,
        ty_params: Vec<String>,
        args: Vec<(String, Mutability, Type)>,
        ret: Type,
        body: impl Into<Expr>,
    ) -> Self {
        Self {
            name,
            ty_params,
            args,
            ret,
            body: Box::new(body.into()),
            monomorphs: Rc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Get the name of this polymorphic procedure.
    /// This is not the mangled name, but the name known to the LIR front-end.
    /// The mangled name is unique for each monomorph of the procedure.
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Take some type arguments and produce a monomorphized version of the procedure.
    /// This monomorphized version can then be compiled directly. Additionally, the
    /// mono version of the procedure is memoized, so that it is only compiled once.
    pub fn monomorphize(&self, ty_args: Vec<Type>, env: &Env) -> Result<Procedure, Error> {
        debug!("Monomorphizing {} with {:?}", self, ty_args);

        // This is a helper function to distribute the defined type
        // arguments over the body and arguments of the function.

        // Simplify all the type arguments until they are concrete
        let simplified_ty_args = ty_args
            .clone()
            .into_iter()
            .map(|ty| {
                // Simplify the type until it is concrete
                ty.simplify_until_concrete(env)
            })
            .collect::<Result<Vec<_>, Error>>()?;

        // This is a helper function to bind the type arguments to the type parameters.
        let bind_type_args = |ty: Type| -> Result<Type, Error> {
            // Add the type parameters to the given type,
            // and apply the arguments.
            let ty = Type::Apply(
                Box::new(Type::Poly(self.ty_params.clone(), Box::new(ty))),
                simplified_ty_args.clone(),
            );
            // Simplify the type until it is simple.
            // This reduces to the concrete version of the type application.
            ty.simplify_until_concrete(env)
        };

        // Distribute the type parameters over the body and arguments of the function.
        let args = self
            .args
            .clone()
            .into_iter()
            .map(|(name, mutability, t)| Ok((name, mutability, bind_type_args(t)?)))
            .collect::<Result<Vec<_>, Error>>()?;
        let ret = bind_type_args(self.ret.clone())?;
        let mut body = *self.body.clone();

        // Substitute the type arguments into the body of the function.
        body.substitute_types(&self.ty_params, &simplified_ty_args);

        // Wrap the body in a let expression to bind the type arguments.
        body = Expr::LetTypes(
            self.ty_params
                .iter()
                .zip(simplified_ty_args.iter())
                .map(|(a, b)| (a.clone(), b.clone()))
                .collect(),
            Box::new(body),
        );

        // Generate a mangled name for the monomorphized procedure.
        let mangled_name = format!("__MONOMORPHIZED_({ty_args:?}){}{args:?}{ret:?}", self.name);

        // Memoize the monomorphized procedure.
        let mut monomorphs = self.monomorphs.lock().unwrap();
        // If the monomorphized procedure has already been memoized, return it, otherwise memoize it.
        let monomorph = monomorphs
            .entry(mangled_name.clone())
            .or_insert_with(|| Procedure::new(Some(mangled_name.clone()), args, ret, body))
            .clone();
        // Unlock the mutex to prevent a deadlock.
        drop(monomorphs);

        // Return the monomorphized procedure.
        Ok(monomorph)
    }
}

impl GetType for PolyProcedure {
    fn get_type_checked(&self, _env: &Env, _i: usize) -> Result<Type, Error> {
        Ok(Type::Poly(
            self.ty_params.clone(),
            Box::new(Type::Proc(
                self.args.iter().map(|(_, _, t)| t.clone()).collect(),
                Box::new(self.ret.clone()),
            )),
        ))
    }

    fn substitute(&mut self, name: &str, ty: &Type) {
        if self.ty_params.contains(&name.to_string()) {
            return;
        }
        self.args
            .iter_mut()
            .for_each(|(_, _, t)| *t = t.substitute(name, ty));
        self.ret = self.ret.substitute(name, ty);
    }
}

impl TypeCheck for PolyProcedure {
    fn type_check(&self, env: &Env) -> Result<(), Error> {
        trace!("Type checking {self}");
        // Create a new scope for the procedure's body, and define the arguments for the scope.
        let mut new_env = env.new_scope();
        for ty_param in &self.ty_params {
            new_env.define_type(ty_param, Type::Unit(ty_param.clone(), Box::new(Type::None)));
        }

        new_env.define_args(self.args.clone())?;
        new_env.set_expected_return_type(self.ret.clone());

        // Typecheck the types of the arguments and return value
        for (_, _, t) in &self.args {
            t.type_check(&new_env)?;
        }
        self.ret.type_check(&new_env)?;

        // Get the type of the procedure's body, and confirm that it matches the return type.
        let body_type = self.body.get_type(&new_env)?;

        if !body_type.can_decay_to(&self.ret, &new_env)? {
            error!("Mismatched types: expected {}, found {}", self.ret, body_type);

            Err(Error::MismatchedTypes {
                expected: self.ret.clone(),
                found: body_type,
                expr: ConstExpr::PolyProc(self.clone()).into(),
            })
        } else {
            // Typecheck the procedure's body.
            self.body.type_check(&new_env)
        }
    }
}

impl fmt::Display for PolyProcedure {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "proc[")?;
        for (i, ty_param) in self.ty_params.iter().enumerate() {
            write!(f, "{}", ty_param)?;
            if i < self.ty_params.len() - 1 {
                write!(f, ", ")?;
            }
        }
        write!(f, "](")?;
        for (i, (name, mutability, ty)) in self.args.iter().enumerate() {
            if mutability.is_mutable() {
                write!(f, "mut ")?;
            }
            write!(f, "{name}: {ty}")?;
            if i < self.args.len() - 1 {
                write!(f, ", ")?
            }
        }
        write!(f, ") -> {} = {}", self.ret, self.body)
    }
}
