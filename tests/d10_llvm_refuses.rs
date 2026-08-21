//! D10: `--llvm` refuses, end to end, and writes nothing.
//!
//! The defect this began as was one arm: `generate_expression` ended with a
//! catch-all returning the constant `0` and no IR, so `let c = Color::Green;`
//! became `store i64 0`, `Color::Red` and `Color::Green` compiled to
//! byte-identical modules, and `pdc compile --llvm` exited 0 saying
//! "Compilation successful".
//!
//! Fixing that arm was necessary and not sufficient. Auditing the rest of the
//! backend turned up seven further sites that fail *quietly* rather than
//! loudly — the sharpest being field access, which hard-codes index 0 for
//! reads and writes alike, so `print_int(p.y)` reads `p.x` and produces
//! **valid IR with the wrong meaning**. Checking the assembly is no defence
//! against that, and a granular refusal list covering half the fabrications
//! reads as protection while providing none. So the flag refuses wholesale.
//!
//! What lives where:
//!
//! * The granular refusals — enum construction, `?`, `.await`, macro
//!   invocation, `break`, `continue`, enum patterns — are still in the backend
//!   and still executable, driven through `compile_unchecked` by the unit tests
//!   in `src/codegen/llvm_text_backend.rs`. They are the record of *what* is
//!   unimplemented and become live again the moment the gate lifts. Their
//!   `help:` receipts live beside them, so the text and the program that proves
//!   the text are in one place, and every assertion reads the real
//!   `to_diagnostic()` rather than a copy of it in a comment.
//! * This file covers the property only a real compilation can show: a `--llvm`
//!   invocation fails, and leaves nothing on disk for a linker to pick up.

use palladium::errors::Diagnostic;
use palladium::{CompileError, Driver};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// Where `LLVMTextBackend::write_output` would put the module for `name.pd`.
///
/// Relative to the crate root, which is the cwd for `cargo test`. Knowing this
/// path is the point: the pre-fix compiler wrote it, and `pdc run --llvm` then
/// handed it straight to gcc.
fn expected_ir_path(name: &str) -> PathBuf {
    PathBuf::from("build_output").join(format!("{}.pd.ll", name))
}

/// Run the full driver under `--llvm` and return the error.
///
/// Deliberately the real `Driver` rather than the backend in isolation: the
/// defect was that a program got all the way to a written `.ll` file, and only
/// an end-to-end run can show that it no longer does.
fn compile_under_llvm(source: &str, name: &str) -> CompileError {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    fs::write(&src, source).unwrap();

    Driver::new()
        .with_llvm()
        .compile_file(&src)
        .map(|p| p.display().to_string())
        .expect_err("`--llvm` must refuse")
}

/// The refusal, plus proof it left no module behind.
///
/// The output path is removed first, so "the file is absent" means *this*
/// compilation did not create it, rather than that nothing ever did.
fn refuse_and_assert_nothing_written(source: &str, name: &str) -> Diagnostic {
    let ir = expected_ir_path(name);
    let _ = fs::remove_file(&ir);

    let err = compile_under_llvm(source, name);

    assert!(
        !ir.exists(),
        "{} was written despite the refusal; a linker would have consumed it",
        ir.display()
    );
    err.to_diagnostic()
}

fn assert_is_the_backend_refusal(diag: &Diagnostic) {
    assert!(
        diag.message.contains("the LLVM backend (`--llvm`)")
            && diag.message.contains("is not implemented"),
        "headline was {:?}",
        diag.message
    );
    assert!(
        diag.notes.iter().any(|n| n.contains("kept for development")),
        "the note must say why the backend still exists: {:?}",
        diag.notes
    );
    let help = &diag
        .suggestions
        .first()
        .expect("a refusal with no `help:` is half a diagnostic")
        .message;
    assert!(
        help.contains("default C backend") && help.contains("dropping `--llvm`"),
        "the help must name the working backend: {:?}",
        help
    );
    assert!(
        help.contains("docs/specification/language-spec.md"),
        "the help must point at the specification, not a milestone number: {:?}",
        help
    );
}

// ---------------------------------------------------------------------------
// The gate
// ---------------------------------------------------------------------------

/// A program with nothing exotic in it at all.
///
/// This is the case that makes the gate wholesale rather than granular: every
/// construct here is one the skeleton "supports", and it still refuses —
/// because support was exactly the claim that was false. Pre-fix this produced
/// a module, linked, and ran.
const ORDINARY: &str = r#"
fn double(x: i64) -> i64 {
    return x * 2;
}

fn main() {
    let v: i64 = double(21);
    print_int(v);
}
"#;

