// Alan von Palladium Compiler Library
// "The foundation where legends are built"

pub mod driver;
pub mod lexer;
pub mod parser;
pub mod ast;
pub mod typeck;
pub mod codegen;
pub mod errors;
pub mod runtime;

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