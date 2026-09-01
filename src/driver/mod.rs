// Compilation driver for Palladium
// "The conductor of the legendary orchestra"

use crate::codegen::CodeGenerator;
use crate::errors::{reporter, reporter::ErrorReporter, CompileError, Result};
use crate::lexer::Lexer;
use crate::linker::{self, LinkError, OptLevel};
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

    /// Compile a file and return the output path.
    ///
    /// EVERY error out of here has already been reported, exactly once, at the
    /// single choke point (`errors::reporter::emit_primary_header`). Callers must
    /// not print it again — that duplicate print is what GI-12 removed.
    pub fn compile_file(&self, path: &Path) -> Result<PathBuf> {
        // THE TWO REPORTER-LESS PATHS. Both of these refuse BEFORE a reporter can
        // exist, so until GI-12 their only diagnostic was `main`'s duplicate
        // print — and deleting that duplicate without routing them here would
        // have made an unreadable source file exit nonzero in silence. They have
        // no source to quote (that is precisely what failed), so they render
        // through the header-only path rather than the snippet one.
        let source = fs::read_to_string(path).map_err(|e| {
            let err = CompileError::IoError(e);
            reporter::report_without_source(&err.to_diagnostic());
            err
        })?;

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        // Create error reporter for better error messages
        let reporter = ErrorReporter::new(path.to_string_lossy().to_string()).map_err(|e| {
            let err = CompileError::IoError(e);
            reporter::report_without_source(&err.to_diagnostic());
            err
        })?;

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

    /// Compile and run a file, reporting WHICH of the four ways it failed.
    pub fn compile_and_run_reporting(&self, path: &Path) -> std::result::Result<(), RunOutcome> {
        // First compile to C (error reporting handled in compile_file)
        let c_path = self.compile_file(path).map_err(RunOutcome::Compile)?;

        // Create build directory if it doesn't exist
        let build_dir = PathBuf::from("target/build");
        if !build_dir.exists() {
            fs::create_dir_all(&build_dir)
                .map_err(|e| RunOutcome::Compile(CompileError::IoError(e)))?;
        }

        // Determine output binary name
        let binary_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("a.out");
        let binary_path = build_dir.join(binary_name);

        // Compile C code with gcc
        println!("🔗 Linking with gcc ({})...", self.opt_level.flag());

        // The shared policy, not a private copy of it. This call site used to
        // hold its own `if !status.success()`, which is how `pdc run` came to
        // disagree with `pdc compile` about the same source.
        let notes =
            linker::link(&c_path, &binary_path, self.opt_level).map_err(RunOutcome::Link)?;
        linker::report_notes(&notes);

        println!("   Created executable: {}", binary_path.display());

        // Run the compiled program
        println!("🚀 Running program...");
        println!("─────────────────────────────────────");

        let run_output = Command::new(&binary_path).output().map_err(|e| {
            RunOutcome::Compile(CompileError::Generic(format!(
                "Failed to run program: {}",
                e
            )))
        })?;

        // Print stdout
        if !run_output.stdout.is_empty() {
            print!("{}", String::from_utf8_lossy(&run_output.stdout));
        }

        // Print stderr if any
        if !run_output.stderr.is_empty() {
            eprint!("{}", String::from_utf8_lossy(&run_output.stderr));
        }

        println!("─────────────────────────────────────");

        // A LAUNCHER THAT REPORTS SUCCESS FOR A SEGFAULT IS THE SAME LIE AS THE
        // DISCARD THIS BRANCH CAME TO FIX. This used to `println!` the child's
        // exit code and then `Ok(())`, so `pdc run` exited 0 for a program that
        // died — including, for one release of this branch, the very program
        // `pdc compile` had just refused to build.
        if !run_output.status.success() {
            println!("⚠️  Program {}", describe_child_status(&run_output.status));
            return Err(RunOutcome::from_child(&run_output.status));
        }
        println!("✅ Program completed successfully");

        // Clean up intermediate files (optional)
        // You might want to keep these for debugging
        // fs::remove_file(&c_path).ok();
        // fs::remove_file(&binary_path).ok();

        Ok(())
    }

    /// Compile and run a file.
    ///
    /// THE SPLIT `compile_and_run_reporting` CLOSES, and the reason this one is
    /// now a wrapper. For one release of this branch, `pdc compile` refused the
    /// type-confusion program (exit 4, no binary) while `pdc run` on the SAME
    /// source built it, ran it, printed `Program exited with code: -1`, and
    /// exited 0 — two adjacent commands, opposite verdicts, and the one that
    /// actually executed the miscompiled binary was the one reporting success.
    ///
    /// This signature is kept for callers that only have a `CompileError` to
    /// return (`tests/examples_test.rs`, `tests/integration_test.rs`). `pdc run`
    /// does NOT use it: collapsing a link verdict and a dead child into one
    /// error type throws away exactly the distinction this module was fixed to
    /// keep.
    ///
    /// (Defined after the function it delegates to, rather than before it,
    /// because `docs/citation-pins.tsv` fingerprints a line of
    /// `compile_and_run_reporting`'s body by LINE NUMBER, and this branch may
    /// not edit that file. See the branch report — the pin mechanism, not the
    /// code, chose this order.)
    pub fn compile_and_run(&self, path: &Path) -> Result<()> {
        self.compile_and_run_reporting(path)
            .map_err(RunOutcome::into_compile_error)
    }
}