/// The measured field-zero corruption, in source form.
///
/// Pre-fix under `--llvm`, `p.y` lowered to a `getelementptr` on index 0, so
/// the program read `x`. The IR was valid. The C backend prints `22`. This is
/// the case that decided the gate: no amount of assembling or linking the
/// output would have caught it.
const FIELD_ZERO: &str = r#"
struct Point {
    x: i64,
    y: i64,
}

fn main() {
    let p = Point { x: 11, y: 22 };
    print_int(p.y);
}
"#;

#[test]
fn an_ordinary_program_is_refused_under_llvm() {
    let diag = refuse_and_assert_nothing_written(ORDINARY, "d10_ordinary");
    assert_is_the_backend_refusal(&diag);
}

#[test]
fn the_field_zero_corruption_can_no_longer_be_reached() {
    let diag = refuse_and_assert_nothing_written(FIELD_ZERO, "d10_field_zero");
    assert_is_the_backend_refusal(&diag);
}

/// Every construct the granular refusals name, through the real driver.
///
/// They all produce the *same* message now, which is the intended outcome: a
/// `--llvm` user has one problem, not seven, and it is the backend.
#[test]
fn every_measured_repro_is_refused_and_writes_nothing() {
    let cases = [
        // enum construction — the headline of the original defect
        (
            r#"
enum Color { Red, Green }

fn main() {
    let c = Color::Green;
    print_int(7);
}
"#,
            "d10_enum",
        ),
        // enum constructor arguments, which used to vanish with their effects
        (
            r#"
enum Wrapper { Val(i64) }

fn loud(x: i64) -> i64 {
    print_int(x);
    return x;
}

fn main() {
    let w = Wrapper::Val(loud(99));
    print_int(1);
}
"#,
            "d10_enum_args",
        ),
        // `?`
        (
            r#"
enum Result<T, E> {
    Ok(T),
    Err(E),
}

fn might_fail(x: i64) -> Result<i64, i64> {
    return might_fail(x);
}

fn helper(x: i64) -> Result<i64, i64> {
    let v: i64 = might_fail(x)?;
    return might_fail(v);
}

fn main() {
    helper(3);
}
"#,
            "d10_question",
        ),
        // `.await`
        (
            r#"
fn work(x: i64) -> Future<i64> {
    return work(x);
}

fn main() {
    let v: i64 = work(3).await;
    print_int(v);
}
"#,
            "d10_await",
        ),
        // `break`
        (
            r#"
fn main() {
    let mut i: i64 = 0;
    while i < 10 {
        if i > 3 {
            break;
        }
        print_int(i);
        i = i + 1;
    }
}
"#,
            "d10_break",
        ),
        // `continue`
        (
            r#"
fn main() {
    let mut i: i64 = 0;
    while i < 5 {
        i = i + 1;
        if i == 3 {
            continue;
        }
        print_int(i);
    }
}
"#,
            "d10_continue",
        ),
        // an enum pattern, isolated from enum construction
        (
            r#"
enum Color { Red, Green }

fn describe(c: Color) {
    match c {
        Color::Red => print_int(1),
        Color::Green => print_int(2),
    }
}

fn main() {
    print_int(0);
}
"#,
            "d10_enum_pattern",
        ),
    ];

    for (source, name) in cases {
        let diag = refuse_and_assert_nothing_written(source, name);
        assert_is_the_backend_refusal(&diag);
    }
}

/// The refusal survives being rendered.
///
/// `Driver::compile_file` pipes every error through `ErrorReporter`, which used
/// to panic on out-of-range span arithmetic — measured aborting the compiler
/// with `end byte index 15 is out of bounds for string of length 11`, taking
/// the diagnostic with it. A crash on the reporting path turns a correct
/// refusal into a lost one, so it is checked rather than assumed.
#[test]
fn reporting_the_refusal_does_not_panic_on_a_short_line() {
    let diag = refuse_and_assert_nothing_written("fn main() {}\n", "d10_short");
    assert_is_the_backend_refusal(&diag);
    assert!(
        diag.span.is_none(),
        "a whole-backend refusal must not claim a source location"
    );
}

/// The default backend still compiles everything above.
///
/// Without this the suite would be equally happy if `--llvm` had been "fixed"
/// by breaking the compiler.
#[test]
fn the_default_backend_still_compiles_what_llvm_now_refuses() {
    for (source, name) in [
        (ORDINARY, "d10_default_ordinary"),
        (FIELD_ZERO, "d10_default_field"),
    ] {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join(format!("{}.pd", name));
        fs::write(&src, source).unwrap();
        Driver::new()
            .compile_file(&src)
            .unwrap_or_else(|e| panic!("the C backend must still compile {}: {}", name, e));
    }
}
