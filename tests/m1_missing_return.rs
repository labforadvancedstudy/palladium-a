//! M1: a function that declares a return type may not reach its closing brace.
//!
//! THE DEFECT THIS CLOSES, MEASURED ON `main` BEFORE THE FIX
//!
//! ```text
//! fn get_value() -> i64 { }
//! fn main() { print_int(get_value()); }
//! ```
//!
//! compiled clean ("✅ Compilation successful"), emitted
//!
//! ```text
//! long long get_value();
//! long long get_value() {
//! }
//! ```
//!
//! and gcc accepted it — `-Wreturn-type` is a warning, and it is not on. The
//! call site read whatever was in the return register. `scripts/check-c-
//! returns.py` names it after the fact; nothing refused it. That is exactly
//! what M1 exists to remove, and the declared XFAIL was
//! `tests/compiler_comprehensive_test.rs::test_missing_return_is_an_error`.
//!
//! WHY THIS FILE IS MOSTLY ACCEPT-SIDE
//! The fix is a REFUSAL, so its errors land on VALID programs — the direction
//! this repo has been bitten in twice. `tests/d3b_tail_if.rs` already receipts
//! the accept side of the *tail-value* refusal; the refusal added here fires on
//! a strictly larger set (every path shape, not only the ones with a value
//! written in tail position), so its accept side needs its own receipts. Each
//! one below compiles, links against the real runtime, RUNS, and is compared
//! against a number — a program that compiles but returns the wrong value is
//! the defect, not the cure.
//!
//! Every accepted program is also handed to `scripts/check-c-returns.py`, the
//! same agreement `tests/d3b_tail_if.rs` executes: the parser decides "returns
//! on every path" over Palladium statements, the checker decides it over the
//! emitted C, and a shape one accepts and the other flags is a disagreement
//! that has to be a failing test rather than a comment.

use palladium::linker::{link_command, OptLevel};
use palladium::{CompileError, Driver};
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

/// Headline + notes + suggestions, i.e. everything the user is shown.
///
/// `CompileError::to_string()` is only the headline, so asserting over it alone
/// would make every claim about the note and the help pass vacuously.
fn rendered(e: CompileError) -> String {
    let d = e.to_diagnostic();
    let mut out = vec![d.message.clone()];
    out.extend(d.notes.iter().cloned());
    out.extend(d.suggestions.iter().map(|s| s.message.clone()));
    out.join("\n")
}

/// Run `scripts/check-c-returns.py` over C this build just emitted and require
/// exit 0. See the module comment for why an accepted program must pass it.
fn assert_net_a_accounts_for(c_file: &Path, what: &str) {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/check-c-returns.py");
    let out = Command::new("python3")
        .arg(&script)
        .arg(c_file)
        .output()
        .unwrap_or_else(|e| {
            panic!(
                "could not run {} (the generated-C invariant is a hard dependency \
                 of this suite, as it is of `make stdlib-gate`): {}",
                script.display(),
                e
            )
        });
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "the generated-C invariant does not accept the C emitted for `{}`, which \
         the parser DID accept — the two analyses disagree.\n\
         exit {:?} (0 clean, 1 finding, 2 malfunction)\n{}",
        what,
        out.status.code(),
        text
    );
}

