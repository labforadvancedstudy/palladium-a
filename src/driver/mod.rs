// Compilation driver for Palladium
// "The conductor of the legendary orchestra"

use crate::codegen::CodeGenerator;
use crate::errors::{reporter::ErrorReporter, CompileError, Result};
use crate::lexer::Lexer;
use crate::linker::{link_command, OptLevel};
use crate::macros::MacroExpander;
use crate::optimizer::Optimizer;
use crate::ownership::BorrowChecker;
use crate::parser::Parser;
use crate::resolver::ModuleResolver;
use crate::typeck::TypeChecker;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

pub struct Driver {
    // Future: compilation options, session state, etc.
    use_llvm: bool,
    opt_level: OptLevel,
}

impl Driver {
    pub fn new() -> Self {
        Self {
            use_llvm: false, // Default to C backend
            // Optimized by default: the C backend emits naive C and relies on
            // gcc for the constant factor. See src/linker.rs.
            opt_level: OptLevel::default(),
        }
    }

    /// Enable LLVM backend
    pub fn with_llvm(mut self) -> Self {
        self.use_llvm = true;
        self
    }

    /// Set the optimization level handed to gcc when linking a native binary.
    pub fn with_opt_level(mut self, opt_level: OptLevel) -> Self {
        self.opt_level = opt_level;
        self
    }

    /// Compile a string of source code and return the output path
    pub fn compile_string(&self, source: &str, filename: &str) -> Result<PathBuf> {
        let total_start = Instant::now();
        println!("🔨 Compiling {}...", filename);

        // Phase 1: Lexing
        println!("📖 Lexing...");
        let lex_start = Instant::now();
        let mut lexer = Lexer::new(source);
        let tokens = lexer.collect_tokens()?;
        let lex_time = lex_start.elapsed();
        println!(
            "   Found {} tokens ({:.2}ms)",
            tokens.len(),
            lex_time.as_secs_f64() * 1000.0
        );

        // Phase 2: Parsing
        println!("🌳 Parsing...");
        let parse_start = Instant::now();
        let mut parser = Parser::new(tokens);
        let mut ast = parser.parse()?;
        let parse_time = parse_start.elapsed();
        println!(
            "   Parsed {} top-level items ({:.2}ms)",
            ast.items.len(),
            parse_time.as_secs_f64() * 1000.0
        );

        // Phase 2.3: Macro expansion
        println!("🔮 Expanding macros...");
        let macro_start = Instant::now();
        let mut macro_expander = MacroExpander::new();
        macro_expander.expand_program(&mut ast)?;
        let macro_time = macro_start.elapsed();
        println!(
            "   Macros expanded successfully! ({:.2}ms)",
            macro_time.as_secs_f64() * 1000.0
        );

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
        let type_start = Instant::now();
        let mut type_checker = TypeChecker::new();

        // Pass resolved modules to type checker
        if !resolved_modules.is_empty() {
            type_checker.set_imported_modules(resolved_modules.clone());
        }

        type_checker.check(&ast)?;
        let type_time = type_start.elapsed();
        println!(
            "   All types check out! ({:.2}ms)",
            type_time.as_secs_f64() * 1000.0
        );

        // Get generic instantiations from type checker
        let instantiations = type_checker.get_instantiations();
        if !instantiations.is_empty() {
            println!(
                "   Found {} generic function instantiations",
                instantiations.len()
            );
        }

        // Get generic struct instantiations from type checker
        let struct_instantiations = type_checker.get_struct_instantiations();
        if !struct_instantiations.is_empty() {
            println!(
                "   Found {} generic struct instantiations",
                struct_instantiations.len()
            );
        }

        // Phase 3.5: Borrow checking
        println!("🔒 Borrow checking...");
        let borrow_start = Instant::now();
        let mut borrow_checker = BorrowChecker::new();

        // Same resolver result and same guard as the type checker above. Without
        // this the borrow checker sees a single-file program: every call to an
        // imported function was rejected as "Use of uninitialized value", because
        // the callee was absent from its function table and was then looked up as
        // a variable.
        if !resolved_modules.is_empty() {
            borrow_checker.set_imported_modules(resolved_modules.clone());
        }

        // Which generic templates get monomorphized, AND WHERE EACH CAME FROM, so
        // the walk over imported bodies covers exactly the bodies codegen emits.
        //
        // Both halves were learned the hard way. Skipping every generic body — on
        // the reasoning that codegen emits only non-generic imported functions,
        // true of the direct path and false of monomorphization — let a
        // use-after-move inside an imported `fn bad<T>` compile, emit `bad__i64`,
        // link and print 7. Then keying on the NAME alone — `instantiations`
        // collapsed to a set of names — made the pass check every same-named
        // imported template including ones a local definition had displaced, and
        // an error in a body nothing emits vetoed the build. The origin map is
        // the same question asked without the lossy projection.
        borrow_checker
            .set_instantiated_generic_origins(type_checker.get_instantiated_generic_origins());

        borrow_checker.check_program(&ast)?;
        let borrow_time = borrow_start.elapsed();
        println!(
            "   Memory safety verified! ({:.2}ms)",
            borrow_time.as_secs_f64() * 1000.0
        );

        // Phase 3.6: Effect analysis
        println!("🌊 Analyzing effects...");
        let mut effect_analyzer = crate::effects::EffectAnalyzer::new();
        for item in &ast.items {
            if let crate::ast::Item::Function(func) = item {
                let effects = effect_analyzer.analyze_function(func)?;
                if !effects.is_pure() {
                    // Sorted, not the raw HashSet: two runs of the same compiler
                    // over the same source must print the same line.
                    println!(
                        "   Function '{}' has effects: {:?}",
                        func.name,
                        effects.sorted()
                    );
                }
            }
        }
        println!("   Effect analysis complete!");

        // Phase 3.7: Unsafe checking
        println!("⚠️  Checking unsafe operations...");
        let mut unsafe_checker = crate::unsafe_ops::UnsafeChecker::new();
        for item in &ast.items {
            if let crate::ast::Item::Function(func) = item {
                unsafe_checker.check_function(func)?;
            }
        }
        println!("   Unsafe operations verified!");

        // Phase 3.8: Optimization (optional but enabled by default)
        println!("🔧 Optimizing...");
        let opt_start = Instant::now();
        let mut optimizer = Optimizer::new().with_logging();
        optimizer.optimize(&mut ast)?;
        let opt_time = opt_start.elapsed();
        println!(
            "   Optimization complete ({:.2}ms)",
            opt_time.as_secs_f64() * 1000.0
        );

        // Phase 4: Code generation
        let output_path = if self.use_llvm {
            println!("⚡ Generating LLVM IR...");
            let mut llvm_gen = crate::codegen::llvm_text_backend::LLVMTextBackend::new(filename)?;
            let ir = llvm_gen.compile(&ast)?;
            let path = llvm_gen.write_output(&ir)?;
            println!("   Generated LLVM IR: {}", path.display());
            path
        } else {
            println!("⚡ Generating C code...");
            let gen_start = Instant::now();
            let mut codegen = CodeGenerator::new(filename)?;

            // Pass resolved modules to code generator
            if !resolved_modules.is_empty() {
                codegen.set_imported_modules(resolved_modules);
            }

            // Pass generic instantiations to code generator
            if !instantiations.is_empty() {
                codegen.set_generic_instantiations(instantiations);
            }

            // Pass generic struct instantiations to code generator
            if !struct_instantiations.is_empty() {
                codegen.set_generic_struct_instantiations(struct_instantiations);
            }

            codegen.compile(&ast)?;
            let output = codegen.write_output()?;
            let gen_time = gen_start.elapsed();
            println!(
                "   Code generation complete ({:.2}ms)",
                gen_time.as_secs_f64() * 1000.0
            );
            output
        };

        let total_time = total_start.elapsed();
        println!("✅ Compilation successful!");
        println!("   Output: {}", output_path.display());
        println!("   Total time: {:.2}ms", total_time.as_secs_f64() * 1000.0);

        Ok(output_path)
    }

