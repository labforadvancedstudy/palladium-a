// Alan von Palladium Compiler - Bootstrap v0.1
// "Where Legends Begin to Compile"

use clap::Parser;
use palladium::{driver::Driver, package::PackageManager};
use std::path::Path;
use std::process;

mod cli;
use cli::{BootstrapCommands, Cli, Commands};

fn main() {
    print_banner();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Compile {
            file,
            output,
            llvm,
            optimize,
        } => compile_file(&file, output.as_deref(), llvm, optimize),
        Commands::Run { file, llvm, args } => run_file(&file, llvm, args),
        Commands::New { name, path, lib } => new_package(&name, path.as_deref(), lib),
        Commands::Init { name, lib } => init_package(name.as_deref(), lib),
        Commands::Build { release, llvm } => build_package(release, llvm),
        Commands::PackageRun { release, args } => run_package(release, args),
        Commands::Add {
            name,
            version,
            dev,
            build,
        } => add_dependency(&name, version.as_deref(), dev, build),
        Commands::Install => install_dependencies(),
        Commands::Update { package } => update_dependencies(package.as_deref()),
        Commands::Check { all } => check_package(all),
        Commands::Test {
            pattern,
            release,
            nocapture,
        } => run_tests(pattern.as_deref(), release, nocapture),
        Commands::Fmt { check, all } => format_code(check, all),
        Commands::Lint { fix, all } => lint_code(fix, all),
        Commands::Doc { open, private } => generate_docs(open, private),
        Commands::Clean { target, cache } => clean_artifacts(target, cache),
        Commands::Bootstrap { command } => handle_bootstrap_command(command),
    };

    if let Err(e) = result {
        eprintln!("\x1b[1;31merror:\x1b[0m {}", e);
        process::exit(1);
    }
}

fn print_banner() {
    println!(
        r#"
     _    __     ______    ____                      _ _           
    / \   \ \   / /  _ \  / ___|___  _ __ ___  _ __ (_) | ___ _ __ 
   / _ \   \ \ / /| |_) || |   / _ \| '_ ` _ \| '_ \| | |/ _ \ '__|
  / ___ \   \ V / |  __/ | |__| (_) | | | | | | |_) | | |  __/ |   
 /_/   \_\   \_/  |_|     \____\___/|_| |_| |_| .__/|_|_|\___|_|   
                                               |_|                  
    
    Alan von Palladium Compiler v0.1-alpha
    "Turing's Proofs Meet von Neumann's Performance"
    "#
    );
}

fn compile_file(
    path: &Path,
    output: Option<&str>,
    llvm: bool,
    _optimize: bool,
) -> Result<(), String> {
    println!("Compiling {}...", path.display());

    let mut driver = Driver::new();
    if llvm {
        driver = driver.with_llvm();
    }

    match driver.compile_file(path) {
        Ok(c_path) => {
            // If output name specified, also compile to executable
            if let Some(name) = output {
                use std::process::Command;

                let build_dir = Path::new("build_output");
                let output_path = build_dir.join(name);

                println!("🔗 Linking with gcc...");
                
                // Get the runtime library path
                let runtime_path = Path::new("runtime/palladium_runtime.c");
                
                let gcc_output = Command::new("gcc")
                    .arg(&c_path)
                    .arg(runtime_path)
                    .arg("-o")
                    .arg(&output_path)
                    .output()
                    .map_err(|e| format!("Failed to run gcc: {}", e))?;

                if !gcc_output.status.success() {
                    let stderr = String::from_utf8_lossy(&gcc_output.stderr);
                    return Err(format!("gcc compilation failed:\n{}", stderr));
                }

                println!("   Created executable: {}", output_path.display());
            }
            Ok(())
        }
        Err(e) => Err(e.to_string()),
    }
}

fn run_file(path: &Path, llvm: bool, _args: Vec<String>) -> Result<(), String> {
    println!("Compiling and running {}...", path.display());

    let mut driver = Driver::new();
    if llvm {
        driver = driver.with_llvm();
    }

    driver.compile_and_run(path).map_err(|e| e.to_string())
}

fn new_package(name: &str, path: Option<&Path>, _lib: bool) -> Result<(), String> {
    let target_path = if let Some(p) = path {
        p.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| format!("Failed to get current directory: {}", e))?
            .join(name)
    };

    // Create the directory
    std::fs::create_dir_all(&target_path)
        .map_err(|e| format!("Failed to create directory: {}", e))?;

    PackageManager::init(name, &target_path).map_err(|e| e.to_string())
}

