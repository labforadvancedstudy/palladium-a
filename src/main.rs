// Alan von Palladium Compiler - Bootstrap v0.1
// "Where Legends Begin to Compile"

use clap::Parser;
use palladium::errors::reporter::emit_primary_header;
use palladium::errors::DiagnosticLevel;
use palladium::{driver::Driver, linker::OptLevel, package::PackageManager, runtime_paths};
use std::path::Path;
use std::process;

mod cli;
use cli::{BootstrapCommands, Cli, Commands};

fn main() {
    let cli = Cli::parse();

    // Machine-readable and banner-free: this is what a package build greps.
    if cli.print_runtime {
        match runtime_paths::runtime_dir() {
            Ok(dir) => {
                println!("{}", dir.display());
                process::exit(0);
            }
            Err(e) => {
                emit_primary_header(DiagnosticLevel::Error, None, &e.to_string());
                process::exit(1);
            }
        }
    }

    // Before the banner, like `--print-runtime`: a consumer of this list parses
    // stdout, and the banner is not parseable.
    //
    // THE TOMBSTONES ARE PRINTED TOO, and that is the point of listing codes from
    // the binary at all: the registry cannot be its own witness that a retired
    // number stayed retired — a commit that re-points a code can edit the
    // registry row in the same breath, and then the file agrees with itself
    // about something false. Two independent authorities have to agree.
    if cli.dump_diagnostic_codes {
        for code in palladium::errors::DiagnosticCode::ALL {
            println!("{}\tactive\t{}", code, code.symbolic_name());
        }
        for (number, condition) in palladium::errors::DiagnosticCode::TOMBSTONES {
            println!("PD{:04}\ttombstone\t{}", number, condition);
        }
        process::exit(0);
    }

    print_banner();

    let Some(command) = cli.command else {
        // Unreachable in practice: clap's arg_required_else_help covers the
        // no-argument case before we get here.
        emit_primary_header(
            DiagnosticLevel::Error,
            None,
            "no command given (try: pdc --help)",
        );
        process::exit(2);
    };

    let result = match command {
        Commands::Compile {
            file,
            output,
            llvm,
            optimize,
            no_opt,
        } => compile_file(
            &file,
            output.as_deref(),
            llvm,
            OptLevel::from_flags(optimize, no_opt),
        ),
        Commands::Run {
            file,
            llvm,
            optimize,
            no_opt,
            args,
        } => run_file(&file, llvm, OptLevel::from_flags(optimize, no_opt), args),
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

    // THIS IS NOT THE COMPILER'S ERROR PATH ANY MORE. It used to print a SECOND
    // primary header for every refusal — `CompileError`'s `Display`, whose
    // wording differs from the reporter's `to_diagnostic()` rendering — so every
    // reject fixture in the corpus emitted two `error:` lines saying the same
    // refusal in different words, and 4 manifest rows had come to pin wording
    // only this line produced. `compile_file`/`run_file` below now leave through
    // their own already-reported exits; what survives here is the
    // package-management commands, which have no diagnostic path of their own.
    // Their header is bare because they carry no code: NO_CODE is a state, not a
    // failure (GI-12 D1).
    if let Err(e) = result {
        emit_primary_header(DiagnosticLevel::Error, None, &e);
        process::exit(1);
    }
}

fn print_banner() {
    // The version comes from Cargo.toml, like `--version`. This line used to
    // spell it out by hand as v0.1-alpha — a third copy of the version,
    // printed on every single compile, two releases stale.
    // tests/version_matches_cargo_toml.rs fails if a literal comes back.
    println!(
        r#"
     _    __     ______    ____                      _ _
    / \   \ \   / /  _ \  / ___|___  _ __ ___  _ __ (_) | ___ _ __
   / _ \   \ \ / /| |_) || |   / _ \| '_ ` _ \| '_ \| | |/ _ \ '__|
  / ___ \   \ V / |  __/ | |__| (_) | | | | | | |_) | | |  __/ |
 /_/   \_\   \_/  |_|     \____\___/|_| |_| |_| .__/|_|_|\___|_|
                                               |_|

    Alan von Palladium Compiler v{}
    "Turing's Proofs Meet von Neumann's Performance"
    "#,
        env!("CARGO_PKG_VERSION")
    );
}

