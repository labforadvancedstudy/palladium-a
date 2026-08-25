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
use palladium::{CompileError, Driver};
use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Run the full driver over `source`.
///
/// Returns the whole rendered diagnostic on failure — headline, notes and
/// suggestions — not `CompileError::to_string()`, which is only the headline.
/// An earlier version of this helper returned the headline, and every
/// assertion below of the form "the help does not say X" passed vacuously
/// because the help was never in the string being searched.
fn compile(source: &str, name: &str, llvm: bool) -> Result<std::path::PathBuf, String> {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    fs::write(&src, source).unwrap();

    let driver = if llvm {
        Driver::new().with_llvm()
    } else {
        Driver::new()
    };
    driver.compile_file(&src).map_err(rendered)
}

/// Headline + notes + suggestions, i.e. everything the user is shown.
fn rendered(e: CompileError) -> String {
    let d = e.to_diagnostic();
    let mut out = vec![d.message.clone()];
    out.extend(d.notes.iter().cloned());
    out.extend(d.suggestions.iter().map(|s| s.message.clone()));
    out.join("\n")
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
        return Err(format!(
            "program failed: {}",
            String::from_utf8_lossy(&run.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&run.stdout).to_string())
}

// ---------------------------------------------------------------------------
// The refusal, through the real pipeline
// ---------------------------------------------------------------------------

/// The measured repro. It declares a `Result` enum and a function returning it,
/// which is how it used to satisfy the type rules that rejected the naive
/// misuse — those rules are gone, and this is kept because it is the program
/// that actually produced bad C: `struct Result __question_result_1 =
/// might_fail(x);`, to which gcc answered `variable has incomplete type`.
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
/// typed as its bare return type, so awaiting one never type checked — which is
/// why, back when type rules gated `.await`, this was the only shape that
/// reached the backend. There it emitted `while (!f.poll(&f)) {}` on a
/// `long long`. The refusal no longer depends on the shape; see
/// `both_constructs_are_rejected_whatever_the_operand_is`.
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

/// The refusal is raised before the operand is looked at, so these reach it
/// too — and the old type rules that would have rejected them first are gone.
///
/// This is the reachability the help text has to survive. `3?` has no enum to
/// match on and `7.await` has no function whose signature could be changed, so
/// a message phrased for "the returned enum" or "the function you declared" is
/// simply wrong here. The assertions below are the durable half: whatever the
/// operand, the diagnostic names the construct and does not name a shape.
#[test]
fn both_constructs_are_rejected_whatever_the_operand_is() {
    let cases = [
        (
            "q_literal",
            "fn main() {\n    let v: i64 = 3?;\n    print_int(v);\n}\n",
            "`?` operator",
        ),
        (
            "q_unknown_call",
            "fn main() {\n    let v: i64 = unknown()?;\n    print_int(v);\n}\n",
            "`?` operator",
        ),
        (
            "a_variable",
            "fn main() {\n    let f: i64 = 7;\n    let v: i64 = f.await;\n    print_int(v);\n}\n",
            "`.await`",
        ),
    ];

    for (name, source, construct) in cases {
        let err = compile(source, name, false).unwrap_err();
        assert!(err.contains(construct), "{}: {}", name, err);
        assert!(err.contains("is not implemented"), "{}: {}", name, err);
    }
}

/// The help must not assume an operand shape it never inspected.
///
/// `?` may not claim the operand *is* a Result (`3?` is not), and `.await` may
/// not claim there is a function to edit (`f.await` on a variable has none).
/// Phrasings that presume either are listed by name so that a future
/// "simplification" back to them fails here rather than in a user's terminal.
#[test]
fn help_text_does_not_presume_an_operand_shape() {
    let question = compile("fn main() {\n    let v: i64 = 3?;\n}\n", "q_shape", false).unwrap_err();
    for presumption in [
        "match on the returned enum",
        "the Result value",
        "return the Result",
    ] {
        assert!(
            !question.contains(presumption),
            "`3?` has no Result: {}",
            question
        );
    }

    let awaited = compile(
        "fn main() {\n    let f: i64 = 7;\n    let v: i64 = f.await;\n}\n",
        "a_shape",
        false,
    )
    .unwrap_err();
    for presumption in [
        "declare the function to return",
        "call the function without",
    ] {
        assert!(
            !awaited.contains(presumption),
            "`f.await` has no function: {}",
            awaited
        );
    }
    // What it may say instead: the conditional form, which is true either way.
    assert!(awaited.contains("If a function is declared"), "{}", awaited);
}

/// The LLVM backend needs its own coverage, and it is the sharper case.
///
/// Its expression lowering HAD no arm for either node: a catch-all returned the
/// constant `0` for `Question`, `Await`, `EnumConstructor` and `MacroInvocation`
/// alike — worse than the C backend's failure, because it compiled and was wrong.
/// D10 replaced it with four separate refusals
/// (`src/codegen/llvm_text_backend.rs:1469-1482`), so the deleted catch-all is
/// described here without a citation form. What this test pins is unchanged and
/// is the reason those programs were safe even then: the type checker refuses
/// before backend selection happens.
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

/// `= help: … return the value and dispatch on it with `match`. Only
/// non-generic enums are compiled …`
///
/// Dispatch, the base case. Note the enum is deliberately non-generic: that is
/// what the help now tells the reader to write, and
/// `generic_result_is_not_a_compilable_workaround` below is why.
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

/// The thing `?` is actually *for*: carrying a failure up to the caller.
///
/// A dispatch-only receipt would have been a proxy — it proves you can print
/// per variant, not that you can replace error propagation. This is the repro's
/// `helper` (which used `?` to propagate) written the way the help says.
///
/// It also pins a real syntactic trap found while measuring: a match arm that
/// is a block must NOT be followed by a comma. Every other form fails to parse:
///
///   `Ok(v) => return …,`     -> Expected expression, but found 'return'
///   `Ok(v) => out = …,`      -> Expected ',' after match arm expression
///   `Ok(v) => { return …; },`-> Expected pattern, but found ','
#[test]
fn question_workaround_propagates_out_of_a_helper() {
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

fn helper(x: i64) -> Result {
    match might_fail(x) {
        Result::Ok(v) => { return Result::Ok(v + 1); }
        Result::Err(e) => { return Result::Err(e); }
    }
}

fn main() {
    match helper(5) {
        Result::Ok(v) => print_int(v),
        Result::Err(e) => print_int(e),
    }
    match helper(0 - 7) {
        Result::Ok(v) => print_int(v),
        Result::Err(e) => print_int(e),
    }
}
"#;
    let out = compile_and_run(source, "q_propagate")
        .expect("propagation is the point of `?`; the replacement must support it");
    assert_eq!(out.split_whitespace().collect::<Vec<_>>(), vec!["11", "7"]);
}

/// The help must not be an `i64`-only trick. A `String` payload is the case
/// most likely to break, since strings are `const char*` in the generated C.
#[test]
fn question_workaround_is_not_limited_to_i64_payloads() {
    let source = r#"
enum Outcome {
    Good(String),
    Bad(String),
}

fn attempt(x: i64) -> Outcome {
    if x < 0 {
        return Outcome::Bad("negative");
    }
    return Outcome::Good("fine");
}

fn main() {
    match attempt(5) {
        Outcome::Good(s) => print(s),
        Outcome::Bad(s) => print(s),
    }
}
"#;
    let out = compile_and_run(source, "q_string").expect("payload types other than i64 must work");
    assert_eq!(out.trim(), "fine");
}

/// The limit the help now states out loud, pinned so it cannot silently drift.
///
/// Code generation skips generic enum definitions entirely, at all four sites —
/// the two that COLLECT (`src/codegen/mod.rs:1910-1914`,
/// `src/codegen/mod.rs:1946-1950`) and the two that EMIT
/// (`src/codegen/mod.rs:2010-2015`, `src/codegen/mod.rs:2040-2044`) — and
/// so a program that constructs one would reach the C compiler with no type,
/// no tag and no constructor to link against.
///
/// The type checker now REFUSES the construction outright rather than letting
/// it through to a link error, so the failure this test pins is that refusal —
/// not the `Type mismatch` the inference used to produce (it inferred only the
/// type parameters a variant mentions, so `Result::Err(e)` yielded
/// `Result<(), Int>` and never matched the declared `Result<i64, i64>`). Either
/// way the conclusion the help text depends on is unchanged: a `match`-based
/// replacement written against a generic `Result<T, E>` does not compile, which
/// is why the help tells the reader to declare a concrete enum.
///
/// If this test ever starts failing because the program now *compiles*, that is
/// good news and the help text should be widened to match.
#[test]
fn generic_result_is_not_a_compilable_workaround() {
    let source = r#"
enum Result<T, E> {
    Ok(T),
    Err(E),
}

fn might_fail(x: i64) -> Result<i64, i64> {
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
}
"#;
    let err = compile(source, "q_generic", false)
        .expect_err("if generic enums now work, widen the `?` help text");
    assert!(
        err.contains("constructs a variant of a GENERIC enum, and generic enums are not implemented"),
        "{}",
        err
    );
    assert!(err.contains("Result::Err"), "{}", err);
}

/// `= help: … If a function is declared `-> Future<T>`, change it to `-> T``
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
