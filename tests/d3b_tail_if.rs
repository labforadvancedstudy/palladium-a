//! D3b: a tail `if`/`match` is the function's return value, and the shapes that
//! cannot be one are refused with advice that actually compiles.
//!
//! WHAT THIS ADDS OVER THE GENERATED-C INVARIANT
//! `scripts/check-c-returns.py` (run over every `tests/stdlib/` driver by
//! `make stdlib-gate`) already catches the *structural* half: a non-void
//! function that can fall off its end. It is the stronger net for that
//! question — it needs no execution, it is optimisation-independent, and it
//! covers constructs nobody has written a fixture for.
//!
//! It cannot see two things, which is what these tests are for:
//!
//!   1. **Which value comes back.** A function can return on every path and
//!      still return the wrong one — a lowering that rewrote only the `if`
//!      branch, or attached the else branch's value to the wrong arm, is
//!      structurally clean and semantically wrong. `tail_if_returns_the_right_
//!      value_from_each_branch` runs the program and compares numbers.
//!   2. **A refusal.** A program that is rejected emits no C at all, so a
//!      checker over emitted C is silent about it by construction. Every
//!      assertion below about `CompileError::Unimplemented` — and both
//!      workarounds its help suggests, compiled AND run — is invisible to the
//!      structural net.
//!
//! The redundant part is deliberately not duplicated here: there is no
//! assertion that some function "contains a `return`". That is the invariant's
//! job, and the invariant states it better ("returns on every path" — the rule
//! that a function with an early `return` plus a tail `if` defeats).

use palladium::linker::{link_command, OptLevel};
use palladium::{CompileError, Driver};
use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Run the full driver over `source`, returning the emitted C file's contents.
fn compile_to_c(source: &str, name: &str) -> Result<String, String> {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    fs::write(&src, source).unwrap();
    let c_file = Driver::new().compile_file(&src).map_err(rendered)?;
    fs::read_to_string(&c_file).map_err(|e| format!("reading {}: {}", c_file.display(), e))
}

/// Headline + notes + suggestions, i.e. everything the user is shown.
///
/// `CompileError::to_string()` is only the headline, so asserting over it would
/// make every claim about the note and the help pass vacuously.
fn rendered(e: CompileError) -> String {
    let d = e.to_diagnostic();
    let mut out = vec![d.message.clone()];
    out.extend(d.notes.iter().cloned());
    out.extend(d.suggestions.iter().map(|s| s.message.clone()));
    out.join("\n")
}