/// Compile, check the emitted C against Net A, link against the real runtime,
/// run, and return stdout.
///
/// `link_command` rather than a bare `cc` so the runtime and prelude are
/// resolved exactly as `pdc compile` resolves them.
fn compile_and_run(source: &str, name: &str) -> Result<String, String> {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    let exe = dir.path().join(name);
    fs::write(&src, source).unwrap();

    let c_file = Driver::new()
        .compile_file(&src)
        .map_err(|e| format!("compilation failed: {}", rendered(e)))?;
    assert_net_a_accounts_for(&c_file, name);

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

/// Compile only, and return the rendered diagnostics on failure.
fn compile(source: &str, name: &str) -> Result<(), String> {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    fs::write(&src, source).unwrap();
    Driver::new()
        .compile_file(&src)
        .map(|_| ())
        .map_err(rendered)
}

// ---------------------------------------------------------------------------
// The refusal — one test per path shape the old code let through
// ---------------------------------------------------------------------------

/// The exact program from the defect report.
#[test]
fn an_empty_body_with_a_return_type_is_refused() {
    let err = compile(
        r#"
fn get_value() -> i64 { }

fn main() {
    print_int(get_value());
}
"#,
        "m1mr_empty",
    )
    .expect_err("`fn get_value() -> i64 { }` returned the register's contents");

    assert!(
        err.contains("may return without a value"),
        "must be the missing-return refusal, got:\n{}",
        err
    );
    assert!(
        err.contains("`get_value`"),
        "the message must name the function, got:\n{}",
        err
    );
    assert!(
        err.contains("the body is empty"),
        "the note must say which path has no value, got:\n{}",
        err
    );
}

/// The second shape the old `#[ignore]` reason named: a last statement that
/// returns on one path only. No value is written in tail position anywhere, so
/// the D3b refusal (which keys on a value having been WRITTEN) never fired.
#[test]
fn an_if_with_no_else_as_the_last_statement_is_refused() {
    let err = compile(
        r#"
fn f(n: i64) -> i64 {
    if n > 0 {
        return 1;
    }
}

fn main() {
    print_int(f(3));
}
"#,
        "m1mr_if_returns",
    )
    .expect_err("the false path reaches the closing brace with nothing to return");

    assert!(
        err.contains("may return without a value"),
        "must be the missing-return refusal, got:\n{}",
        err
    );
    assert!(
        err.contains("no `else` branch"),
        "the note must name the path that has no value, got:\n{}",
        err
    );
}

/// The third shape it named: a loop that may not be entered. `while` is not
/// `while true`, so nothing proves the `return` inside it runs.
#[test]
fn a_loop_that_may_not_be_entered_is_refused() {
    let err = compile(
        r#"
fn f(n: i64) -> i64 {
    while n > 0 {
        return 1;
    }
}

fn main() {
    print_int(f(3));
}
"#,
        "m1mr_loop",
    )
    .expect_err("a `while` with a runtime guard may run zero times");

    assert!(
        err.contains("may return without a value"),
        "must be the missing-return refusal, got:\n{}",
        err
    );
    assert!(
        err.contains("reach its closing brace"),
        "the note must say the body can fall off its end, got:\n{}",
        err
    );
}

/// A body that ends in an ordinary `;` statement. The plainest shape of all,
/// and the one a `-> i64` typo produces.
#[test]
fn a_body_ending_in_a_semicolon_statement_is_refused() {
    let err = compile(
        r#"
fn f(n: i64) -> i64 {
    let x: i64 = n * 2;
    print_int(x);
}

fn main() {
    print_int(f(3));
}
"#,
        "m1mr_semi",
    )
    .expect_err("`print_int(x);` is not a value");

    assert!(
        err.contains("may return without a value"),
        "must be the missing-return refusal, got:\n{}",
        err
    );
}

/// The refusal must carry advice, and the advice must be about returning a
/// value. A refusal with no help is how a compiler tells the truth and is still
/// useless.
#[test]
fn the_refusal_names_the_defect_and_offers_a_fix() {
    let err = compile(
        "fn g() -> i64 { }\nfn main() { print_int(g()); }",
        "m1mr_help",
    )
    .expect_err("must be refused");
    assert!(
        err.contains("return register"),
        "the note must say what the old behaviour actually was, got:\n{}",
        err
    );
    assert!(
        err.contains("return a value on every path"),
        "the suggestion must tell the author what to do, got:\n{}",
        err
    );
}

// ---------------------------------------------------------------------------
// The accept side — the programs this refusal must NOT touch
//
// A refusal over-approximates by rejecting valid code, and that failure is
// silent unless something asserts the acceptance. Each of these ran and printed
// the asserted numbers.
// ---------------------------------------------------------------------------

/// A tail expression. The oldest accepted shape (D3) and the one a naive "the
/// last statement must be a `return`" rule rejects first.
#[test]
fn a_tail_expression_is_still_accepted() {
    let out = compile_and_run(
        r#"
fn add(a: i64, b: i64) -> i64 {
    a + b
}

fn main() {
    print_int(add(20, 22));
}
"#,
        "m1mr_ok_tail",
    )
    .expect("a tail expression is the function's value");
    assert_eq!(out.trim(), "42");
}

/// A tail `if`. Legitimate since D3b, and the shape the task brief singles out:
/// `fn f() -> i64 { if c { a } else { b } }`.
#[test]
fn a_tail_if_is_still_accepted() {
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
        "m1mr_ok_tail_if",
    )
    .expect("a tail `if` with both branches valued is the function's value");
    assert_eq!(out.trim(), "111\n222", "each branch returns its own value");
}

