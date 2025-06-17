// Alan von Palladium Compiler - Bootstrap v0.1
// "Where Legends Begin to Compile"

use std::env;
use std::process;

fn main() {
    println!(r#"
     _    __     ______    ____                      _ _           
    / \   \ \   / /  _ \  / ___|___  _ __ ___  _ __ (_) | ___ _ __ 
   / _ \   \ \ / /| |_) || |   / _ \| '_ ` _ \| '_ \| | |/ _ \ '__|
  / ___ \   \ V / |  __/ | |__| (_) | | | | | | |_) | | |  __/ |   
 /_/   \_\   \_/  |_|     \____\___/|_| |_| |_| .__/|_|_|\___|_|   
                                               |_|                  
    
    Alan von Palladium Compiler v0.1-alpha
    "Turing's Proofs Meet von Neumann's Performance"
    "#);

    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: {} <command> [options]", args[0]);
        eprintln!("Commands:");
        eprintln!("  compile <file>  - Compile a Palladium source file");
        eprintln!("  run <file>      - Compile and run a Palladium source file");
        eprintln!("  --version       - Show version information");
        eprintln!("  --help          - Show this help message");
        process::exit(1);
    }

    match args[1].as_str() {
        "compile" => {
            if args.len() < 3 {
                eprintln!("Error: Please specify a file to compile");
                process::exit(1);
            }
            
            // Parse -o option for output name
            let output_name = if args.len() >= 5 && args[3] == "-o" {
                Some(args[4].as_str())
            } else {
                None
            };
            
            println!("Compiling {}...", args[2]);
            compile_file(&args[2], output_name);
        }
        "run" => {
            if args.len() < 3 {
                eprintln!("Error: Please specify a file to run");
                process::exit(1);
            }
            println!("Compiling and running {}...", args[2]);
            compile_and_run(&args[2]);
        }
        "--version" | "-v" => {
            print_version();
        }
        "--help" | "-h" => {
            print_help();
        }
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            eprintln!("Use --help for usage information");
            process::exit(1);
        }
    }
}

fn compile_file(filename: &str, output_name: Option<&str>) {
    use std::path::Path;
    use palladium::driver::Driver;
    
    let driver = Driver::new();
    let path = Path::new(filename);
    
    match driver.compile_file(path) {
        Ok(c_path) => {
            // If output name specified, also compile to executable
            if let Some(name) = output_name {
                use std::process::Command;
                
                let build_dir = Path::new("build_output");
                let output_path = build_dir.join(name);
                
                println!("🔗 Linking with gcc...");
                let gcc_output = Command::new("gcc")
                    .arg(&c_path)
                    .arg("-o")
                    .arg(&output_path)
                    .output()
                    .expect("Failed to run gcc");
                
                if !gcc_output.status.success() {
                    let stderr = String::from_utf8_lossy(&gcc_output.stderr);
                    eprintln!("❌ gcc compilation failed:\n{}", stderr);
                    process::exit(1);
                }
                
                println!("   Created executable: {}", output_path.display());
            }
        },
        Err(e) => {
            eprintln!("❌ Compilation failed: {}", e);
            process::exit(1);
        }
    }
}

fn compile_and_run(filename: &str) {
    use std::path::Path;
    use palladium::driver::Driver;
    
    let driver = Driver::new();
    let path = Path::new(filename);
    
    match driver.compile_and_run(path) {
        Ok(()) => {},
        Err(e) => {
            eprintln!("❌ Error: {}", e);
            process::exit(1);
        }
    }
}

fn print_version() {
    println!("Alan von Palladium Compiler");
    println!("Version: 0.1-alpha");
    println!("Build: 2025-01-01");
    println!();
    println!("Features:");
    println!("  - Basic type system");
    println!("  - Function definitions");
    println!("  - LLVM backend (planned)");
    println!("  - Formal verification (planned)");
}

fn print_help() {
    println!("Alan von Palladium Compiler - The Future of Systems Programming");
    println!();
    println!("Usage: palladium <command> [options]");
    println!();
    println!("Commands:");
    println!("  compile <file>  - Compile a .pd source file");
    println!("  run <file>      - Compile and execute a .pd source file");
    println!("  --version, -v   - Display version information");
    println!("  --help, -h      - Display this help message");
    println!();
    println!("Examples:");
    println!("  palladium compile hello.pd");
    println!("  palladium run fibonacci.pd");
    println!();
    println!("For more information, visit: https://alan-von-palladium.org");
}