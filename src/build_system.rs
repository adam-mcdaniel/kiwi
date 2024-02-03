use maplit::btreemap;

use sage::{
    lir::*,
    parse::*,
    targets::{self, SageBuild, CompiledTarget},
    vm::*,
    LOGO_WITH_COLOR, *,
};
use std::{
    fmt,
    fs::{read_to_string, write},
};
use clap::*;

use log::*;
use env_logger::*;

// The stack sizes of the threads used to compile the code.
const RELEASE_STACK_SIZE_MB: usize = 512;
const DEBUG_STACK_SIZE_MB: usize = RELEASE_STACK_SIZE_MB;

#[derive(clap::ValueEnum, Default, Clone, Debug, PartialEq)]
enum LogLevel {
    /// Print all the errors
    Error,
    /// Print all the warnings and errors
    Warn,
    /// Print all the info messages
    Info,
    /// Print all the debug information
    Debug,
    /// Trace the compilation of the program
    Trace,
    /// Display no messages
    #[default]
    Off,
}

/// The argument parser for the CLI.
#[derive(Parser, Debug)]
#[clap(author, version, about = Some(LOGO_WITH_COLOR), long_about = Some(LOGO_WITH_COLOR), max_term_width=90)]
struct Args {
    /// The input file to compiler.
    #[clap(value_parser)]
    input: String,

    /// The log level to use.
    #[clap(short, long, value_parser, default_value = "off")]
    log_level: LogLevel,

    /// The symbol to debug (if any exists). This will
    /// also enable debug logging.
    #[clap(short, long, value_parser)]
    debug: Option<String>,
}

/// The types of errors returned by the CLI.
enum Error {
    /// With the given source code location and the source code itself.
    WithSourceCode {
        loc: SourceCodeLocation,
        source_code: String,
        err: Box<Self>,
    },
    /// Error in reading source or writing generated code.
    IO(std::io::Error),
    /// Error parsing the source code.
    Parse(String),
    /// Error generated when compiling LIR code.
    LirError(lir::Error),
    /// Error generated when assembling input code.
    AsmError(asm::Error),
    /// Error generated by the interpreter executing input code.
    InterpreterError(String),
    /// Error when building the virtual machine code for a given target.
    BuildError(String),
    /// Invalid source code (expected core but got standard).
    InvalidSource(String),
}

impl Error {
    pub fn annotate_with_source(self, code: &str) -> Self {
        match self {
            Self::LirError(lir::Error::Annotated(ref err, ref metadata)) => {
                if let Some(loc) = metadata.location().cloned() {
                    Self::WithSourceCode {
                        loc,
                        source_code: code.to_owned(),
                        err: Box::new(Error::LirError(*err.clone())),
                    }
                } else {
                    self
                }
            }
            _ => self,
        }
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::IO(e) => write!(f, "IO error: {:?}", e),
            Error::Parse(e) => write!(f, "Parse error: {}", e),
            Error::AsmError(e) => write!(f, "Assembly error: {:?}", e),
            Error::LirError(e) => write!(f, "LIR error: {}", e),
            Error::WithSourceCode {
                loc,
                source_code,
                err,
            } => {
                // use codespan_reporting::files::SimpleFiles;
                use codespan_reporting::diagnostic::{Diagnostic, Label};
                use codespan_reporting::files::SimpleFiles;
                use codespan_reporting::term::{
                    emit,
                    termcolor::{ColorChoice, StandardStream},
                };
                use no_comment::{languages, IntoWithoutComments};

                let SourceCodeLocation {
                    line,
                    column,
                    filename,
                    offset,
                    length,
                } = loc;

                let mut files = SimpleFiles::new();

                let source_code = source_code
                    .to_string()
                    .chars()
                    .without_comments(languages::rust())
                    .collect::<String>();

                let filename = filename.clone().unwrap_or("unknown".to_string());

                let file_id = files.add(
                    filename.clone(),
                    source_code,
                );

                let loc = format!("{}:{}:{}:{}", filename, line, column, offset);

                // let code = format!("{}\n{}^", code, " ".repeat(*column - 1));
                // write!(f, "Error at {}:\n{}\n{:?}", loc, code, err)?

                let diagnostic = Diagnostic::error()
                    .with_message(format!("Error at {}", loc))
                    .with_labels(vec![Label::primary(
                        file_id,
                        *offset..*offset + length.unwrap_or(0),
                    )
                        .with_message(format!("{err:?}"))]);

                let writer = StandardStream::stderr(ColorChoice::Always);
                let config = codespan_reporting::term::Config::default();

                emit(&mut writer.lock(), &config, &files, &diagnostic).unwrap();

                Ok(())
            }
            Error::InterpreterError(e) => write!(f, "Interpreter error: {}", e),
            Error::BuildError(e) => write!(f, "Build error: {}", e),
            Error::InvalidSource(e) => write!(f, "Invalid source: {}", e),
        }
    }
}