fn compile_file(
    path: &Path,
    output: Option<&str>,
    llvm: bool,
    opt: OptLevel,
) -> Result<(), String> {
    println!("Compiling {}...", path.display());

    let mut driver = Driver::new().with_opt_level(opt);
    if llvm {
        driver = driver.with_llvm();
    }

    match driver.compile_file(path) {
        Ok(c_path) => {
            // If output name specified, also compile to executable
            if let Some(name) = output {
                let build_dir = Path::new("build_output");
                let output_path = build_dir.join(name);

                println!("🔗 Linking with gcc ({})...", opt.flag());

                // One `if` here answered three questions with one string: stderr
                // was read only when the status was nonzero (so warnings were
                // discarded — the segfaulting program in `src/linker.rs`'s module
                // comment compiled clean right here), and every nonzero status
                // became `gcc compilation failed` (so a killed gcc read as a
                // rejection). `linker::link` separates them; `report_link` says
                // WHICH through the exit code, which no fixture text can forge.
                let n = palladium::linker::link(&c_path, &output_path, opt);
                palladium::linker::report_notes(&n.unwrap_or_else(|e| report_link(e)));

                println!("   Created executable: {}", output_path.display());
            }
            Ok(())
        }
        // ALREADY REPORTED, AT THE CHOKE POINT. Returning the error here would
        // hand `main` a string it would print as a second primary header, which
        // is the duplicate GI-12 R1 deletes. Exiting directly is the shape
        // `report_link`/`report_run` below already use for the same reason: the
        // diagnostic is out, only the status is left to say. The status is
        // unchanged (1), which is what the old path produced.
        Err(_) => process::exit(1),
    }
}

fn run_file(path: &Path, llvm: bool, opt: OptLevel, _args: Vec<String>) -> Result<(), String> {
    println!("Compiling and running {}...", path.display());

    let mut driver = Driver::new().with_opt_level(opt);
    if llvm {
        driver = driver.with_llvm();
    }

    // `compile_and_run_reporting`, not `compile_and_run`: the latter collapses
    // a link verdict and a dead child into one `CompileError`, and this is the
    // command that actually EXECUTES what the link stage let through. See
    // `report_run` at the end of this file.
    driver
        .compile_and_run_reporting(path)
        .unwrap_or_else(|o| report_run(o));
    Ok(())
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
    // `run_reporting`, for the reason `run_file` uses it: this command runs the
    // program, and the outer handler exits 1 for every error it is given.
    pm.run_reporting(args, release)
        .unwrap_or_else(|o| report_run(o));
    Ok(())
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

/// Report a link-stage failure and leave, with the exit code that says WHICH.
///
/// WHY THIS DOES NOT GO THROUGH `main`'s ERROR PATH. That path takes a `String`
/// and always exits 1, which is the flattening this change exists to undo: a
/// gcc that rejected our C, a gcc that was killed, and a gcc that warned about
/// C we should never have emitted would arrive as one status and three
/// sentences, and a shell gate would have to guess by grepping the sentences.
///
/// It cannot grep them. gcc echoes the generated C, the generated C carries the
/// fixture's identifiers, so any marker word this compiler prints is a word a
/// fixture can arrange to have printed — measured in this repo, where a fixture
/// containing `Linking` was classified as a link failure by exactly that grep.
/// The exit code has no route from fixture text, so that is where the
/// distinction goes. The numbers and their meanings are
/// `palladium::linker::EXIT_BACKEND_REJECT` / `EXIT_BACKEND_ILL_TYPED` /
/// `EXIT_TOOLCHAIN` / `EXIT_GCC_UNEXPLAINED`; the human sentence still goes to
/// stderr, unchanged in wording from before for the rejection case.
fn report_link(e: palladium::linker::LinkError) -> ! {
    // Through the single choke point, bare: a link failure is gcc's verdict, not
    // a language rule this compiler enforces, so it has no PD code to carry.
    emit_primary_header(DiagnosticLevel::Error, None, &e.to_string());
    process::exit(e.exit_code());
}

/// Report why `pdc run` did not end in a successful program, and leave.
///
/// Same reasoning as `report_link`, one layer out. The addition is the CHILD:
/// this used to print `Program exited with code: -1` and then return `Ok(())`,
/// so `pdc run` exited 0 for a program that segfaulted — and for one release of
/// this branch, for the exact program `pdc compile` had just refused to build.
fn report_run(o: palladium::driver::RunOutcome) -> ! {
    use palladium::driver::RunOutcome;
    let code = o.exit_code();
    match o {
        // A child that ran and failed has already printed its own output and
        // the driver's `⚠️` line. Repeating it as a compiler `error:` would say
        // the COMPILATION failed, which it did not — the program did.
        RunOutcome::Child { .. } => {}
        // A COMPILE refusal has already been reported by the driver, at the
        // choke point, with the reporter's wording and its source snippet. This
        // arm used to print it a second time in `Display`'s wording — the same
        // duplicate `main`'s handler produced, one command along.
        RunOutcome::Compile(_) => {}
        other => emit_primary_header(
            DiagnosticLevel::Error,
            None,
            &other.into_compile_error().to_string(),
        ),
    }
    process::exit(code);
}
