//! D10: the LLVM backend refuses what it cannot lower, instead of inventing `0`.
//!
//! Before this fix, `src/codegen/llvm_text_backend.rs` ended its expression
//! match with
//!
//! ```text
//! _ => {
//!     // TODO: Implement EnumConstructor, Question, MacroInvocation, Await
//!     Ok((String::new(), "0".to_string()))
//! }
//! ```
//!
//! which is a worse failure than the C backend's version of the same gap. The
//! C backend emits references to types it never defines, so gcc rejects the
//! program and the user finds out. This one produced valid-looking IR: with
//! `--llvm`, `let c = Color::Green;` became `store i64 0`, `Color::Red` and
//! `Color::Green` compiled to byte-identical modules, and the arguments of a
//! data-carrying variant were dropped along with any side effects inside them.
//! The compiler exited 0 and printed "Compilation successful".
//!
//! Two things are pinned here.
//!
//! 1. Each construct the backend cannot lower now produces a diagnostic. These
//!    tests fail against the pre-fix source, where every one of them returned
//!    `Ok` and wrote a `.ll` file.
//! 2. Every `help:` line is a workaround that is compiled and run, not
//!    asserted. A suggestion nobody has executed is a claim, and this milestone
//!    is about the compiler not making claims it cannot back.
//!
//! The exhaustiveness gate is not testable from here: it is the absence of a
//! `_` arm in `generate_expression`, which makes `rustc` refuse to build the
//! compiler when someone adds an `Expr` variant. That is a stronger guarantee
//! than any test, and it is what stops the next unlowerable node from becoming
//! the next silent `0`.

use palladium::ast::{Expr, Function, Item, Program, Stmt, Visibility};
use palladium::codegen::llvm_text_backend::LLVMTextBackend;
use palladium::errors::Span;
use palladium::linker::{link_command, OptLevel};
use palladium::Driver;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// Run the full driver over `source`. Returns the error message on failure.
///
/// Deliberately the real `Driver` rather than the backend in isolation: the
/// point of the defect was that a program got all the way to a written `.ll`
/// file, and only an end-to-end run can show that it no longer does.
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

/// Compile with the C backend, link against the real runtime, run, return stdout.
///
/// Uses `link_command` rather than a bare `cc` because it resolves
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

fn assert_refused(err: &str, construct: &str) {
    assert!(
        err.contains(construct),
        "expected a refusal naming {:?}, got: {}",
        construct,
        err
    );
    assert!(
        err.contains("is not implemented"),
        "expected an `is not implemented` refusal, got: {}",
        err
    );
}

// ---------------------------------------------------------------------------
// The headline: enum construction
// ---------------------------------------------------------------------------

/// The measured repro.
///
/// Pre-fix, `pdc compile --llvm` on this exited 0 and wrote a module whose
/// `main` began `%0 = alloca i64 / store i64 0, i64* %0`. Replacing `Green`
/// with `Red` produced a byte-identical module: the backend had no way to tell
/// two variants apart because it was not looking at them.
const ENUM_REPRO: &str = r#"
enum Color {
    Red,
    Green,
}

fn main() {
    let c = Color::Green;
    let n: i64 = 7;
    print_int(n);
}
"#;

#[test]
fn enum_construction_is_refused_instead_of_becoming_zero() {
    let err = compile(ENUM_REPRO, "d10_enum", true).unwrap_err();
    assert_refused(&err, "enum construction (`Color::Green`)");
}

/// The sharper half of the same defect: the constructor's *arguments* were
/// dropped, not just its tag. `loud(99)` prints, and pre-fix the `--llvm`
/// module contained no call to `loud` at all — a side effect deleted in
/// silence. The C backend, on the same source, prints `99` then `1`.
const ENUM_SIDE_EFFECT_REPRO: &str = r#"
enum Wrapper {
    Val(i64),
}

fn loud(x: i64) -> i64 {
    print_int(x);
    return x;
}

fn main() {
    let w = Wrapper::Val(loud(99));
    print_int(1);
}
"#;

#[test]
fn enum_constructor_arguments_are_not_silently_discarded() {
    let err = compile(ENUM_SIDE_EFFECT_REPRO, "d10_enum_args", true).unwrap_err();
    assert_refused(&err, "enum construction (`Wrapper::Val`)");
}

/// `= help: build with the default C backend by dropping `--llvm`; it lowers
/// enum construction and `match` on enums`
///
/// The advice is only worth giving if it is true, so it is executed: the same
/// side-effecting program, on the backend the help names, prints both lines.
#[test]
fn enum_workaround_on_the_c_backend_compiles_and_runs() {
    let out = compile_and_run(ENUM_SIDE_EFFECT_REPRO, "d10_enum_wa")
        .expect("the suggested workaround must compile");
    assert_eq!(out.split_whitespace().collect::<Vec<_>>(), vec!["99", "1"]);
}

