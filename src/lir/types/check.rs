//! # Type Checking
//! 
//! This module contains the type checking logic for the Lower Intermediate Representation.
//! Type checking is the process of ensuring that the types of expressions are sound.
//! This performs a number of checks, including:
//! - Ensuring that all types are defined.
//! - Ensuring that all constants are defined.
//! - Ensuring that all procedures are defined.
//! - Ensuring that all variables are defined.
//! - Ensuring that all array lengths are non-negative.
//! - Ensuring that you don't attempt to access a variable that is out of scope.
use super::*;

/// A trait used to enforce type checking.
/// 
/// Whenever this is applied, it will return `Ok(())`
/// if the typing is sound, and `Err(...)` if it is not.
pub trait TypeCheck {
    /// Type check the expression.
    fn type_check(&self, env: &Env) -> Result<(), Error>;
}

/// Check the soundness of a given type in the environment.
impl TypeCheck for Type {
    fn type_check(&self, env: &Env) -> Result<(), Error> {
        // TODO: Also add checks for infinitely sized types.
        match self {
            Self::Any
            | Self::Never
            | Self::None
            | Self::Cell
            | Self::Int
            | Self::Float
            | Self::Bool
            | Self::Char
            | Self::Enum(_) => Ok(()),

            // Units are sound if their inner type is sound.
            Self::Unit(_unit_name, t) => t.type_check(env),

            // Symbols are sound if they are defined in the environment
            Self::Symbol(name) => {
                if env.get_type(name).is_some() {
                    Ok(())
                } else {
                    Err(Error::TypeNotDefined(name.clone()))
                }
            }
            // Let bindings are sound if their inner types are sound.
            Self::Let(name, t, ret) => {
                // Create a new environment with the type defined.
                let mut new_env = env.clone();
                new_env.define_type(name, *t.clone());
                // Check the inner type and the return type.
                t.type_check(&new_env)?;
                ret.type_check(&new_env)
            }
            // Arrays are sound if their inner type is sound.
            Self::Array(t, len) => {
                // Check the inner type and the length constant-expression.
                t.type_check(env)?;
                len.clone().type_check(env)?;
                // Check that the length is non-negative.
                if len.clone().as_int(env)? < 0 {
                    // If it is negative, return an error.
                    return Err(Error::NegativeArrayLength(Expr::ConstExpr(*len.clone())));
                }
                // Otherwise, return success.
                Ok(())
            }
            Self::Tuple(ts) => {
                // Check each inner type.
                for t in ts {
                    // Check the inner type.
                    t.type_check(env)?;
                }
                // Return success if all the types are sound.
                Ok(())
            }
            Self::Struct(fields) | Self::Union(fields) => {
                // Check each inner type.
                for t in fields.values() {
                    // Check the inner type.
                    t.type_check(env)?;
                }
                // Return success if all the types are sound.
                Ok(())
            }

            Self::Proc(args, ret) => {
                // Check each argument type.
                for t in args {
                    // Check the argument type.
                    t.type_check(env)?;
                }
                // Check the return type.
                ret.type_check(env)
            }

            // Pointers are sound if their inner type is sound.
            Self::Pointer(t) => t.type_check(env),
        }
    }
}


