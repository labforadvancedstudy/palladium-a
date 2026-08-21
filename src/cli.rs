// CLI interface for Palladium
// "Command line interface for legends"

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "pdc")]
#[command(author = "Alan von Palladium Team")]
// Derived from Cargo.toml, never written out by hand. The literal that used to
// be here said `0.1.0-alpha` while the package was at 0.3.0 — two releases of
// drift in the one string a user reads to find out what they are running. A
// binary that misreports its own version cannot be used to reproduce anything.
// `tests/version_matches_cargo_toml.rs` fails if this becomes a literal again.
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "Alan von Palladium Compiler - Where Legends Compile")]
#[command(long_about = r#"
Alan von Palladium Compiler
"Turing's Proofs Meet von Neumann's Performance"

A systems programming language that combines formal verification 
with high performance, inspired by the legendary minds of computing.
"#)]
#[command(arg_required_else_help = true)]
pub struct Cli {
    /// Print the resolved Palladium runtime directory and exit
    ///
    /// Packaging check: verifies an installed pdc can find its runtime
    /// without compiling anything.
    #[arg(long)]
    pub print_runtime: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Compile a Palladium source file
    Compile {
        /// The source file to compile
        file: PathBuf,

        /// Output file name
        #[arg(short = 'o', long)]
        output: Option<String>,

        /// Use LLVM backend instead of C
        #[arg(long)]
        llvm: bool,

        /// Optimize harder: gcc -O3 (default is -O2)
        #[arg(short = 'O', long)]
        optimize: bool,

        /// Disable optimization: gcc -O0, for debugging the generated C
        #[arg(long, conflicts_with = "optimize")]
        no_opt: bool,
    },

    /// Compile and run a Palladium source file
    Run {
        /// The source file to run
        file: PathBuf,

        /// Use LLVM backend instead of C
        #[arg(long)]
        llvm: bool,

        /// Optimize harder: gcc -O3 (default is -O2)
        #[arg(short = 'O', long)]
        optimize: bool,

        /// Disable optimization: gcc -O0, for debugging the generated C
        #[arg(long, conflicts_with = "optimize")]
        no_opt: bool,

        /// Arguments to pass to the program
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },

    /// Create a new Palladium package
    New {
        /// Package name
        name: String,

        /// Path where to create the package (defaults to current directory)
        #[arg(long)]
        path: Option<PathBuf>,

        /// Create a library package instead of binary
        #[arg(long)]
        lib: bool,
    },

    /// Initialize a new package in current directory
    Init {
        /// Package name (defaults to directory name)
        name: Option<String>,

        /// Create a library package instead of binary
        #[arg(long)]
        lib: bool,
    },

    /// Build the current package
    Build {
        /// Build in release mode with optimizations
        #[arg(long)]
        release: bool,

        /// Use LLVM backend
        #[arg(long)]
        llvm: bool,
    },

    /// Run the current package
    PackageRun {
        /// Build in release mode with optimizations
        #[arg(long)]
        release: bool,

        /// Arguments to pass to the program
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },

    /// Add a dependency to the current package
    Add {
        /// Dependency name
        name: String,

        /// Version requirement (defaults to latest)
        #[arg(long)]
        version: Option<String>,

        /// Add as dev dependency
        #[arg(long)]
        dev: bool,

        /// Add as build dependency
        #[arg(long)]
        build: bool,
    },

    /// Install dependencies
    Install,

    /// Update dependencies
    Update {
        /// Package to update (updates all if not specified)
        package: Option<String>,
    },

    /// Check the current package for errors without building
    Check {
        /// Check all targets (including tests and examples)
        #[arg(long)]
        all: bool,
    },

    /// Run tests
    Test {
        /// Test name pattern
        pattern: Option<String>,

        /// Run in release mode
        #[arg(long)]
        release: bool,

        /// Show output from passing tests
        #[arg(long)]
        nocapture: bool,
    },

    /// Format source code
    Fmt {
        /// Check formatting without changing files
        #[arg(long)]
        check: bool,

        /// Format all packages in workspace
        #[arg(long)]
        all: bool,
    },

    /// Run the linter
    Lint {
        /// Automatically fix problems
        #[arg(long)]
        fix: bool,

        /// Lint all packages in workspace
        #[arg(long)]
        all: bool,
    },

    /// Documentation commands
    Doc {
        /// Open documentation in browser
        #[arg(long)]
        open: bool,

        /// Include private items
        #[arg(long)]
        private: bool,
    },

    /// Clean build artifacts
    Clean {
        /// Remove target directory
        #[arg(long)]
        target: bool,

        /// Remove package cache
        #[arg(long)]
        cache: bool,
    },

    /// Bootstrap compiler commands
    Bootstrap {
        #[command(subcommand)]
        command: BootstrapCommands,
    },
}

#[derive(Subcommand)]
pub enum BootstrapCommands {
    /// Build the bootstrap compiler
    Build,

    /// Test self-hosting capability
    SelfHost,

    /// Validate bootstrap compiler against Rust compiler
    Validate {
        /// File to validate
        file: PathBuf,
    },

    /// Compile a file using the bootstrap compiler
    Compile {
        /// File to compile
        file: PathBuf,
    },
}