/// The other half of the same help text: `match` on enums, on the C backend.
/// `Color::Green` is the second variant, so a backend that fabricates `0`
/// would print `1` here rather than `2`.
#[test]
fn enum_match_workaround_on_the_c_backend_prints_the_right_variant() {
    let source = r#"
enum Color {
    Red,
    Green,
}

fn main() {
    let c = Color::Green;
    match c {
        Color::Red => print_int(1),
        Color::Green => print_int(2),
    }
}
"#;
    let out = compile_and_run(source, "d10_enum_match_wa")
        .expect("the suggested workaround must compile");
    assert_eq!(out.trim(), "2");
}

// ---------------------------------------------------------------------------
// The other three expression kinds the old catch-all covered
// ---------------------------------------------------------------------------

/// Pre-fix this compiled to `store i64 0` with *no call to `might_fail`* in the
/// function at all, so the operand was never evaluated.
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
/// the only shape that reached the backend, where pre-fix it printed `0`.
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
fn question_is_refused_by_the_llvm_backend() {
    let err = compile(QUESTION_REPRO, "d10_question", true).unwrap_err();
    assert_refused(&err, "the `?` operator");
}

#[test]
fn await_is_refused_by_the_llvm_backend() {
    let err = compile(AWAIT_REPRO, "d10_await", true).unwrap_err();
    assert_refused(&err, "`.await`");
}

/// `= help: match on the enum the operand returns and handle each variant
/// explicitly, and build with the default C backend by dropping `--llvm``
///
/// Both halves of that sentence are load-bearing. Dropping `--llvm` alone does
/// not help — the C backend emits C for a `struct Result` layout it never
/// defines and gcc answers `variable has incomplete type`. The `?` has to go
/// too, and this is the repro rewritten exactly as the help says.
#[test]
fn question_workaround_on_the_c_backend_compiles_and_runs() {
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
    let out = compile_and_run(source, "d10_question_wa")
        .expect("the suggested workaround must compile");
    assert_eq!(out.split_whitespace().collect::<Vec<_>>(), vec!["10", "7"]);
}

/// `= help: declare the function to return its value directly (`-> T`, not
/// `-> Future<T>`) and call it`
///
/// This one is checked on *both* backends, because the diagnostic is raised by
/// the LLVM backend and so its reader is an `--llvm` user: the rewrite must get
/// past that backend, which it does. It is then run on the C backend to prove
/// the answer is also correct, which is where a run can be asserted portably —
/// the `.ll` path needs an assembler that accepts LLVM IR, and `gcc` only does
/// on platforms where it is clang.
#[test]
fn await_workaround_compiles_under_llvm_and_runs_correctly() {
    let source = r#"
fn work(x: i64) -> i64 {
    return x * 2;
}

fn main() {
    let v: i64 = work(3);
    print_int(v);
}
"#;
    compile(source, "d10_await_wa_llvm", true)
        .expect("the suggested workaround must get past the backend that suggested it");

    let out =
        compile_and_run(source, "d10_await_wa").expect("the suggested workaround must compile");
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
    let err = compile(source, "d10_await_naive", true).unwrap_err();
    assert!(err.contains("Type mismatch"), "{}", err);
    assert!(err.contains("Future"), "{}", err);
}

// ---------------------------------------------------------------------------
// MacroInvocation: covered at the backend, because no source program reaches it
// ---------------------------------------------------------------------------

fn main_fn(body: Vec<Stmt>) -> Program {
    Program {
        imports: vec![],
        items: vec![Item::Function(Function {
            visibility: Visibility::Private,
            is_async: false,
            name: "main".to_string(),
            lifetime_params: vec![],
            type_params: vec![],
            const_params: vec![],
            params: vec![],
            return_type: None,
            body,
            span: Span::dummy(),
            effects: None,
        })],
    }
}

/// The fourth name in the deleted TODO comment.
///
/// Measured: no `.pd` program gets a `MacroInvocation` to this backend. An
/// undefined macro fails in expansion (`Unknown macro 'println'`), a defined
/// one is replaced by its expansion, and anything that survives expansion —
/// `Await` is the one expression `MacroExpander::expand_expr` does not recurse
/// into — is refused by the type checker at `src/typeck/mod.rs:2415`. So the
/// node is driven straight into the backend here.
///
/// Unreachable-from-source is not a reason to fabricate: the arm exists, and it
/// refuses. Pre-fix this assertion fails, because `compile` returned `Ok`.
#[test]
fn a_macro_invocation_reaching_the_backend_is_refused_not_lowered_to_zero() {
    let program = main_fn(vec![Stmt::Expr(Expr::MacroInvocation {
        name: "println".to_string(),
        args: vec![],
        span: Span::dummy(),
    })]);

    let err = LLVMTextBackend::new("d10_macro")
        .unwrap()
        .compile(&program)
        .unwrap_err()
        .to_string();
    assert_refused(&err, "the macro invocation `println!`");
}