/// Check the type-soundness of a given expression.
impl TypeCheck for Expr {
    fn type_check(&self, env: &Env) -> Result<(), Error> {
        match self {
            Self::UnaryOp(unop, expr) => {
                // Check if the unary operator is sound with
                // the given expression.
                unop.type_check(expr, env)
            }
            Self::BinaryOp(binop, lhs, rhs) => {
                // Check if the binary operator is sound with
                // the given expressions.
                binop.type_check(lhs, rhs, env)
            }
            Self::TernaryOp(ternop, a, b, c) => {
                // Check if the ternary operator is sound with
                // the given expressions.
                ternop.type_check(a, b, c, env)
            }
            Self::AssignOp(op, dst, src) => {
                // Check if the assignment operator is sound with
                // the given expressions.
                op.type_check(dst, src, env)
            }

            // Typecheck the inner constant expression.
            Self::ConstExpr(c) => c.type_check(env),

            // Typecheck a block of expressions.
            Self::Many(exprs) => {
                // Typecheck each expression.
                for expr in exprs {
                    // Check the inner expression.
                    expr.type_check(env)?;
                }
                // Return success if all the expressions are sound.
                Ok(())
            }

            // Typecheck a declaration of a constant.
            Self::LetConst(name, e, ret) => {
                // Typecheck the constant expression we're assigning to the variable.
                let mut new_env = env.clone();
                new_env.define_const(name.clone(), e.clone());
                e.type_check(&new_env)?;
                // Typecheck the resulting expression with the constant
                // defined in the environment.
                ret.type_check(&new_env)
            }

            // Typecheck a declaration of multiple constants.
            Self::LetConsts(constants, ret) => {
                // Add all the constants to the scope.
                let mut new_env = env.clone();
                for (name, c) in constants {
                    // Define the constant in the environment.
                    new_env.define_const(name, c.clone());
                }
                // Typecheck the constant expression we're assigning to each name.
                for c in constants.values() {
                    // Typecheck the constant expression in the new environment.
                    c.type_check(&new_env)?;
                }
                // Typecheck the resulting expression with the constants
                // defined in the environment.
                ret.type_check(&new_env)
            }

            // Typecheck a declaration of a procedure.
            Self::LetProc(var, proc, ret) => {
                // Create a new environment with the procedure defined.
                let mut new_env = env.clone();
                new_env.define_proc(var.clone(), proc.clone());
                // Typecheck the procedure we're defining.
                proc.type_check(&new_env)?;
                // Typecheck the resulting expression with the procedure
                // defined in the environment.
                ret.type_check(&new_env)
            }

            Self::LetProcs(procs, ret) => {
                // Create a new environment with the procedures defined.
                let mut new_env = env.clone();
                for (name, proc) in procs {
                    // Define the procedure in the environment.
                    new_env.define_proc(name, proc.clone());
                }
                // Typecheck the procedures we're defining.
                for (_, proc) in procs {
                    // Typecheck the procedure in the new environment.
                    proc.type_check(&new_env)?;
                }
                // Typecheck the resulting expression with the procedures
                // defined in the environment.
                ret.type_check(&new_env)
            }

            // Typecheck a declaration of a type.
            Self::LetType(name, t, ret) => {
                // Create a new environment with the type defined.
                let mut new_env = env.clone();
                new_env.define_type(name.clone(), t.clone());
                // Typecheck the type we're defining.
                t.type_check(&new_env)?;
                // Typecheck the resulting expression with the type
                // defined in the environment.
                ret.type_check(&new_env)
            }

            // Typecheck a declaration of multiple types.
            Self::LetTypes(types, ret) => {
                // Create a new environment with the types defined.
                let mut new_env = env.clone();
                for (name, ty) in types {
                    // Define the type in the environment.
                    new_env.define_type(name, ty.clone());
                }
                // Typecheck the types we're defining.
                for (_, t) in types {
                    // Typecheck the type in the new environment.
                    t.type_check(&new_env)?;
                }
                // Typecheck the resulting expression with the types
                // defined in the environment.
                ret.type_check(&new_env)
            }

            // Typecheck a declaration of a variable.
            Self::LetVar(var, t, e, ret) => {
                // Typecheck the expression we're assigning to the variable.
                e.type_check(env)?;
                // Get the inferred type of the expression.
                let inferred_t = e.get_type(env)?;
                // If there's a type specification for the variable, check it.
                if let Some(t) = t {
                    // Typecheck the type.
                    t.type_check(env)?;

                    // Check that the inferred type is compatible with the type specified.
                    if !inferred_t.equals(t, env)? {
                        return Err(Error::MismatchedTypes {
                            expected: t.clone(),
                            found: inferred_t,
                            expr: self.clone(),
                        });
                    }
                }

                // Create a new environment with the variable defined.
                let mut new_env = env.clone();
                new_env.define_var(var, t.clone().unwrap_or(inferred_t))?;
                // Typecheck the resulting expression with the variable
                // defined in the environment.
                ret.type_check(&new_env)
            }

            // Typecheck a declaration of multiple variables.
            Self::LetVars(vars, ret) => {
                let mut new_env = env.clone();
                for (var, t, e) in vars {
                    // Typecheck the expression we're assigning to the variable.
                    e.type_check(&new_env)?;
                    // Get the inferred type of the expression.
                    let inferred_t = e.get_type(&new_env)?;
                    // If there's a type specification for the variable, check it.
                    if let Some(t) = t {
                        // Typecheck the type.
                        t.type_check(env)?;

                        // Check that the inferred type is compatible with the type specified.
                        if !inferred_t.equals(t, env)? {
                            return Err(Error::MismatchedTypes {
                                expected: t.clone(),
                                found: inferred_t,
                                expr: self.clone(),
                            });
                        }
                    }
                    // Define the variable in the environment.
                    new_env.define_var(var, t.clone().unwrap_or(inferred_t))?;
                }
                // Typecheck the resulting expression with the variables
                // defined in the environment.
                ret.type_check(&new_env)
            }

            Self::While(cond, body) => {
                // Typecheck the condition.
                cond.type_check(env)?;
                // Typecheck the body.
                body.type_check(env)
            }

            Self::If(cond, t, e) => {
                // Typecheck the condition.
                cond.type_check(env)?;
                // Typecheck the then and else branches.
                t.type_check(env)?;
                e.type_check(env)?;

                // Get the types of the then and else branches.
                let t_type = t.get_type(env)?;
                let e_type = e.get_type(env)?;
                // Check that the types of the then and else branches are compatible.
                if !t_type.equals(&e_type, env)? {
                    // If they're not, return an error.
                    return Err(Error::MismatchedTypes {
                        expected: t_type,
                        found: e_type,
                        expr: self.clone(),
                    });
                }
                Ok(())
            }

            Self::When(cond, t, e) => {
                // Typecheck the condition.
                cond.type_check(env)?;
                // Typecheck the then and else branches.
                t.type_check(env)?;
                e.type_check(env)
                // Since `when` expressions are computed at compile time,
                // we don't have to care about matching the types of the then and else branches.
            }

            // Typecheck a reference to a value.
            Self::Refer(e) => e.type_check(env),
            // Typecheck a dereference of a pointer.
            Self::Deref(e) => {
                // Typecheck the expression which evaluates
                // to the address we will dereference.
                e.type_check(env)?;
                // Get the type of the expression.
                let t = e.get_type(env)?;
                // Check that the type is a pointer.
                if let Type::Pointer(_) = t {
                    // If it is, return success.
                    Ok(())
                } else {
                    // If it isn't, return an error.
                    Err(Error::MismatchedTypes {
                        // The expected type is a pointer.
                        expected: Type::Pointer(Box::new(Type::Any)),
                        found: t,
                        expr: self.clone(),
                    })
                }
            }

            // Typecheck an assignment of a value to the data stored at
            // a given pointer.
            Self::DerefMut(ptr, val) => {
                // Typecheck the pointer and the value we want to assign.
                ptr.type_check(env)?;
                val.type_check(env)?;
                // Get the types of the pointer and the value.
                let ptr_type = ptr.get_type(env)?;
                let val_type = val.get_type(env)?;
                // Check that the pointer is a pointer.
                if let Type::Pointer(t) = ptr_type {
                    // Check that the type of the value is compatible
                    // with the type of data stored at the pointer's
                    // address.
                    if t.equals(&val_type, env)? {
                        // If it is, return success.
                        Ok(())
                    } else {
                        // If it isn't, return an error.
                        Err(Error::MismatchedTypes {
                            expected: val_type,
                            found: *t,
                            expr: self.clone(),
                        })
                    }
                } else {
                    // If the destination to store isn't a pointer, return an error.
                    Err(Error::MismatchedTypes {
                        expected: Type::Pointer(Box::new(Type::Any)),
                        found: ptr_type,
                        expr: self.clone(),
                    })
                }
            }

            // Typecheck a function application.
            Self::Apply(f, args) => {
                // Typecheck the expression we want to call as a procedure.
                f.type_check(env)?;
                // Typecheck the supplied arguments.
                for arg in args {
                    arg.type_check(env)?;
                }
                // Get the type of the function.
                let f_type = f.get_type(env)?;
                // Infer the types of the supplied arguments.
                let mut args_inferred = vec![];
                for arg in args {
                    args_inferred.push(arg.get_type(env)?);
                }
                if let Type::Proc(args_t, ret_t) = f_type {
                    // If the number of arguments is incorrect, then return an error.
                    if args_t.len() != args_inferred.len() {
                        return Err(Error::MismatchedTypes {
                            expected: Type::Proc(args_t, ret_t.clone()),
                            found: Type::Proc(args_inferred, ret_t),
                            expr: self.clone(),
                        });
                    }
                    // If the function is a procedure, confirm that the type of each
                    // argument matches the the type of the supplied value.
                    for (arg_t, arg) in args_t.into_iter().zip(args_inferred.into_iter()) {
                        // If the types don't match, return an error.
                        if !arg_t.equals(&arg, env)? {
                            return Err(Error::MismatchedTypes {
                                expected: arg_t,
                                found: arg,
                                expr: self.clone(),
                            });
                        }
                    }
                    Ok(())
                } else {
                    // If the function is not a procedure, return an error.
                    Err(Error::MismatchedTypes {
                        expected: Type::Proc(args_inferred, Box::new(Type::Any)),
                        found: f_type,
                        expr: self.clone(),
                    })
                }
            }

            // Typecheck a return statement.
            Self::Return(e) => e.type_check(env),

            // Typecheck an array or tuple literal.
            Self::Array(items) => {
                let mut last_type: Option<Type> = None;
                // Typecheck each item in the array.
                for item in items {
                    // Typecheck the item.
                    item.type_check(env)?;
                    // Get the type of the item.
                    let item_type = item.get_type(env)?;
                    // If the type of the item is different from the last item,
                    if let Some(last_type) = last_type {
                        // Confirm that the type of the item is the same as the
                        // last item.
                        if !last_type.equals(&item_type, env)? {
                            // If it isn't, return an error.
                            return Err(Error::MismatchedTypes {
                                expected: last_type,
                                found: item_type,
                                expr: self.clone()
                            })
                        }
                    }
                    last_type = Some(item_type);
                }
                Ok(())
            }
            Self::Tuple(elems) => {
                for elem in elems {
                    elem.type_check(env)?;
                }
                Ok(())
            }

            // Typecheck a struct literal.
            Self::Struct(fields) => {
                for field_expr in fields.values() {
                    field_expr.type_check(env)?;
                }
                Ok(())
            }

            // Typecheck a union literal.
            Self::Union(t, variant, val) => {
                // Typecheck the type.
                t.type_check(env)?;
                if let Type::Union(fields) = t.clone().simplify(env)? {
                    // Confirm that the variant is a valid variant.
                    if let Some(ty) = fields.get(variant) {
                        // Typecheck the value assigned to the variant.
                        val.type_check(env)?;
                        let found = val.get_type(env)?;
                        if !ty.equals(&found, env)? {
                            return Err(Error::MismatchedTypes {
                                expected: ty.clone(),
                                found,
                                expr: self.clone(),
                            });
                        }
                        Ok(())
                    } else {
                        Err(Error::VariantNotFound(t.clone(), variant.clone()))
                    }
                } else {
                    Err(Error::VariantNotFound(t.clone(), variant.clone()))
                }
            }

            // Typecheck a type-cast.
            Self::As(e, t) => {
                // Typecheck the expression we want to cast.
                e.type_check(env)?;
                // Get the actual type of the expression.
                let original_t = e.get_type(env)?;

                // Check that the cast is valid.
                if original_t.can_cast_to(t, env)? {
                    // If it is, return success.
                    Ok(())
                } else {
                    // Otherwise, it isn't a valid cast, so return an error.
                    Err(Error::InvalidAs(self.clone(), original_t, t.clone()))
                }
            }

            // Typecheck a member access.
            Self::Member(e, field) => {
                // Typecheck the expression we want to access a member of.
                e.type_check(env)?;
                // Get the type of the expression.
                let e_type = e.get_type(env)?;
                // Confirm that the type has the member we want to access
                // by calculating the offset of the member in the type.
                e_type.get_member_offset(field, e, env).map(|_| ())
            }

            // Typecheck an index access.
            Self::Index(val, idx) => {
                // Typecheck the expression we want to index.
                val.type_check(env)?;
                // Typecheck the index we want to access.
                idx.type_check(env)?;
                // Get the type of the expression we want to index.
                let val_type = val.get_type(env)?;
                // Get the type of the index.
                let idx_type = idx.get_type(env)?;
                // Confirm that the type is an array or pointer.
                match val_type {
                    Type::Array(_, _) | Type::Pointer(_) => {}
                    // If it isn't, return an error.
                    _ => return Err(Error::InvalidIndex(self.clone())),
                }

                // Confirm that the index is an integer.
                if let Type::Int = idx_type {
                    // If it is, return success.
                    Ok(())
                } else {
                    // Otherwise, return an error.
                    Err(Error::InvalidIndex(self.clone()))
                }
            }
        }
    }
}


