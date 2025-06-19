// Alan von Palladium Compiler Library
// "The foundation where legends are built"

pub mod ast;
pub mod async_runtime;
pub mod bootstrap;
pub mod codegen;
pub mod driver;
pub mod effects;
pub mod errors;
pub mod lexer;
pub mod lsp;
pub mod macros;
pub mod optimizer;
pub mod ownership;
pub mod package;
pub mod parser;
pub mod resolver;
pub mod runtime;
pub mod typeck;
pub mod unsafe_ops;

#[cfg(test)]
mod tests;

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
