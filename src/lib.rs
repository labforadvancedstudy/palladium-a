// Alan von Palladium Compiler Library
// "The foundation where legends are built"

pub mod ast;
pub mod async_runtime;
pub mod bootstrap;
pub mod builtins;
pub mod codegen;
pub mod driver;
pub mod effects;
pub mod errors;
pub mod lexer;
pub mod linker;
pub mod lsp;
pub mod macros;
pub mod optimizer;
pub mod ownership;
pub mod package;
pub mod parser;
pub mod resolver;
pub mod runtime;
pub mod runtime_paths;
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

/// The version cargo built this library from. Derived, never typed.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

// A FOURTH SPELLING OF THE VERSION USED TO LIVE HERE, AND IT WAS DELETED.
//
//     pub const VERSION_STRING: &str = concat!(
//         "Alan von Palladium Compiler v", env!("CARGO_PKG_VERSION"), "-alpha");
//
// It derived the version and then glued `-alpha` onto it, so with Cargo.toml at
// 0.3.0 it rendered `v0.3.0-alpha` — a version this package does not have and
// never had, which is verbatim the defect `pdc --version` was fixed for one file
// over. Two answers were available: correct the string, or delete it.
//
// DELETED, for a reason that is a measurement and not a preference: it had ZERO
// call sites in the tracked tree (grepped across src tests benchmarks tools
// stdlib bootstrap examples scripts Makefile — only its own definition), so
// nothing rendered it and nothing would have noticed it was wrong. Correcting it
// would have kept a public string with no consumer, which is a surface that can
// drift again and a claim nobody reads. It was also invisible to BOTH gates by
// construction: `make version-gate` runs the binary and reads `--version`, and
// no binary printed this; the source scan in tests/version_matches_cargo_toml.rs
// looks for version-SHAPED literals, and `"-alpha"` on its own has no digits in
// it. So the deletion carries its own control, in that test: a bare pre-release
// tail (`"-alpha"`, `"-beta"`, `"-rc"`, …) may no longer appear as a string
// literal anywhere under src/, which is red the moment this const comes back.