// /// Write some contents to a file.
// fn write_file(file: String, contents: String) -> Result<(), Error> {
//     write(file, contents).map_err(Error::IO)
// }

/// Read the contents of a file.
fn read_file(name: &str) -> Result<String, Error> {
    ::std::fs::read_to_string(name).map_err(Error::IO)
}

/// Run the CLI.
fn cli() {
    // Parse the arguments to the CLI.
    let args = Args::parse();
    let mut builder = env_logger::Builder::from_default_env();
    builder.format_timestamp(None);

    let target = args.debug.as_deref();

    // Set the log level.
    builder.filter(target, match args.log_level {
        LogLevel::Error if args.debug.is_none() => log::LevelFilter::Error,
        LogLevel::Warn if args.debug.is_none() => log::LevelFilter::Warn,
        LogLevel::Off if args.debug.is_none() => log::LevelFilter::Error,
        LogLevel::Info if args.debug.is_none() =>log::LevelFilter::Info,
        LogLevel::Trace => log::LevelFilter::Trace,
        _ => log::LevelFilter::Debug,
    });

    builder.init();

    let package_config = &::std::path::Path::new("Build.toml");
    let builder = SageBuild::from_package_config(&package_config).unwrap();
    
    match read_file(&args.input) {
        Ok(code) => {
            match builder.build(&code, Some(&args.input)) {
                Ok(()) => {
                    info!("Build successful");
                }
                Err(e) => {
                    error!("Error in build system: {:?}", e);
                }
            }
        }
        Err(e) => {
            error!("Error reading file: {e:?}");
        }
    }
}


fn main() {
    // let code = r#"
    //     extern def add(a: Int, b: Int): Int;
    //     println("Hello, World! 1 + 2 is ", add(1, 2));
    // "#;

    // use lir::Type::*;

    // println!("{}", eval(code, "Testing", vec![
    //     (
    //         // A function add that takes two Ints and returns an Int
    //         ("add", Tuple(vec![Int, Int]), Int),
    //         // The function body
    //         |channel, _| {
    //             let a = channel.pop_front().unwrap();
    //             let b = channel.pop_front().unwrap();
    //             channel.push_back(a + b);
    //         }
    //     ),
    // ]).unwrap());

    // If we're in debug mode, start the compilation in a separate thread.
    // This is to allow the process to have more stack space.
    if !cfg!(debug_assertions) {
        let child = std::thread::Builder::new()
            .stack_size(RELEASE_STACK_SIZE_MB * 1024 * 1024)
            .spawn(cli)
            .unwrap();

        // Wait for the thread to finish.
        child.join().unwrap()
    } else {
        let child = std::thread::Builder::new()
            .stack_size(DEBUG_STACK_SIZE_MB * 1024 * 1024)
            .spawn(cli)
            .unwrap();

        // Wait for the thread to finish.
        child.join().unwrap()
    }
}