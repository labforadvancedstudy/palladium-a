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
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

/// Run the full driver over `source`, returning the path of the emitted C.
fn compile_to_c_path(source: &str, name: &str) -> Result<PathBuf, String> {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    fs::write(&src, source).unwrap();
    Driver::new().compile_file(&src).map_err(rendered)
}

/// Run the full driver over `source`, returning the emitted C file's contents.
///
/// Every accepted program also goes through the generated-C invariant; see
/// [`assert_net_a_accounts_for`].
fn compile_to_c(source: &str, name: &str) -> Result<String, String> {
    compile_to_c_expecting(source, name, NetA::Accepts)
}

fn compile_to_c_expecting(source: &str, name: &str, expect: NetA) -> Result<String, String> {
    let c_file = compile_to_c_path(source, name)?;
    assert_net_a(&c_file, name, expect);
    fs::read_to_string(&c_file).map_err(|e| format!("reading {}: {}", c_file.display(), e))
}

/// Run `scripts/check-c-returns.py` over C THIS BUILD just emitted.
///
/// WHY THIS EXISTS, AND WHY IT IS THE MOST IMPORTANT ASSERTION IN THIS FILE
/// The parser decides "does this branch fall through?" over Palladium
/// statements; `scripts/check-c-returns.py` decides the same question over the
/// C those statements become. Two hand-written analyses of one property drift,
/// and their drift is silent in both directions: a shape the parser accepts and
/// the checker flags turns a gate red on valid code, and the reverse ships a
/// miscompile under a green gate. Round 1 kept them in step by DISCIPLINE — a
/// comment naming each pair — and round 2 found the same defect in both at
/// once (an unreachable `break` counted as an escape), which is exactly what
/// discipline buys you.
///
/// So agreement is no longer asserted, it is EXECUTED: every program these
/// tests accept is handed to the real checker, as a process, with its exit
/// status read. A disagreement is a failing test rather than a claim, and any
/// NEW shape codegen starts emitting turns this red too — which is the thing
/// that makes "the generator's invariants are enforced" falsifiable.
///
/// Exit codes are the checker's own taxonomy: 0 clean, 1 a finding, 2 a
/// malfunction. Only 0 is acceptable for a program the parser accepted —
/// except for the one declared, owned disagreement below.
#[derive(Clone, Copy)]
enum NetA {
    /// The parser and the checker agree: the emitted C returns on every path.
    Accepts,
    /// THE ONE DECLARED DISAGREEMENT, and it is an OPEN DEFECT, not an excuse.
    ///
    /// The parser accepts a tail `match` whose arms all return. Codegen lowers
    /// `match` to an `if`/`else if` chain with NO FINAL `else`
    /// (src/codegen/mod.rs), so the emitted C can fall past the last arm and
    /// the checker is right to flag it — gcc's `-Wreturn-type` agrees. The
    /// defect is match exhaustiveness, tracked separately from D3b.
    ///
    /// It is spelled as a REQUIRED finding rather than an exemption so that it
    /// cannot go stale: when the final `else` lands, the checker returns 0, THIS
    /// ASSERTION FAILS, and whoever fixed it is told what to update. That is the
    /// same XPASS handoff scripts/conformance.sh uses.
    ///
    /// THE HANDOFF, BY NAME — the rows that change together when the final
    /// `else` is emitted, so nobody has to go looking:
    ///
    ///   1. `tests/stdlib/DRIVERS.tsv`, row `stdlib_tail_match`: column 3 is
    ///      `known_violation:area_code,sides`; promote it to `clean`.
    ///      `make stdlib-gate` announces that transition itself — it prints
    ///      "XPASS: … is recorded known_violation:… but its C is now CLEAN"
    ///      (scripts/stdlib-gate.sh) and stays red until the row is promoted.
    ///   2. This expectation: `StillFindsTheOpenMatchDefect` -> `Accepts`, at
    ///      its single use in `tail_match_arms_are_lowered_to_returns`.
    ///   3. `src/parser/mod.rs`, the NOTE inside `returns_on_every_path` that
    ///      records this residual — it stops being a residual.
    ///
    /// `tests/conformance-manifest.txt` needs NO change: its
    /// `tests/stdlib/stdlib_tail_match.pd` row is `run`/`expected` and pins the
    /// VALUES the program prints, which a final `else` does not alter.
    StillFindsTheOpenMatchDefect,
}