/// Compile, link against the real runtime, run, and return stdout.
///
/// Uses `link_command` rather than a bare `cc` so the runtime and prelude are
/// resolved exactly as `pdc compile` resolves them.
fn compile_and_run(source: &str, name: &str) -> Result<String, String> {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    let exe = dir.path().join(name);
    fs::write(&src, source).unwrap();

    let c_file = Driver::new()
        .compile_file(&src)
        .map_err(|e| format!("compilation failed: {}", rendered(e)))?;

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
// The repro
// ---------------------------------------------------------------------------

/// The exact program from the defect report. Measured before the fix:
/// compiled clean, exit 0, no diagnostic, and printed 8261746944.
const FIB: &str = r#"
fn fib(n: i64) -> i64 {
    if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
}

fn main() {
    print_int(fib(10));
}
"#;

#[test]
fn fib_with_a_tail_if_returns_55_not_garbage() {
    let out = compile_and_run(FIB, "d3b_fib").expect("fib must compile, link and run");
    assert_eq!(
        out.trim(),
        "55",
        "fib(10) is 55; a tail `if` that emits no `return` printed 8261746944 here"
    );
}

/// The value-level half of the fix, which the structural invariant cannot see:
/// BOTH branches must return, and each must return ITS OWN value.
///
/// A lowering that rewrote only the first branch, or that duplicated one
/// branch's expression into the other, still returns on every path and would
/// pass `scripts/check-c-returns.py`.
#[test]
fn tail_if_returns_the_right_value_from_each_branch() {
    let out = compile_and_run(
        r#"
fn pick(n: i64) -> i64 {
    if n > 0 { 111 } else { 222 }
}

fn main() {
    print_int(pick(1));
    print_int(pick(-1));
}
"#,
        "d3b_branches",
    )
    .expect("must compile, link and run");
    assert_eq!(out.trim(), "111\n222", "each branch returns its own value");
}

/// `else if` is not in the grammar ("Expected '{' after else", measured), so a
/// three-way choice nests an `if` inside the `else`. The lowering has to
/// recurse to reach it.
#[test]
fn a_tail_if_nested_in_an_else_is_lowered_too() {
    let out = compile_and_run(
        r#"
fn sign(n: i64) -> i64 {
    if n > 0 { 1 } else { if n < 0 { 2 } else { 3 } }
}

fn main() {
    print_int(sign(7));
    print_int(sign(-7));
    print_int(sign(0));
}
"#,
        "d3b_nested",
    )
    .expect("must compile, link and run");
    assert_eq!(out.trim(), "1\n2\n3");
}

/// A tail expression in one branch and an explicit `return` in the other.
///
/// This is why the parser walks the statements alongside the tail rather than
/// the tail alone: the `if` branch has no tail expression to lower, but it is
/// not a fall-through either. A tail-only rule refuses this program, and this
/// program is fine.
#[test]
fn a_branch_that_already_returns_is_not_refused() {
    let out = compile_and_run(
        r#"
fn mixed(n: i64) -> i64 {
    if n > 0 {
        return n * 10;
    } else {
        n - 1
    }
}

fn main() {
    print_int(mixed(3));
    print_int(mixed(-3));
}
"#,
        "d3b_mixed",
    )
    .expect("must compile, link and run");
    assert_eq!(out.trim(), "30\n-4");
}

/// A branch that ends in `panic(...)` never comes back, so it needs no value.
/// This mirrors `NORETURN_RE` in `scripts/check-c-returns.py`; without it the
/// parser would refuse a program whose C is correct.
#[test]
fn a_branch_that_panics_is_not_refused() {
    let out = compile_and_run(
        r#"
fn checked(n: i64) -> i64 {
    if n > 0 {
        n * 2
    } else {
        panic("checked: n must be positive");
    }
}

fn main() {
    print_int(checked(21));
}
"#,
        "d3b_panic",
    )
    .expect("must compile, link and run");
    assert_eq!(out.trim(), "42");
}

// ---------------------------------------------------------------------------
// Tail `match` — the same defect, through the `pattern => expression,` arm
// ---------------------------------------------------------------------------

/// Measured before the fix: `area(Square)` printed -16.
///
/// This is the only match arm form that can carry a value; a block-bodied arm
/// parses with `parse_statement` and so demands a `;`, making `Circle => { 1 }`
/// a parse error.
#[test]
fn tail_match_arms_are_lowered_to_returns() {
    let out = compile_and_run(
        r#"
enum Shape {
    Circle,
    Square,
}

fn area(s: Shape) -> i64 {
    match s {
        Shape::Circle => 1,
        Shape::Square => 2,
    }
}

fn main() {
    print_int(area(Shape::Circle));
    print_int(area(Shape::Square));
}
"#,
        "d3b_match",
    )
    .expect("must compile, link and run");
    assert_eq!(out.trim(), "1\n2", "each arm returns its own value");
}

// ---------------------------------------------------------------------------
// The refusal, and the two workarounds its help suggests
// ---------------------------------------------------------------------------

#[test]
fn a_tail_if_with_no_else_is_refused_not_miscompiled() {
    let err = compile_to_c(
        r#"
fn f(n: i64) -> i64 {
    if n > 0 { n }
}

fn main() {
    print_int(f(3));
}
"#,
        "d3b_no_else",
    )
    .expect_err("a tail `if` with no `else` has no value on the false path");

    assert!(
        err.contains("is not implemented"),
        "must be the house refusal, got:\n{}",
        err
    );
    assert!(
        err.contains("tail `if`"),
        "the message must name the construct, got:\n{}",
        err
    );
    assert!(
        err.contains("no `else` branch"),
        "the note must name the path that has no value, got:\n{}",
        err
    );
}

/// A branch that produces no value while another does is the same defect
/// without the missing `else` — the `else` is there and simply has nothing to
/// return.
#[test]
fn a_tail_if_whose_else_produces_no_value_is_refused() {
    let err = compile_to_c(
        r#"
fn f(n: i64) -> i64 {
    if n > 0 { n } else { print_int(0); }
}

fn main() {
    print_int(f(3));
}
"#,
        "d3b_dead_else",
    )
    .expect_err("the `else` branch has no value to return");
    assert!(
        err.contains("the `else` branch"),
        "the note must name the else branch, got:\n{}",
        err
    );
}

#[test]
fn a_tail_match_with_a_valueless_arm_is_refused() {
    let err = compile_to_c(
        r#"
enum Shape {
    Circle,
    Square,
}

fn area(s: Shape) -> i64 {
    match s {
        Shape::Circle => 1,
        Shape::Square => {
            print_int(0);
        }
    }
}

fn main() {
    print_int(area(Shape::Circle));
}
"#,
        "d3b_match_valueless",
    )
    .expect_err("arm 2 has no value to return");
    assert!(
        err.contains("tail `match`"),
        "the message must name `match`, not `if`, got:\n{}",
        err
    );
    assert!(
        err.contains("match arm 2"),
        "the note must name the offending arm, got:\n{}",
        err
    );
}

/// The help says "add the missing `else`/arm with its own tail expression".
/// A suggestion that has never been executed is a claim.
#[test]
fn no_else_workaround_else_branch_compiles_and_runs() {
    let out = compile_and_run(
        r#"
fn f(n: i64) -> i64 {
    if n <= 1 { n } else { n * 2 }
}

fn main() {
    print_int(f(1));
    print_int(f(5));
}
"#,
        "d3b_fix_else",
    )
    .expect("the suggested `else` workaround must compile and run");
    assert_eq!(out.trim(), "1\n10");
}

/// The help's second suggestion: drop tail position and write explicit returns.
#[test]
fn no_else_workaround_explicit_returns_compiles_and_runs() {
    let out = compile_and_run(
        r#"
fn f(n: i64) -> i64 {
    if n <= 1 { return n; }
    return n * 2;
}

fn main() {
    print_int(f(1));
    print_int(f(5));
}
"#,
        "d3b_fix_returns",
    )
    .expect("the suggested explicit-return workaround must compile and run");
    assert_eq!(out.trim(), "1\n10");
}

// ---------------------------------------------------------------------------
// What must NOT change
// ---------------------------------------------------------------------------

/// A unit function's tail value is discarded, not returned. Lowering it would
/// emit `return __pd_print_int(n);` from a `void` function, which gcc rejects.
/// This is the regression guard on the `return_type.is_some()` condition.
#[test]
fn a_unit_functions_tail_if_is_not_lowered() {
    let c = compile_to_c(
        r#"
fn f(n: i64) {
    if n > 0 { print_int(n); } else { print_int(0); }
}

fn main() {
    f(3);
}
"#,
        "d3b_unit",
    )
    .expect("a unit function with a tail `if` must still compile");
    assert!(
        !c.contains("return __pd_print_int"),
        "a void function must not return its tail value:\n{}",
        c
    );
}

/// An `if` that is not the last statement of the body is an ordinary
/// statement, and its branches keep ordinary expression statements.
#[test]
fn an_if_in_the_middle_of_a_body_is_not_lowered() {
    let out = compile_and_run(
        r#"
fn f(n: i64) -> i64 {
    if n > 0 {
        print_int(n);
    }
    n + 1
}

fn main() {
    print_int(f(3));
}
"#,
        "d3b_middle",
    )
    .expect("must compile, link and run");
    assert_eq!(
        out.trim(),
        "3\n4",
        "the mid-body `if` runs for effect; the tail expression is the value"
    );
}
