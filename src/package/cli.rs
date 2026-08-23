// Package manager CLI for Palladium
// "Command line interface for legendary package management"

use super::{
    build::{BuildConfig, BuildSystem},
    PackageManager,
};
use crate::errors::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Palladium package manager
#[derive(Parser)]
#[command(name = "pdm")]
#[command(about = "Palladium package manager - Manage your legendary packages")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create a new Palladium package
    New {
        /// Package name
        name: String,
        /// Path to create the package (defaults to package name)
        #[arg(long)]
        path: Option<PathBuf>,
    },

    /// Initialize a new package in the current directory
    Init {
        /// Package name (defaults to directory name)
        #[arg(long)]
        name: Option<String>,
    },

    /// Build the current package
    Build {
        /// Build in release mode
        #[arg(long, short)]
        release: bool,
        /// Use LLVM backend
        #[arg(long)]
        llvm: bool,
        /// Verbose output
        #[arg(long, short)]
        verbose: bool,
        /// Number of parallel jobs
        #[arg(long, short, default_value_t = 0)]
        jobs: usize,
        /// Features to enable
        #[arg(long)]
        features: Vec<String>,
    },

    /// Run the current package
    Run {
        /// Build in release mode
        #[arg(long, short)]
        release: bool,
        /// Arguments to pass to the program
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },

    /// Run tests
    Test {
        /// Test name filter
        filter: Option<String>,
        /// Build in release mode
        #[arg(long, short)]
        release: bool,
        /// Verbose output
        #[arg(long, short)]
        verbose: bool,
    },

    /// Clean build artifacts
    Clean,

    /// Add a dependency
    Add {
        /// Dependency name
        name: String,
        /// Version requirement
        #[arg(default_value = "*")]
        version: String,
        /// Add as dev dependency
        #[arg(long)]
        dev: bool,
    },

    /// Remove a dependency
    Remove {
        /// Dependency name
        name: String,
        /// Remove from dev dependencies
        #[arg(long)]
        dev: bool,
    },

    /// Update dependencies
    Update {
        /// Package to update (updates all if not specified)
        package: Option<String>,
    },

    /// Search for packages in the registry
    Search {
        /// Search query
        query: String,
        /// Maximum number of results
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },

    /// Show package information
    Info {
        /// Package name
        name: String,
    },

    /// Publish a package to the registry
    Publish {
        /// Dry run (don't actually publish)
        #[arg(long)]
        dry_run: bool,
    },

    /// Install a package globally
    Install {
        /// Package name
        name: String,
        /// Version to install
        #[arg(long)]
        version: Option<String>,
    },

    /// Uninstall a global package
    Uninstall {
        /// Package name
        name: String,
    },

    /// List installed packages
    List {
        /// Show global packages
        #[arg(long)]
        global: bool,
        /// Show dependency tree
        #[arg(long)]
        tree: bool,
    },
}

impl Cli {
    /// Execute the CLI command
    pub fn execute(self) -> Result<()> {
        match self.command {
            Commands::New { name, path } => {
                let target_path = path.unwrap_or_else(|| PathBuf::from(&name));

                if target_path.exists() {
                    return Err(crate::errors::CompileError::Generic(format!(
                        "Directory '{}' already exists",
                        target_path.display()
                    )));
                }

                std::fs::create_dir_all(&target_path)?;
                PackageManager::init(&name, &target_path)?;

                println!("\n📝 Next steps:");
                println!("   cd {}", target_path.display());
                println!("   pdm build");
                println!("   pdm run");
            }

            Commands::Init { name } => {
                let current_dir = std::env::current_dir()?;
                let package_name = name.unwrap_or_else(|| {
                    current_dir
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("my_package")
                        .to_string()
                });

                PackageManager::init(&package_name, &current_dir)?;
            }

            Commands::Build {
                release,
                llvm,
                verbose,
                jobs,
                features,
            } => {
                let config = BuildConfig {
                    release,
                    use_llvm: llvm,
                    verbose,
                    jobs: if jobs == 0 {
                        super::build::num_cpus::get()
                    } else {
                        jobs
                    },
                    features: features.into_iter().collect(),
                    ..Default::default()
                };

                let mut build_system = BuildSystem::new(config);
                build_system.build()?;
            }

            Commands::Run { release, args } => {
                let config = BuildConfig {
                    release,
                    ..Default::default()
                };

                // `run_reporting`, not `run`: `main() -> Result<()>` prints an
                // error and exits 1 for everything, so the taxonomy this branch
                // built (3 backend reject, 4 ill-typed C, 5 toolchain, 6
                // unexplained) died one frame above here, and a signalled child
                // was indistinguishable from a build failure.
                let mut build_system = BuildSystem::new(config);
                if let Err(o) = build_system.run_reporting(args) {
                    let code = o.exit_code();
                    if !matches!(o, crate::driver::RunOutcome::Child { .. }) {
                        eprintln!("\x1b[1;31merror:\x1b[0m {}", o.into_compile_error());
                    }
                    std::process::exit(code);
                }
            }

            Commands::Test {
                filter,
                release,
                verbose,
            } => {
                let config = BuildConfig {
                    release,
                    verbose,
                    ..Default::default()
                };

                let mut build_system = BuildSystem::new(config);
                build_system.test(filter.as_deref())?;
            }

            Commands::Clean => {
                let build_system = BuildSystem::new(BuildConfig::default());
                build_system.clean()?;
            }

            Commands::Add { name, version, dev } => {
                let mut pm = PackageManager::new()?;
                pm.add_dependency(&name, &version, dev)?;
            }

            Commands::Remove { name: _, dev: _ } => {
                println!("🚧 Remove command not yet implemented");
                // TODO: Implement dependency removal
            }

            Commands::Update { package: _ } => {
                println!("🚧 Update command not yet implemented");
                // TODO: Implement dependency updates
            }

            Commands::Search { query, limit } => {
                println!("🔍 Searching for '{}'...", query);
                println!("🚧 Search functionality not yet implemented");
                println!("   (Would show up to {} results)", limit);
                // TODO: Implement package search
            }

            Commands::Info { name } => {
                println!("📦 Package: {}", name);
                println!("🚧 Info command not yet implemented");
                // TODO: Implement package info
            }

            Commands::Publish { dry_run } => {
                if dry_run {
                    println!("🧪 Dry run mode");
                }
                println!("🚧 Publish command not yet implemented");
                // TODO: Implement package publishing
            }

            Commands::Install { name, version } => {
                println!(
                    "📥 Installing {} {}",
                    name,
                    version.as_deref().unwrap_or("latest")
                );
                println!("🚧 Install command not yet implemented");
                // TODO: Implement global package installation
            }

            Commands::Uninstall { name } => {
                println!("📤 Uninstalling {}", name);
                println!("🚧 Uninstall command not yet implemented");
                // TODO: Implement global package uninstallation
            }

            Commands::List { global, tree } => {
                if global {
                    println!("🌍 Global packages:");
                } else {
                    println!("📦 Local dependencies:");
                }

                if tree {
                    println!("🌳 Dependency tree view");
                }

                println!("🚧 List command not yet implemented");
                // TODO: Implement package listing
            }
        }

        Ok(())
    }
}

/// Run the package manager CLI
pub fn run_cli() -> Result<()> {
    let cli = Cli::parse();
    cli.execute()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        // Test that CLI can be parsed
        let _cli = Cli::try_parse_from(["pdm", "build", "--release"]);
    }
}
