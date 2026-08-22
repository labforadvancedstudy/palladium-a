//! M3: calling a function that came from an `import`.
//!
//! The defect: **no program that calls an imported function compiled.**
//!
//! ```text
//! lib2.pd:  pub fn helper() -> i64 { return 5; }
//! call.pd:  import lib2;
//!           fn main() { print_int(helper()); }
//! -> error: Use of uninitialized value: helper
//! ```
//!
//! The type checker was handed the resolver's output (`src/driver/mod.rs:104-107`,
//! `type_checker.set_imported_modules(...)`), so `helper` type-checked. The borrow
//! checker was constructed with `BorrowChecker::new()` and handed the *pre-resolution*
//! AST (`src/driver/mod.rs:137-138`) while `resolved_modules` sat live and unused in
//! the same scope. Its function table was seeded from `crate::builtins::BUILTINS` and
//! nothing else (`src/ownership/borrow_checker.rs:114-118`), and its first pass walks
//! `program.items` only — `Program.imports` (`src/ast/mod.rs:9`) is never read, and
//! `Item` (`src/ast/mod.rs:24-32`) has no `Import` variant, so no walk over items could
//! have reached one. `helper()` therefore fell out of the function table at
//! `Expr::Ident` (`:502`), was looked up as a *value*, was not found, and died at
//! `:527` as `UseOfUninitializedValue`.
//!
//! The pass was not wrong; it was structurally single-file. These tests drive the real
//! `pdc` binary over two real files in a scratch directory, because the resolver reads
//! the filesystem and the claim under test is about a two-file program.

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

/// Compile `main.pd` in a scratch directory that also holds the given modules,
/// then run the produced executable. Returns (compiler output, run result).
///
/// `current_dir` is the scratch directory because the module resolver searches
/// `.` (`src/resolver/mod.rs:37`) and the driver writes `build_output/` relative
/// to the working directory — so both the input side and the output side of this
/// test belong to a directory it owns.
fn compile_and_run(modules: &[(&str, &str)], main_src: &str) -> (bool, String, Option<String>) {
    let dir = TempDir::new().unwrap();
    for (name, src) in modules {
        let path = dir.path().join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, src).unwrap();
    }
    fs::write(dir.path().join("main.pd"), main_src).unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_pdc"))
        .args(["compile", "main.pd", "-o", "prog"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run pdc");

    let compiler_output = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let compiled = out.status.success();

    let exe = dir.path().join("build_output").join("prog");
    let stdout = if compiled && Path::new(&exe).exists() {
        let run = Command::new(&exe)
            .current_dir(dir.path())
            .output()
            .expect("failed to run the compiled program");
        Some(String::from_utf8_lossy(&run.stdout).to_string())
    } else {
        None
    };

    (compiled, compiler_output, stdout)
}

/// The measured defect, verbatim. Fails on `main` with
/// `Use of uninitialized value: helper`.
#[test]
fn test_calling_an_imported_function_compiles() {
    let (compiled, output, _) = compile_and_run(
        &[("lib2.pd", "pub fn helper() -> i64 { return 5; }\n")],
        "import lib2;\n\nfn main() {\n    print_int(helper());\n}\n",
    );
    assert!(
        compiled,
        "a call to an imported function was rejected; compiler said:\n{}",
        output
    );
    assert!(
        !output.contains("Use of uninitialized value"),
        "the borrow checker still treats the imported callee as a variable:\n{}",
        output
    );
}

/// Compiling is not the claim; *running correctly* is. A borrow-check fix that
/// let the program through while codegen emitted no body for `helper` would
/// satisfy the test above and still hand the user garbage — which is exactly the
/// failure mode this repo has been burned by (D3 tail return).
#[test]
fn test_calling_an_imported_function_prints_its_result() {
    let (compiled, output, stdout) = compile_and_run(
        &[("lib2.pd", "pub fn helper() -> i64 { return 5; }\n")],
        "import lib2;\n\nfn main() {\n    print_int(helper());\n}\n",
    );
    assert!(compiled, "compilation failed:\n{}", output);
    assert_eq!(
        stdout.as_deref(),
        Some("5\n"),
        "the imported function did not run; compiler said:\n{}",
        output
    );
}
