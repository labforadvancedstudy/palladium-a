//! D5: `?` and `.await` are rejected, and the suggested workarounds compile.
//!
//! The unit tests in `src/typeck` and `src/codegen` drive one phase each, so
//! they cannot show that a *real* compilation reaches the refusal — phase
//! order, macro expansion, the optimizer and backend selection are all
//! invisible to them. These tests run the whole driver.
//!
//! They also do the thing a diagnostic normally gets away with not doing:
//! compile and run the workaround the message suggests. A suggestion that has
//! never been executed is a claim, and this milestone is about the compiler not
//! making claims it cannot back.

use palladium::linker::{link_command, OptLevel};
use palladium::Driver;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Run the full driver over `source`. Returns the error message on failure.
fn compile(source: &str, name: &str, llvm: bool) -> Result<std::path::PathBuf, String> {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    fs::write(&src, source).unwrap();

    let driver = if llvm {
        Driver::new().with_llvm()
    } else {
        Driver::new()
    };
    driver.compile_file(&src).map_err(|e| e.to_string())
}

/// Compile, link against the real runtime, run, and return stdout.
///
/// Deliberately uses `link_command` rather than a bare `cc`: it resolves
/// `palladium_runtime.c` and the prelude header, which is what an actual
/// `pdc compile` does. A workaround that only links without the runtime is not
/// a workaround anyone can use.
fn compile_and_run(source: &str, name: &str) -> Result<String, String> {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    let exe = dir.path().join(name);
    fs::write(&src, source).unwrap();

    let c_file = Driver::new()
        .compile_file(&src)
        .map_err(|e| format!("compilation failed: {}", e))?;

    let out = link_command(&c_file, &exe, OptLevel::Default)
        .map_err(|e| format!("link_command: {}", e))?
        .output()
        .map_err(|e| format!("gcc: {}", e))?;
    if !out.status.success() {
        return Err(format!(
            "gcc rejected the C: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }

    let run = Command::new(&exe)
        .output()
        .map_err(|e| format!("run: {}", e))?;
    if !run.status.success() {
        return Err(format!("program failed: {}", String::from_utf8_lossy(&run.stderr)));
    }
    Ok(String::from_utf8_lossy(&run.stdout).to_string())
}

// ---------------------------------------------------------------------------
// The refusal, through the real pipeline
// ---------------------------------------------------------------------------

/// The measured repro. It declares a `Result` enum and a function returning it,
/// which is how it satisfies the type rules that reject the naive misuse.
/// Before D5 this produced C containing `struct Result __question_result_1 =
/// might_fail(x);` and gcc answered `variable has incomplete type`.
const QUESTION_REPRO: &str = r#"
enum Result<T, E> {
    Ok(T),
    Err(E),
}

fn might_fail(x: i64) -> Result<i64, i64> {
    return might_fail(x);
}

fn helper(x: i64) -> Result<i64, i64> {
    let v: i64 = might_fail(x)?;
    print_int(v);
    return might_fail(v);
}

fn main() {
    helper(3);
}
"#;

/// A plain function *declared* `-> Future<i64>`. A call to a real `async fn` is
/// typed as its bare return type, so awaiting one never type checked; this is
/// the only shape that reached the backend, where it emitted
/// `while (!f.poll(&f)) {}` on a `long long`.
const AWAIT_REPRO: &str = r#"
fn work(x: i64) -> Future<i64> {
    return work(x);
}

fn main() {
    let v: i64 = work(3).await;
    print_int(v);
}
"#;

#[test]
fn question_is_rejected_by_the_full_pipeline() {
    let err = compile(QUESTION_REPRO, "question", false).unwrap_err();
    assert!(err.contains("`?` operator"), "{}", err);
    assert!(err.contains("is not implemented"), "{}", err);
}

#[test]
fn await_is_rejected_by_the_full_pipeline() {
    let err = compile(AWAIT_REPRO, "await", false).unwrap_err();
    assert!(err.contains("`.await`"), "{}", err);
    assert!(err.contains("is not implemented"), "{}", err);
}

/// The LLVM backend needs its own coverage, and it is the sharper case.
///
/// Its expression lowering has no arm for either node: the catch-all at
/// `src/codegen/llvm_text_backend.rs:1378` returns the constant `0` for
/// `Question`, `Await`, `EnumConstructor` and `MacroInvocation` alike. That is
/// worse than the C backend's failure — it compiles, and it is wrong. These
/// programs are only safe because the type checker refuses before backend
/// selection happens, and that ordering is what this test pins.
#[test]
fn both_are_rejected_before_the_llvm_backend_can_return_zero() {
    let err = compile(QUESTION_REPRO, "question_llvm", true).unwrap_err();
    assert!(err.contains("`?` operator"), "{}", err);
    assert!(err.contains("is not implemented"), "{}", err);

    let err = compile(AWAIT_REPRO, "await_llvm", true).unwrap_err();
    assert!(err.contains("`.await`"), "{}", err);
    assert!(err.contains("is not implemented"), "{}", err);
}

/// The optimizer runs between type checking and code generation, so a constant
/// fold or a dead-code pass could in principle remove the node before any
/// backend sees it. It cannot: the refusal is raised in an earlier phase. A
/// `?` in a function nobody calls is still an error.
#[test]
fn question_in_unreachable_code_is_still_rejected() {
    let source = r#"
enum Result<T, E> {
    Ok(T),
    Err(E),
}

fn might_fail(x: i64) -> Result<i64, i64> {
    return might_fail(x);
}

fn never_called(x: i64) -> Result<i64, i64> {
    let v: i64 = might_fail(x)?;
    return might_fail(v);
}

fn main() {
    print("main does not call never_called");
}
"#;
    let err = compile(source, "unreachable", false).unwrap_err();
    assert!(err.contains("`?` operator"), "{}", err);
}

// ---------------------------------------------------------------------------
// The workarounds the diagnostics suggest, executed
// ---------------------------------------------------------------------------

/// `= help: match on the returned enum and handle each variant explicitly`
///
/// This is the `?` repro rewritten the way the diagnostic says to rewrite it.
/// It compiles, links and prints both branches.
#[test]
fn question_workaround_compiles_and_runs() {
    let source = r#"
enum Result {
    Ok(i64),
    Err(i64),
}

fn might_fail(x: i64) -> Result {
    if x < 0 {
        return Result::Err(0 - x);
    }
    return Result::Ok(x * 2);
}

fn main() {
    match might_fail(5) {
        Result::Ok(v) => print_int(v),
        Result::Err(e) => print_int(e),
    }
    match might_fail(0 - 7) {
        Result::Ok(v) => print_int(v),
        Result::Err(e) => print_int(e),
    }
}
"#;
    let out =
        compile_and_run(source, "q_workaround").expect("the suggested workaround must compile");
    assert_eq!(out.split_whitespace().collect::<Vec<_>>(), vec!["10", "7"]);
}

/// `= help: declare the function to return its value directly (`-> T`, not
/// `-> Future<T>`) and call it`
#[test]
fn await_workaround_compiles_and_runs() {
    let source = r#"
fn work(x: i64) -> i64 {
    return x * 2;
}

fn main() {
    let v: i64 = work(3);
    print_int(v);
}
"#;
    let out =
        compile_and_run(source, "a_workaround").expect("the suggested workaround must compile");
    assert_eq!(out.trim(), "6");
}

/// The suggestion the `.await` diagnostic must never make.
///
/// Deleting `.await` and changing nothing else leaves a `Future<i64>` bound to
/// an `i64`. This test exists so that if anyone "simplifies" the help text back
/// to "just remove the .await", the receipt for why that is wrong is already in
/// the suite.
#[test]
fn deleting_the_await_alone_does_not_compile() {
    let source = r#"
fn work(x: i64) -> Future<i64> {
    return work(x);
}

fn main() {
    let v: i64 = work(3);
    print_int(v);
}
"#;
    let err = compile(source, "await_naive_fix", false).unwrap_err();
    assert!(err.contains("Type mismatch"), "{}", err);
    assert!(err.contains("Future"), "{}", err);
}
