// Alan von Palladium Compiler Library
// "The foundation where legends are built"

pub mod ast;
pub mod codegen;
pub mod driver;
pub mod errors;
pub mod lexer;
pub mod optimizer;
pub mod parser;
pub mod resolver;
pub mod runtime;
pub mod typeck;

// Re-export main components
pub use driver::Driver;
pub use errors::{CompileError, Result};

/// The main entry point for compilation
pub fn compile(source: &str, filename: &str) -> Result<()> {
    let driver = Driver::new();
    driver.compile_string(source, filename)?;
    Ok(())
}

/// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const VERSION_STRING: &str = concat!(
    "Alan von Palladium Compiler v",
    env!("CARGO_PKG_VERSION"),
    "-alpha"
);