fn init_package(name: Option<&str>, _lib: bool) -> Result<(), String> {
    let current_dir =
        std::env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;

    let package_name = if let Some(n) = name {
        n.to_string()
    } else {
        current_dir
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("Could not determine package name from directory")?
            .to_string()
    };

    PackageManager::init(&package_name, &current_dir).map_err(|e| e.to_string())
}

fn build_package(release: bool, _llvm: bool) -> Result<(), String> {
    let pm = PackageManager::new().map_err(|e| e.to_string())?;
    pm.build(release).map_err(|e| e.to_string())
}

fn run_package(release: bool, args: Vec<String>) -> Result<(), String> {
    let pm = PackageManager::new().map_err(|e| e.to_string())?;
    pm.run(args, release).map_err(|e| e.to_string())
}

fn add_dependency(
    name: &str,
    version: Option<&str>,
    dev: bool,
    _build: bool,
) -> Result<(), String> {
    let mut pm = PackageManager::new().map_err(|e| e.to_string())?;
    let ver = version.unwrap_or("*");
    pm.add_dependency(name, ver, dev).map_err(|e| e.to_string())
}

fn install_dependencies() -> Result<(), String> {
    let mut pm = PackageManager::new().map_err(|e| e.to_string())?;
    pm.install().map_err(|e| e.to_string())
}

fn update_dependencies(package: Option<&str>) -> Result<(), String> {
    let mut pm = PackageManager::new().map_err(|e| e.to_string())?;
    pm.update(package).map_err(|e| e.to_string())
}

fn check_package(_all: bool) -> Result<(), String> {
    eprintln!("Package check not yet implemented");
    Ok(())
}

fn run_tests(_pattern: Option<&str>, _release: bool, _nocapture: bool) -> Result<(), String> {
    eprintln!("Test runner not yet implemented");
    Ok(())
}

fn format_code(_check: bool, _all: bool) -> Result<(), String> {
    eprintln!("Code formatter not yet implemented");
    Ok(())
}

fn lint_code(_fix: bool, _all: bool) -> Result<(), String> {
    eprintln!("Linter not yet implemented");
    Ok(())
}

fn generate_docs(_open: bool, _private: bool) -> Result<(), String> {
    eprintln!("Documentation generator not yet implemented");
    Ok(())
}

fn clean_artifacts(target: bool, cache: bool) -> Result<(), String> {
    if target && Path::new("target").exists() {
        std::fs::remove_dir_all("target")
            .map_err(|e| format!("Failed to remove target directory: {}", e))?;
        println!("✅ Removed target directory");
    }

    if cache {
        let home_dir = dirs::home_dir().ok_or("Could not find home directory")?;
        let cache_dir = home_dir.join(".palladium").join("cache");

        if cache_dir.exists() {
            std::fs::remove_dir_all(&cache_dir)
                .map_err(|e| format!("Failed to remove cache directory: {}", e))?;
            println!("✅ Removed cache directory");
        }
    }

    if !target && !cache {
        // Default: clean build_output
        if Path::new("build_output").exists() {
            std::fs::remove_dir_all("build_output")
                .map_err(|e| format!("Failed to remove build_output directory: {}", e))?;
            println!("✅ Removed build_output directory");
        }
    }

    Ok(())
}

fn handle_bootstrap_command(command: BootstrapCommands) -> Result<(), String> {
    use palladium::bootstrap::{self_hosting_test, validate_bootstrap, BootstrapCompiler};

    match command {
        BootstrapCommands::Build => {
            println!("Building bootstrap compiler...");
            let _compiler = BootstrapCompiler::new().map_err(|e| e.to_string())?;
            println!("✅ Bootstrap compiler ready!");
            Ok(())
        }
        BootstrapCommands::SelfHost => {
            println!("Testing self-hosting capability...");
            self_hosting_test().map_err(|e| e.to_string())
        }
        BootstrapCommands::Validate { file } => {
            println!(
                "Validating {} against bootstrap compiler...",
                file.display()
            );
            validate_bootstrap(&file).map_err(|e| e.to_string())
        }
        BootstrapCommands::Compile { file } => {
            println!("Compiling {} with bootstrap compiler...", file.display());
            let compiler = BootstrapCompiler::new().map_err(|e| e.to_string())?;
            compiler.compile(&file).map_err(|e| e.to_string())
        }
    }
}