/// An early `return` in one branch and a tail expression in the other. This is
/// the case a tail-only rule refuses and a return-only rule refuses too — it
/// needs both halves of the analysis at once.
#[test]
fn an_early_return_in_one_branch_is_still_accepted() {
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
        "m1mr_ok_mixed",
    )
    .expect("one branch returning explicitly is not a fall-through");
    assert_eq!(out.trim(), "30\n-4");
}

/// Explicit `return`s all the way down, with no tail expression anywhere. This
/// is the shape the OLD code could not distinguish from a fall-through, because
/// no value is written in tail position — which is exactly why it did nothing
/// and let the empty body through.
#[test]
fn explicit_returns_on_every_path_are_still_accepted() {
    let out = compile_and_run(
        r#"
fn classify(n: i64) -> i64 {
    if n > 0 {
        return 1;
    } else {
        return 2;
    }
}

fn main() {
    print_int(classify(5));
    print_int(classify(-5));
}
"#,
        "m1mr_ok_returns",
    )
    .expect("both branches return explicitly");
    assert_eq!(out.trim(), "1\n2");
}

/// An early `return` followed by a final `return`. The commonest shape in the
/// bootstrap compiler, and the one that dies if the rule is "the LAST statement
/// must terminate" applied to a body whose last statement is the guard.
#[test]
fn a_guard_clause_before_a_final_return_is_still_accepted() {
    let out = compile_and_run(
        r#"
fn fib(n: i64) -> i64 {
    if n <= 1 {
        return n;
    }
    return fib(n - 1) + fib(n - 2);
}

fn main() {
    print_int(fib(10));
}
"#,
        "m1mr_ok_guard",
    )
    .expect("a guard clause then a final return");
    assert_eq!(out.trim(), "55");
}

/// A body whose last statement is an infinite loop. It has no exit edge, so it
/// needs no value — and the refusal must know that, or `while true { … }`
/// service loops become uncompilable.
#[test]
fn an_infinite_loop_with_no_break_is_still_accepted() {
    let out = compile_and_run(
        r#"
fn spin(c: bool) -> i64 {
    if c {
        return 7;
    }
    while true {
        print("x");
        return 9;
    }
}

fn main() {
    print_int(spin(true));
}
"#,
        "m1mr_ok_spin",
    )
    .expect("an infinite loop cannot fall through to the closing brace");
    assert_eq!(out.trim(), "7");
}

/// THE REACHABILITY RULE, pinned from the accept side.
///
/// `while true { return 42; break; }` is a correct program: the `return` leaves
/// the function, so the `break` is dead text, so the loop has no exit edge, so
/// the body cannot reach its closing brace. `contains_escaping_break` gets this
/// right by stopping its scan at the first statement that cannot fall through.
///
/// A regression from that back to "count every syntactically present `break`"
/// makes this loop look escapable and the whole function look like a
/// fall-through — a REFUSAL OF A VALID PROGRAM, which is the direction that
/// costs a user their day rather than merely a diagnostic. `tests/d3b_tail_if.rs`
/// pins the same rule for a branch tail; this pins it for a whole body, which
/// is the surface the missing-return refusal added.
#[test]
fn an_unreachable_break_does_not_make_the_loop_escapable() {
    let out = compile_and_run(
        r#"
fn f() -> i64 {
    while true {
        return 42;
        break;
    }
}

fn main() {
    print_int(f());
}
"#,
        "m1mr_ok_dead_break",
    )
    .expect("the `break` is unreachable, so the loop has no exit edge");
    assert_eq!(out.trim(), "42");
}