fn assert_net_a(c_file: &Path, what: &str, expect: NetA) {
    match expect {
        NetA::Accepts => assert_net_a_accounts_for(c_file, what),
        NetA::StillFindsTheOpenMatchDefect => {
            let (code, text) = net_a_verdict(c_file);
            assert_eq!(
                code,
                Some(1),
                "`{}` emits a tail `match`, whose C has no final `else`, so the \
                 generated-C invariant must still report a finding. If it now \
                 returns 0 the missing-`else` defect is FIXED: change this \
                 expectation to NetA::Accepts (and retire the `known_violation` \
                 pin on tests/stdlib/stdlib_tail_match.pd). Got exit {:?}:\n{}",
                what,
                code,
                text
            );
            assert!(
                text.contains("FINDING") && text.contains("may fall off its end"),
                "exit 1 must be corroborated by the well-formed finding, not by \
                 arbitrary output:\n{}",
                text
            );
        }
    }
}

fn net_a_verdict(c_file: &Path) -> (Option<i32>, String) {
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
    (
        out.status.code(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

fn assert_net_a_accounts_for(c_file: &Path, what: &str) {
    let (code, text) = net_a_verdict(c_file);
    assert_eq!(
        code,
        Some(0),
        "the generated-C invariant does not accept the C emitted for `{}`, which \
         the parser DID accept — the two analyses disagree.\n\
         exit {:?} (0 clean, 1 finding, 2 malfunction)\n{}",
        what,
        code,
        text
    );
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
    compile_and_run_expecting(source, name, NetA::Accepts)
}

fn compile_and_run_expecting(
    source: &str,
    name: &str,
    expect: NetA,
) -> Result<String, String> {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    let exe = dir.path().join(name);
    fs::write(&src, source).unwrap();

    let c_file = Driver::new()
        .compile_file(&src)
        .map_err(|e| format!("compilation failed: {}", rendered(e)))?;
    assert_net_a(&c_file, name, expect);

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

/// A branch that ends in `while true { … }` with no `break` never reaches the
/// closing brace, so it needs no value — exactly like the `panic` case above,
/// and for the same reason.
///
/// MEASURED AT 199c7bd, BEFORE THIS FIX: refused with "a tail `if` that
/// produces a value on some paths but not all". That is a correct program
/// rejected, and the refusal's own justification ("nothing correct is lost")
/// was therefore false.
///
/// The two analyses have to agree about this shape. `while true` emits
/// `while (1)` (src/codegen/mod.rs, `Expr::Bool`), which is the exact spelling
/// `scripts/check-c-returns.py` treats as an infinite loop, so the C this emits
/// passes the generated-C invariant as well as gcc's `-Wreturn-type`.
#[test]
fn a_branch_that_loops_forever_is_not_refused() {
    let c = compile_to_c(
        r#"
fn pick(c: bool) -> i64 {
    if c {
        1
    } else {
        while true {
            print("spinning");
        }
    }
}

fn main() {
    print_int(pick(true));
}
"#,
        "d3b_infinite",
    )
    .expect("an `else` that cannot fall through needs no value");
    assert!(
        c.contains("while (1)"),
        "`while true` must emit the literal `while (1)` that \
         scripts/check-c-returns.py recognises:\n{}",
        c
    );
    assert!(
        c.contains("return 1;"),
        "the valued branch must still be lowered to a return:\n{}",
        c
    );
}

/// The other half of that judgement: a loop that CAN be left falls through, so
/// the same shape with a `break` must still be refused. Without this the fix
/// above would read "any loop terminates", which is how a real fall-through
/// gets cleared.
#[test]
fn a_branch_whose_loop_can_break_out_is_still_refused() {
    let err = compile_to_c(
        r#"
fn pick(c: bool) -> i64 {
    if c { 1 } else { while true { break; } }
}

fn main() {
    print_int(pick(true));
}
"#,
        "d3b_breakable",
    )
    .expect_err("a `break` reaches the closing brace with no value");
    assert!(
        err.contains("the `else` branch"),
        "the note must name the branch that falls through, got:\n{}",
        err
    );
}

/// A `break` written inside a NESTED loop belongs to that loop, so the outer
/// one is still inescapable. This mirrors `contains_break`'s depth rule in
/// `scripts/check-c-returns.py`; a version that just grepped for `break` would
/// refuse this program.
#[test]
fn a_break_in_a_nested_loop_does_not_escape_the_outer_one() {
    let c = compile_to_c(
        r#"
fn pick(c: bool) -> i64 {
    if c {
        1
    } else {
        while true {
            while true {
                break;
            }
        }
    }
}

fn main() {
    print_int(pick(true));
}
"#,
        "d3b_nested_break",
    )
    .expect("the inner `break` binds to the inner loop");
    assert!(c.contains("return 1;"), "{}", c);
}

/// A `break` written after a `return` never runs, so it does not give the loop
/// an exit edge and the branch still cannot fall through.
///
/// MEASURED AT fcbabca, BEFORE THIS FIX: refused. Round 1 made
/// `already_terminates` reachability-aware but that reachability did not reach
/// into break detection — the same defect one level down, in both analyses at
/// once.
#[test]
fn a_branch_whose_only_break_is_unreachable_is_not_refused() {
    let out = compile_and_run(
        r#"
fn pick(c: bool) -> i64 {
    if c { 1 } else { while true { return 2; break; } }
}

fn main() {
    print_int(pick(true));
    print_int(pick(false));
}
"#,
        "d3b_unreachable_break",
    )
    .expect("a `break` after a `return` cannot escape the loop");
    assert_eq!(out.trim(), "1\n2");
}

/// The order matters, and this is the guard: with the `break` FIRST it is
/// reachable, the loop can be left, and the branch does fall through. A fix
/// that just ignored breaks near returns would accept this too.
#[test]
fn a_reachable_break_before_a_return_still_escapes() {
    let err = compile_to_c(
        r#"
fn pick(c: bool) -> i64 {
    if c { 1 } else { while true { break; return 2; } }
}

fn main() {
    print_int(pick(true));
}
"#,
        "d3b_reachable_break",
    )
    .expect_err("the `break` runs on the first iteration");
    assert!(
        err.contains("the `else` branch"),
        "the note must name the branch that falls through, got:\n{}",
        err
    );
}

/// A `break` under a nested `if` is still ours: an `if` is not a loop, so the
/// break binds to the enclosing `while`. `contains_escaping_break` recurses
/// through both arms, mirroring `contains_break`'s treatment of a compound with
/// an `if` header.
#[test]
fn a_break_under_a_nested_if_still_escapes_the_loop() {
    let err = compile_to_c(
        r#"
fn pick(c: bool, d: bool) -> i64 {
    if c {
        1
    } else {
        while true {
            if d {
                break;
            }
        }
    }
}

fn main() {
    print_int(pick(true, true));
}
"#,
        "d3b_break_under_if",
    )
    .expect_err("a break under an `if` still leaves the loop");
    assert!(err.contains("the `else` branch"), "{}", err);
}

/// `unsafe { … }` is a scope, not a loop or a function: a `return` inside it
/// terminates the enclosing branch, and a `break` inside it still escapes the
/// enclosing loop. Both sides model it as a bare block — codegen emits exactly
/// that (`// unsafe block` then `{`), so `check-c-returns.py` reads it through
/// its `h in ("", "do")` case.
#[test]
fn an_unsafe_block_that_returns_terminates_its_branch() {
    let out = compile_and_run(
        r#"
fn pick(c: bool) -> i64 {
    if c {
        1
    } else {
        unsafe {
            return 2;
        }
    }
}

fn main() {
    print_int(pick(true));
    print_int(pick(false));
}
"#,
        "d3b_unsafe_returns",
    )
    .expect("a `return` inside an `unsafe` block still returns");
    assert_eq!(out.trim(), "1\n2");
}

#[test]
fn a_break_inside_an_unsafe_block_still_escapes_the_loop() {
    let err = compile_to_c(
        r#"
fn pick(c: bool) -> i64 {
    if c {
        1
    } else {
        while true {
            unsafe {
                break;
            }
        }
    }
}

fn main() {
    print_int(pick(true));
}
"#,
        "d3b_unsafe_break",
    )
    .expect_err("an `unsafe` scope does not capture a `break`");
    assert!(err.contains("the `else` branch"), "{}", err);
}

/// THE AGREEMENT BOUNDARY, PINNED. Only the literal `while true` is treated as
/// infinite, because only it emits the literal `while (1)` that
/// `scripts/check-c-returns.py` recognises. `while 1 == 1` emits
/// `while ((1 == 1))`, which neither side calls infinite — so the program is
/// refused here rather than accepted here and flagged there.
///
/// This is a deliberate limit, not an oversight: widening one side without the
/// other is how the two analyses drift apart.
#[test]
fn a_loop_that_is_infinite_but_not_spelled_true_is_refused() {
    let err = compile_to_c(
        r#"
fn pick(c: bool) -> i64 {
    if c { 1 } else { while 1 == 1 { print("spinning"); } }
}

fn main() {
    print_int(pick(true));
}
"#,
        "d3b_not_literal_true",
    )
    .expect_err("only the literal `while true` is modelled as infinite");
    assert!(err.contains("the `else` branch"), "{}", err);
}

/// Statements written after a `return` are unreachable, so the branch they are
/// in cannot fall through either.
///
/// MEASURED AT 199c7bd: refused, because the termination test looked only at
/// the LAST statement of the branch. Both sides of code generation now scan the
/// whole list (`already_terminates` here, `terminates` in
/// scripts/check-c-returns.py) — they must agree, or a program this side
/// accepts is flagged by that one.
///
/// The residual is stated rather than papered over: unreachability that needs a
/// condition to be evaluated (`if false { … }`) is modelled by neither side.
#[test]
fn a_branch_with_unreachable_code_after_a_return_is_not_refused() {
    let out = compile_and_run(
        r#"
fn mixed(n: i64) -> i64 {
    if n > 0 {
        return n * 10;
        print_int(999);
    } else {
        n - 1
    }
}

fn main() {
    print_int(mixed(3));
    print_int(mixed(-3));
}
"#,
        "d3b_unreachable",
    )
    .expect("statements after a `return` do not make the branch fall through");
    assert_eq!(out.trim(), "30\n-4");
}

// ---------------------------------------------------------------------------
// Tail `match` — the same defect, through the `pattern => expression,` arm
// ---------------------------------------------------------------------------

/// Measured before the fix: `area(Square)` printed -16.
///
/// This is the only match arm form that can carry a value; a block-bodied arm
/// parses with `parse_statement` and so demands a `;`, making `Circle => { 1 }`
/// a parse error.
///
/// AND IT IS THE ONE PLACE THE TWO ANALYSES DISAGREE — declared, not exempted.
/// The parser lowers every arm to a `return`; codegen lowers the `match` to an
/// `if`/`else if` chain with no final `else`, so the emitted C can still fall
/// past the last arm and `scripts/check-c-returns.py` correctly says so. That
/// is the open missing-`else` defect, separate from D3b. See
/// `NetA::StillFindsTheOpenMatchDefect` for why this is spelled as a REQUIRED
/// finding: when the defect is fixed, this line goes red and asks to be changed
/// rather than rotting into a silent exemption.
#[test]
fn tail_match_arms_are_lowered_to_returns() {
    let out = compile_and_run_expecting(
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
        NetA::StillFindsTheOpenMatchDefect,
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
/// emit `return __pd_print_int(n);` from a `void` function.
///
/// Be precise about what that costs, because an earlier version of this comment
/// said "which gcc rejects" and the test two screens down measures the
/// opposite: returning a VOID expression from a void function is a constraint
/// violation that both gcc and clang accept as an extension (see
/// `an_explicit_unit_return_type_with_a_unit_tail_still_runs`, which compiles,
/// links and runs exactly that C). So this is not a build failure waiting to
/// happen — it is C nobody should be emitting on purpose, and the guard below
/// is on the `return_type.is_some()` condition that keeps the OMITTED-return-
/// type case out of the lowering.
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

/// The guard above is `return_type.is_some()`, so it tests an OMITTED return
/// type. An explicit `-> ()` is `Some(Type::Unit)` and DOES enter the lowering.
/// These three tests pin what actually happens, because "it is guarded" was not
/// true of the explicit spelling.
///
/// (a) `-> ()` with a unit-valued tail: lowered, and the emitted C returns a
/// void expression from a void function. gcc and clang both accept that
/// (returning a void expression from a void function is a documented
/// extension), and the program runs correctly — verified here by running it,
/// not by reading the C.
#[test]
fn an_explicit_unit_return_type_with_a_unit_tail_still_runs() {
    let out = compile_and_run(
        r#"
fn f(n: i64) -> () {
    if n > 0 { print_int(n) } else { print_int(0) }
}

fn main() {
    f(3);
    f(-3);
}
"#,
        "d3b_explicit_unit",
    )
    .expect("an explicit `-> ()` function must compile, link and run");
    assert_eq!(out.trim(), "3\n0");
}

/// (b) `-> ()` with a NON-unit tail. The lowering produces `return 5;` in a
/// function declared to return nothing, and the semantic pass — not the C
/// compiler — is what refuses it. This is the check that would be lost if the
/// parser skipped lowering for `Some(Type::Unit)`: a bare expression statement
/// would silently discard the value instead.
#[test]
fn an_explicit_unit_return_type_with_a_valued_tail_is_a_type_error() {
    let err = compile_to_c(
        r#"
fn f() -> () {
    5
}

fn main() {
    f();
}
"#,
        "d3b_explicit_unit_valued",
    )
    .expect_err("`-> ()` cannot return an i64");
    assert!(
        err.contains("expected ()") && err.contains("found Int"),
        "the type checker must catch this before codegen, got:\n{}",
        err
    );
}

/// (c) The mirror: a unit-valued tail in a NON-unit function. The parser
/// happily lowers `print_int(n)` to a `return`, and again it is the semantic
/// pass that rejects it — before any C is emitted.
#[test]
fn a_unit_valued_tail_in_a_non_unit_function_is_a_type_error() {
    let err = compile_to_c(
        r#"
fn f(n: i64) -> i64 {
    if n > 0 { print_int(n) } else { print_int(0) }
}

fn main() {
    print_int(f(3));
}
"#,
        "d3b_unit_tail_in_i64",
    )
    .expect_err("`print_int` produces no value for an i64 function to return");
    assert!(
        err.contains("expected Int") && err.contains("found ()"),
        "the type checker must catch this before codegen, got:\n{}",
        err
    );
}

// ---------------------------------------------------------------------------
// The exact C spellings the generated-C invariant depends on
// ---------------------------------------------------------------------------

/// `scripts/check-c-returns.py` recognises constructs by their TEXT. Its
/// docstring names the codegen sites those spellings come from, but a comment
/// cannot go red: change `Expr::Bool` to emit `true` (with `<stdbool.h>`) and
/// every `while true` silently stops being an infinite loop to that reader,
/// while the parser goes on treating it as one. That is a silent divergence in
/// the dangerous direction, and this test is what stops it.
///
/// Each assertion below names the regex or branch in the checker that consumes
/// the spelling.
#[test]
fn codegen_spellings_the_generated_c_invariant_depends_on() {
    let c = compile_to_c(
        r#"
fn spin(c: bool) -> i64 {
    if c {
        1
    } else {
        while true {
            unsafe {
                print("x");
            }
        }
    }
}

fn checked(n: i64) -> i64 {
    if n > 0 { n } else { panic("no"); }
}

fn nearly(c: bool) {
    while 1 == 1 {
        print("y");
    }
}

fn main() {
    print_int(spin(true));
    print_int(checked(1));
}
"#,
        "d3b_spellings",
    )
    .expect("must compile");

    // `while true` -> `while (1)`: the `while\s*\(\s*1\s*\)` case of
    // item_terminates(). This is the whole infinite-loop agreement.
    assert!(
        c.contains("while (1) {"),
        "`while true` must emit `while (1)`:\n{}",
        c
    );
    // MEASURED, AND NOT WHAT AN EARLIER COMMENT IN THIS REPO CLAIMED:
    // `while 1 == 1` ALSO emits `while (1)`, because constant folding runs
    // before code generation and rewrites the comparison to `Expr::Bool(true)`
    // (src/optimizer/constant_folding.rs, `BinOp::Eq`). The two analyses
    // therefore disagree about this program — but in the SAFE direction: the
    // parser reads the UNFOLDED ast, does not call the loop infinite, and
    // REFUSES the program (see
    // `a_loop_that_is_infinite_but_not_spelled_true_is_refused`), so the
    // checker never gets a chance to be more permissive on a program that
    // shipped. A refusal costs a valid program; the reverse would cost a
    // miscompile.
    assert_eq!(
        c.matches("while (1) {").count(),
        2,
        "constant folding turns `while 1 == 1` into `while (1)` too; if this \
         ever becomes 1, the parser and the checker have swapped which of them \
         is stricter and the direction of the divergence must be re-judged:\n{}",
        c
    );
    // `panic(...)` -> `__pd_panic(`: NORETURN_RE.
    assert!(
        c.contains("__pd_panic("),
        "`panic` must emit `__pd_panic(` so NORETURN_RE matches:\n{}",
        c
    );
    // `unsafe { … }` -> a bare block: the `h in ("", "do")` case.
    assert!(
        c.contains("// unsafe block"),
        "`unsafe` must emit a bare block:\n{}",
        c
    );
    // A `return` statement starts the line: RETURN_RE is anchored.
    assert!(
        c.lines().any(|l| l.trim_start().starts_with("return ")),
        "a return must start its statement: {}",
        c
    );
    // THE STRUCTURAL ONE: every definition's `{` is the LAST character of its
    // line. check_file()'s top-level scan refuses any other shape outright, so
    // a codegen change here does not merely weaken the reader — it stops it.
    for line in c.lines() {
        let is_def = !line.starts_with(char::is_whitespace)
            && line.contains('(')
            && line.contains(')')
            && line.contains('{')
            && !line.trim_start().starts_with("//");
        if is_def {
            assert!(
                line.ends_with('{'),
                "a definition whose `{{` is not last on its line is refused by \
                 scripts/check-c-returns.py: {:?}",
                line
            );
        }
    }
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