/// `= help: declare the macro before the code that invokes it, or write out the
/// code the macro expands to`
///
/// The second clause is the one a user can always take, so it is the one that
/// gets executed: the expansion, written out, on both backends.
#[test]
fn macro_workaround_written_out_inline_compiles_and_runs() {
    let source = r#"
fn main() {
    print_int(6);
}
"#;
    compile(source, "d10_macro_wa_llvm", true).expect("the written-out expansion must compile");
    let out =
        compile_and_run(source, "d10_macro_wa").expect("the written-out expansion must compile");
    assert_eq!(out.trim(), "6");
}

// ---------------------------------------------------------------------------
// The statement-level fabrications in the same file
// ---------------------------------------------------------------------------

/// Pre-fix: `br label %loop_end_placeholder`, a label this backend never
/// defines. The module was invalid, but `pdc compile --llvm` still exited 0 and
/// still printed "Compilation successful" — the same lie in a different shape.
const BREAK_REPRO: &str = r#"
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
"#;

const CONTINUE_REPRO: &str = r#"
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
"#;

#[test]
fn break_is_refused_by_the_llvm_backend() {
    let err = compile(BREAK_REPRO, "d10_break", true).unwrap_err();
    assert_refused(&err, "`break`");
}

#[test]
fn continue_is_refused_by_the_llvm_backend() {
    let err = compile(CONTINUE_REPRO, "d10_continue", true).unwrap_err();
    assert_refused(&err, "`continue`");
}

/// `= help: build with the default C backend by dropping `--llvm`; it lowers
/// `break``
#[test]
fn break_workaround_on_the_c_backend_compiles_and_runs() {
    let out =
        compile_and_run(BREAK_REPRO, "d10_break_wa").expect("the suggested workaround must compile");
    assert_eq!(
        out.split_whitespace().collect::<Vec<_>>(),
        vec!["0", "1", "2", "3"]
    );
}

/// `= help: build with the default C backend by dropping `--llvm`; it lowers
/// `continue``
#[test]
fn continue_workaround_on_the_c_backend_compiles_and_runs() {
    let out = compile_and_run(CONTINUE_REPRO, "d10_continue_wa")
        .expect("the suggested workaround must compile");
    assert_eq!(
        out.split_whitespace().collect::<Vec<_>>(),
        vec!["1", "2", "4", "5"]
    );
}

/// Enum patterns in a `match` arm.
///
/// Isolated from enum *construction* on purpose — `describe` is never called,
/// and nothing in this program constructs a `Color` — so the refusal that fires
/// is the pattern one and not the constructor one. Pre-fix, the arm emitted
/// `br label %match_arm12`, a label allocated for a later arm and never
/// defined; the scrutinee was not compared against anything.
const ENUM_PATTERN_REPRO: &str = r#"
enum Color {
    Red,
    Green,
}

fn describe(c: Color) {
    match c {
        Color::Red => print_int(1),
        Color::Green => print_int(2),
    }
}

fn main() {
    print_int(0);
}
"#;

#[test]
fn enum_patterns_are_refused_by_the_llvm_backend() {
    let err = compile(ENUM_PATTERN_REPRO, "d10_enum_pattern", true).unwrap_err();
    assert_refused(&err, "matching the enum pattern `Color::Red`");
}

/// `= help: build with the default C backend by dropping `--llvm`; it lowers
/// `match` on enums`
#[test]
fn enum_pattern_workaround_on_the_c_backend_compiles_and_runs() {
    let out = compile_and_run(ENUM_PATTERN_REPRO, "d10_enum_pattern_wa")
        .expect("the suggested workaround must compile");
    assert_eq!(out.trim(), "0");
}

// ---------------------------------------------------------------------------
// The property, stated once
// ---------------------------------------------------------------------------

/// No refusal writes output.
///
/// The defect was not only a wrong value; it was a wrong value that reached
/// disk and then the linker. `compile` returning `Err` is what stops that, and
/// this checks the whole set in one place so that a future arm which "refuses"
/// by logging and continuing is caught.
#[test]
fn nothing_the_backend_refuses_produces_a_module() {
    for (source, name) in [
        (ENUM_REPRO, "d10_none_enum"),
        (ENUM_SIDE_EFFECT_REPRO, "d10_none_enum_args"),
        (QUESTION_REPRO, "d10_none_question"),
        (AWAIT_REPRO, "d10_none_await"),
        (BREAK_REPRO, "d10_none_break"),
        (CONTINUE_REPRO, "d10_none_continue"),
        (ENUM_PATTERN_REPRO, "d10_none_pattern"),
    ] {
        let result = compile(source, name, true);
        assert!(
            result.is_err(),
            "{} compiled to {:?} under --llvm; the backend cannot lower it",
            name,
            result.ok()
        );
    }
}