/// The signal that killed a process, where the platform reports one.
fn death_signal(status: &std::process::ExitStatus) -> Option<i32> {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        status.signal()
    }
    #[cfg(not(unix))]
    {
        let _ = status;
        None
    }
}

/// How the child ended, for the human line above the verdict.
///
/// `-1` was printed here before, for every abnormal end, and said nothing.
pub fn describe_child_status(status: &std::process::ExitStatus) -> String {
    match (status.code(), death_signal(status)) {
        (Some(c), _) => format!("exited with code {}", c),
        (None, Some(s)) => format!("was killed by signal {}", s),
        (None, None) => "terminated abnormally".to_string(),
    }
}

/// Why `pdc run` did not end with a program that ran successfully.
///
/// Four causes with three different owners: the front end refused the source,
/// the link stage refused (or could not reach) the C, or the program itself
/// ran and failed. Flattening them is how a launcher comes to report success
/// for a segfault.
#[derive(Debug)]
pub enum RunOutcome {
    /// The front end, or any non-link step, refused.
    Compile(CompileError),
    /// The link stage. Carries the full [`LinkError`] so the exit code survives.
    Link(LinkError),
    /// The program was built, started, and failed.
    Child {
        /// `None` when the child was killed rather than exiting.
        code: Option<i32>,
        /// The signal that killed it, where the platform reports one.
        signal: Option<i32>,
    },
}

impl RunOutcome {
    /// Build the child verdict from a real `ExitStatus`.
    ///
    /// One constructor so that `pdc run` and `pdm run` cannot disagree about
    /// what a dead child is — they used to, because one kept the status and the
    /// other turned it into a sentence.
    pub fn from_child(status: &std::process::ExitStatus) -> Self {
        RunOutcome::Child {
            code: status.code(),
            signal: death_signal(status),
        }
    }

    /// The process exit code `pdc run` reports.
    ///
    /// THE BOUNDARY, stated because it is the one thing here a gate could get
    /// wrong: 3/4/5/6 mean the program NEVER STARTED, and they come from
    /// [`LinkError::exit_code`]. Once the program starts, `pdc run` is a
    /// launcher and the status belongs to the program — so a program that
    /// exits 4 makes `pdc run` exit 4 too, and the two are not distinguishable
    /// from the outside. A gate that must tell them apart has to use
    /// `pdc compile`, which never runs anything; `scripts/conformance.sh`
    /// already does.
    pub fn exit_code(&self) -> i32 {
        match self {
            RunOutcome::Compile(_) => 1,
            RunOutcome::Link(e) => e.exit_code(),
            // A plain nonzero exit is passed through unchanged; a signalled
            // child uses the shell's 128+N, which is what every other launcher
            // on this platform reports and what `$?` already means to a script.
            RunOutcome::Child { code, signal } => code.unwrap_or_else(|| 128 + signal.unwrap_or(0)),
        }
    }

    /// Collapse into the historical error type, for callers that only have one.
    pub fn into_compile_error(self) -> CompileError {
        match self {
            RunOutcome::Compile(e) => e,
            RunOutcome::Link(e) => CompileError::Generic(e.to_string()),
            RunOutcome::Child { code, signal } => CompileError::Generic(match (code, signal) {
                (Some(c), _) => format!("program exited with code {}", c),
                (None, Some(s)) => format!("program was killed by signal {}", s),
                (None, None) => "program terminated abnormally".to_string(),
            }),
        }
    }
}

impl Default for Driver {
    fn default() -> Self {
        Self::new()
    }
}