// Typecheck a constant expression.
impl TypeCheck for ConstExpr {
    fn type_check(&self, env: &Env) -> Result<(), Error> {
        match self {
            // These are all guaranteed to be valid, or 
            // to fail at compile time.
            Self::None
            | Self::Null
            | Self::Int(_)
            | Self::Float(_)
            | Self::Char(_)
            | Self::Bool(_)
            | Self::SizeOfType(_) => Ok(()),

            Self::TypeOf(expr) => expr.type_check(env),

            // Typecheck a constant type-cast.
            Self::As(expr, cast_ty) => {
                // Calculate the inferred type of the expression.
                let found = expr.get_type(env)?;
                // Confirm that the cast is valid.
                if !found.can_cast_to(&cast_ty, env)? {
                    // If it isn't, return an error.
                    return Err(Error::InvalidAs(
                        Expr::ConstExpr(*expr.clone()),
                        found,
                        cast_ty.clone()
                    ))
                }
                // If it is, return success.
                Ok(())
            }

            // Get the size of an expression in cells.
            Self::SizeOfExpr(e) => e.type_check(env),

            // Typecheck a core-builtin inline assembly procedure.
            Self::CoreBuiltin(builtin) => builtin.type_check(env),
            // Typecheck a standard-builtin inline assembly procedure.
            Self::StandardBuiltin(builtin) => builtin.type_check(env),
            // Typecheck a procedure.
            Self::Proc(proc) => proc.type_check(env),

            // Typecheck a symbol.
            Self::Symbol(name) => {
                // If there is some binding for the symbol, return success.
                if env.get_const(name).is_some()
                    || env.get_proc(name).is_some()
                    || env.get_var(name).is_some()
                {
                    // Return success.
                    Ok(())
                } else {
                    // If there is no binding for the symbol, return an error.
                    Err(Error::SymbolNotDefined(name.clone()))
                }
            }

            // Typecheck a variant of an enum.
            Self::Of(t, variant) => {
                // If the type is an enum, and the enum contains the variant,
                if let Type::Enum(variants) = t.clone().simplify(env)? {
                    // If the enum contains the variant, return success.
                    if variants.contains(variant) {
                        // Return success.
                        Ok(())
                    } else {
                        // Otherwise, the variant isn't contained in the enum,
                        // so return an error.
                        Err(Error::VariantNotFound(t.clone(), variant.clone()))
                    }
                } else {
                    // If the type isn't an enum, return an error.
                    Err(Error::VariantNotFound(t.clone(), variant.clone()))
                }
            }

            // Typecheck a tuple literal.
            Self::Tuple(items) => {
                // Typecheck each item in the tuple.
                for item in items {
                    // Typecheck the item.
                    item.type_check(env)?;
                }
                // Return success.
                Ok(())
            }

            // Typecheck an array literal.
            Self::Array(items) => {
                let mut last_type: Option<Type> = None;
                // Typecheck each item in the array.
                for item in items {
                    // Typecheck the item.
                    item.type_check(env)?;
                    // Get the type of the item.
                    let item_type = item.get_type(env)?;
                    // If the type of the item is different from the last item,
                    if let Some(last_type) = last_type {
                        // Confirm that the type of the item is the same as the
                        // last item.
                        if !last_type.equals(&item_type, env)? {
                            // If it isn't, return an error.
                            return Err(Error::MismatchedTypes {
                                expected: last_type,
                                found: item_type,
                                expr: Expr::ConstExpr(self.clone())
                            })
                        }
                    }
                    last_type = Some(item_type);
                }
                // Return success.
                Ok(())
            }

            // Typecheck a struct literal.
            Self::Struct(fields) => {
                // Typecheck each field in the struct.
                for item in fields.values() {
                    // Typecheck the item.
                    item.type_check(env)?;
                }
                // Return success.
                Ok(())
            }

            // Typecheck a union literal.
            Self::Union(t, variant, val) => {
                // Confirm the type supplied is a union.
                if let Type::Union(fields) = t.clone().simplify(env)? {
                    // Confirm that the variant is contained within the union.
                    if let Some(ty) = fields.get(variant) {
                        // Typecheck the value assigned to the variant.
                        val.type_check(env)?;
                        let found = val.get_type(env)?;
                        if !ty.equals(&found, env)? {
                            return Err(Error::MismatchedTypes {
                                expected: ty.clone(),
                                found,
                                expr: Expr::ConstExpr(self.clone()),
                            });
                        }
                        Ok(())
                    } else {

                        Err(Error::VariantNotFound(t.clone(), variant.clone()))
                    }
                } else {
                    Err(Error::VariantNotFound(t.clone(), variant.clone()))
                }
            }
        }
    }
}