    /// Compile a file and return the output path
    pub fn compile_file(&self, path: &Path) -> Result<PathBuf> {
        let source = fs::read_to_string(path).map_err(CompileError::IoError)?;

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        // Create error reporter for better error messages
        let reporter = ErrorReporter::new(path.to_string_lossy().to_string())
            .map_err(CompileError::IoError)?;

        match self.compile_string(&source, filename) {
            Ok(output) => Ok(output),
            Err(e) => {
                // Convert error to diagnostic and report it
                let diagnostic = e.to_diagnostic();
                reporter.report(&diagnostic);
                Err(e)
            }
        }
    }

    /// Compile and run a file
    pub fn compile_and_run(&self, path: &Path) -> Result<()> {
        // First compile to C (error reporting handled in compile_file)
        let c_path = self.compile_file(path)?;

        // Create build directory if it doesn't exist
        let build_dir = PathBuf::from("target/build");
        if !build_dir.exists() {
            fs::create_dir_all(&build_dir).map_err(CompileError::IoError)?;
        }

        // Determine output binary name
        let binary_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("a.out");
        let binary_path = build_dir.join(binary_name);

        // Compile C code with gcc
        println!("🔗 Linking with gcc ({})...", self.opt_level.flag());

        let gcc_output = link_command(&c_path, &binary_path, self.opt_level)?
            .output()
            .map_err(|e| CompileError::Generic(format!("Failed to run gcc: {}", e)))?;

        if !gcc_output.status.success() {
            let stderr = String::from_utf8_lossy(&gcc_output.stderr);
            return Err(CompileError::Generic(format!(
                "gcc compilation failed:\n{}",
                stderr
            )));
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
