// Compilation driver for Palladium
// "The conductor of the legendary orchestra"

use crate::errors::{CompileError, Result};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::typeck::TypeChecker;
use crate::codegen::CodeGenerator;
use crate::resolver::ModuleResolver;
use std::path::{Path, PathBuf};
use std::fs;
use std::process::Command;
use std::collections::HashMap;

pub struct Driver {
    // Future: compilation options, session state, etc.
}

impl Driver {
    pub fn new() -> Self {
        Self {}
    }
    
    /// Compile a string of source code and return the output path
    pub fn compile_string(&self, source: &str, filename: &str) -> Result<PathBuf> {
        println!("🔨 Compiling {}...", filename);
        
        // Phase 1: Lexing
        println!("📖 Lexing...");
        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens()?;
        println!("   Found {} tokens", tokens.len());
        
        // Phase 2: Parsing
        println!("🌳 Parsing...");
        let mut parser = Parser::new(tokens);
        let ast = parser.parse()?;
        println!("   Parsed {} top-level items", ast.items.len());
        
        // Phase 2.5: Module resolution
        let resolved_modules = if !ast.imports.is_empty() {
            println!("📦 Resolving modules...");
            let mut resolver = ModuleResolver::new();
            let modules = resolver.resolve_program(&ast)?;
            println!("   Resolved {} modules", modules.len());
            modules
        } else {
            HashMap::new()
        };
        
        // Phase 3: Type checking
        println!("🔍 Type checking...");
        let mut type_checker = TypeChecker::new();
        
        // Pass resolved modules to type checker
        if !resolved_modules.is_empty() {
            type_checker.set_imported_modules(resolved_modules.clone());
        }
        
        type_checker.check(&ast)?;
        println!("   All types check out!");
        
        // Get generic instantiations from type checker
        let instantiations = type_checker.get_instantiations();
        if !instantiations.is_empty() {
            println!("   Found {} generic instantiations", instantiations.len());
        }
        
        // Phase 4: Code generation
        println!("⚡ Generating code...");
        let mut codegen = CodeGenerator::new(filename)?;
        
        // Pass resolved modules to code generator
        if !resolved_modules.is_empty() {
            codegen.set_imported_modules(resolved_modules);
        }
        
        // Pass generic instantiations to code generator
        if !instantiations.is_empty() {
            codegen.set_generic_instantiations(instantiations);
        }
        
        codegen.compile(&ast)?;
        let output_path = codegen.write_output()?;
        
        println!("✅ Compilation successful!");
        println!("   Output: {}", output_path.display());
        
        Ok(output_path)
    }
    
    /// Compile a file and return the output path
    pub fn compile_file(&self, path: &Path) -> Result<PathBuf> {
        let source = fs::read_to_string(path)
            .map_err(|e| CompileError::IoError(e))?;
        
        let filename = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
            
        self.compile_string(&source, filename)
    }
    
    /// Compile and run a file
    pub fn compile_and_run(&self, path: &Path) -> Result<()> {
        // First compile to C
        let c_path = self.compile_file(path)?;
        
        // Create build_output directory if it doesn't exist
        let build_dir = PathBuf::from("build_output");
        if !build_dir.exists() {
            fs::create_dir_all(&build_dir)
                .map_err(|e| CompileError::IoError(e))?;
        }
        
        // Determine output binary name
        let binary_name = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("a.out");
        let binary_path = build_dir.join(binary_name);
        
        // Compile C code with gcc
        println!("🔗 Linking with gcc...");
        let gcc_output = Command::new("gcc")
            .arg(&c_path)
            .arg("-o")
            .arg(&binary_path)
            .output()
            .map_err(|e| CompileError::Generic(format!("Failed to run gcc: {}", e)))?;
        
        if !gcc_output.status.success() {
            let stderr = String::from_utf8_lossy(&gcc_output.stderr);
            return Err(CompileError::Generic(format!("gcc compilation failed:\n{}", stderr)));
        }
        
        println!("   Created executable: {}", binary_path.display());
        
        // Run the compiled program
        println!("🚀 Running program...");
        println!("─────────────────────────────────────");
        
        let run_output = Command::new(&binary_path)
            .output()
            .map_err(|e| CompileError::Generic(format!("Failed to run program: {}", e)))?;
        
        // Print stdout
        if !run_output.stdout.is_empty() {
            print!("{}", String::from_utf8_lossy(&run_output.stdout));
        }
        
        // Print stderr if any
        if !run_output.stderr.is_empty() {
            eprint!("{}", String::from_utf8_lossy(&run_output.stderr));
        }
        
        println!("─────────────────────────────────────");
        
        if !run_output.status.success() {
            let exit_code = run_output.status.code().unwrap_or(-1);
            println!("⚠️  Program exited with code: {}", exit_code);
        } else {
            println!("✅ Program completed successfully");
        }
        
        // Clean up intermediate files (optional)
        // You might want to keep these for debugging
        // fs::remove_file(&c_path).ok();
        // fs::remove_file(&binary_path).ok();
        
        Ok(())
    }
}

impl Default for Driver {
    fn default() -> Self {
        Self::new()
    }
}