/// The control on the rule above, from the refuse side: a break that IS
/// reachable does let control out, so the body can reach its closing brace.
/// Without this, "reachability-aware" could degenerate into "ignore all breaks"
/// and the accept case above would still pass.
#[test]
fn a_reachable_break_still_makes_the_body_fall_through() {
    let err = compile(
        r#"
fn f(c: bool) -> i64 {
    while true {
        if c {
            break;
        }
        return 42;
    }
}

fn main() {
    print_int(f(true));
}
"#,
        "m1mr_live_break",
    )
    .expect_err("the `break` is reachable, so the loop can be left with no value");
    assert!(
        err.contains("may return without a value"),
        "must be the missing-return refusal, got:\n{}",
        err
    );
}

/// A body that ends in `panic(...)`. `__pd_panic` calls `abort()`, so control
/// does not come back and there is nothing to return.
#[test]
fn a_body_ending_in_panic_is_still_accepted() {
    let out = compile_and_run(
        r#"
fn checked(n: i64) -> i64 {
    if n > 0 {
        return n;
    }
    panic("not positive");
}

fn main() {
    print_int(checked(4));
}
"#,
        "m1mr_ok_panic",
    )
    .expect("a trailing `panic` is a terminator");
    assert_eq!(out.trim(), "4");
}

/// Unit-returning functions are OUTSIDE the rule, under BOTH spellings.
/// `fn f() {}` and `fn f() -> () {}` are one type, and the condition the
/// refusal hangs off (`return_type.is_some()`) is true for the second — so the
/// naive reading rejects a correct program.
#[test]
fn both_spellings_of_a_unit_return_are_still_accepted() {
    let out = compile_and_run(
        r#"
fn shout() {
    print("a");
}

fn shout_explicit() -> () {
    print("b");
}

fn nothing() -> () { }

fn main() {
    shout();
    shout_explicit();
    nothing();
}
"#,
        "m1mr_ok_unit",
    )
    .expect("falling off the end of a unit function is what it is for");
    assert_eq!(out.trim(), "a\nb");
}

/// A tail `match` whose arms all return.
///
/// NOT routed through `compile_and_run`: codegen lowers `match` to an
/// `if`/`else if` chain with no final `else`, so the emitted C can fall past
/// the last arm and Net A is right to flag it. That is the open
/// match-exhaustiveness defect, pinned at `tests/stdlib/DRIVERS.tsv`
/// (`stdlib_tail_match`, `known_violation:area_code,sides`) and by
/// `NetA::StillFindsTheOpenMatchDefect` in `tests/d3b_tail_if.rs` — not this
/// one. What this test states is the part that IS this refusal's business: a
/// tail `match` with every arm valued must not be refused by the parser.
#[test]
fn a_tail_match_with_every_arm_valued_is_still_accepted() {
    compile(
        r#"
enum Shape {
    Circle,
    Square,
}

fn sides(s: Shape) -> i64 {
    match s {
        Shape::Circle => 0,
        Shape::Square => 4,
    }
}

fn main() {
    print_int(sides(Shape::Square));
}
"#,
        "m1mr_ok_match",
    )
    .expect("every arm has a value, so the parser must accept it");
}

// ---------------------------------------------------------------------------
// The boundary between this refusal and the D3b one
// ---------------------------------------------------------------------------

/// The two refusals must not collapse into each other. A value written in tail
/// position with a valueless sibling is still D3b's `Unimplemented` refusal
/// (which names the construct and suggests the missing `else`), NOT the
/// missing-return one — the messages are different because the situations are:
/// one has evidence of what the author meant, the other only has the signature.
#[test]
fn a_written_tail_value_still_gets_the_d3b_refusal() {
    let err = compile(
        r#"
fn f(n: i64) -> i64 {
    if n > 0 { n }
}

fn main() {
    print_int(f(3));
}
"#,
        "m1mr_d3b_boundary",
    )
    .expect_err("a tail `if` with no `else` has no value on the false path");

    assert!(
        err.contains("is not implemented") && err.contains("tail `if`"),
        "this shape belongs to the D3b refusal, got:\n{}",
        err
    );
    assert!(
        !err.contains("may return without a value"),
        "the missing-return refusal must not shadow the more specific one, got:\n{}",
        err
    );
}
