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

/// Compile a two-file program: `lib.pd` imported by `app.pd`.
///
/// A SUBPROCESS, not `Driver`, and not because it is tidier: the resolver looks
/// for `<module>.pd` beside the file being compiled and codegen writes into
/// `build_output/` relative to the CURRENT DIRECTORY, so an in-process test
/// would have to change the working directory of a process whose other tests
/// are running in parallel threads. `pdc` with `current_dir` set has no such
/// race.
///
/// -> Ok(stdout of the built binary) | Err(the compiler's diagnostics)
fn compile_and_run_with_import(lib: &str, app: &str, name: &str) -> Result<String, String> {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("lib.pd"), lib).unwrap();
    fs::write(dir.path().join("app.pd"), app).unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_pdc"))
        .args(["compile", "app.pd", "-o", name])
        .current_dir(dir.path())
        .output()
        .expect("failed to run pdc");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    if !out.status.success() {
        return Err(text);
    }
    let exe = dir.path().join("build_output").join(name);
    let run = Command::new(&exe)
        .output()
        .map_err(|e| format!("running {}: {}", exe.display(), e))?;
    // THE EXIT STATUS IS PART OF THE RESULT. Without this a binary could print
    // exactly the expected stdout and then exit nonzero, and every caller that
    // compares stdout would pass — an assertion narrower than its label, in the
    // helper that carries this file's import claims.
    if !run.status.success() {
        return Err(format!(
            "{} exited {:?}: {}",
            exe.display(),
            run.status.code(),
            String::from_utf8_lossy(&run.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&run.stdout).to_string())
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
    ///   1. `tests/stdlib/DRIVERS.tsv:31`, row `stdlib_tail_match`: column 3 is
    ///      `known_violation:area_code,sides`; promote it to `clean`.
    ///      `make stdlib-gate` announces that transition itself — the
    ///      known_violation branch is scripts/stdlib-gate.sh:379-381, the XPASS
    ///      note it prints when the C goes clean is
    ///      scripts/stdlib-gate.sh:390, and a CHANGED violation set (a
    ///      different function list) is scripts/stdlib-gate.sh:398. Both are
    ///      `note`, which is what makes that gate red.
    ///   2. This expectation: `StillFindsTheOpenMatchDefect` -> `Accepts`, at
    ///      its single use in `tail_match_arms_are_lowered_to_returns`.
    ///   3. `src/parser/mod.rs`, the NOTE inside `returns_on_every_path` that
    ///      records this residual — it stops being a residual.
    ///   4. AND THEN THE FLAG. `-Werror=return-type` is deliberately absent
    ///      from the shared gcc invocation (src/linker.rs:73-86 passes only
    ///      `-O<n>`, `-I <runtime>`, `-o`) for ONE reason: this defect would
    ///      fail every compilation today. That is a temporary position, not a
    ///      steady state. Once the final `else` lands, the flag belongs in that
    ///      invocation, and Net A's role changes with it — from the primary
    ///      structural boundary to ATTRIBUTION, which is what it is better at
    ///      (it names the function and the line without needing a compiler, and
    ///      it runs on C that never links). Do not add the flag before the
    ///      `else`; do not leave it out after. THAT OBLIGATION IS TRACKED, not
    ///      merely written here: `the_linker_will_ask_gcc_to_reject_a_function_
    ///      that_falls_off_its_end` in this file is an #[ignore]d XFAIL with a
    ///      row in tests/rust-debt-manifest.txt, so `make test-xfail` requires
    ///      it to keep existing and the two declarations to agree.
    ///
    /// `tests/conformance-manifest.txt` needs NO change: its
    /// `tests/stdlib/stdlib_tail_match.pd` row is `run`/`expected` and pins the
    /// VALUES the program prints, which a final `else` does not alter.
    ///
    /// The payload is the name of the ONE C function expected to carry the
    /// finding. It is not decoration: without it, the assertion passed on any
    /// exit-1 output containing a generic fall-through message, so the tail-
    /// `match` defect could be fixed while some other function supplied a
    /// finding — and the handoff built to demand a transition would have stayed
    /// green and demanded nothing. An assertion whose predicate is broader than
    /// the property it names is the same defect as a check whose scope is.
    StillFindsTheOpenMatchDefect(&'static str),
}

fn assert_net_a(c_file: &Path, what: &str, expect: NetA) {
    match expect {
        NetA::Accepts => assert_net_a_accounts_for(c_file, what),
        NetA::StillFindsTheOpenMatchDefect(func) => {
            let (code, text) = net_a_verdict(c_file);
            assert_eq!(
                code,
                Some(1),
                "`{}` emits a tail `match`, whose C has no final `else`, so the \
                 generated-C invariant must still report a finding. If it now \
                 returns 0 the missing-`else` defect is FIXED — follow the \
                 handoff on NetA::StillFindsTheOpenMatchDefect. Got exit {:?}:\n{}",
                what,
                code,
                text
            );
            if let Err(why) = declared_match_finding(&text, func) {
                panic!("`{}`: {}\n{}", what, why, text);
            }
        }
    }
}

/// Is `text` EXACTLY the declared tail-`match` finding for `func`?
///
/// Split out from the assertion so it can itself be tested. The predicate used
/// to be "the output contains `FINDING` and the words `may fall off its end`",
/// which is broader than the property it names: the declared defect could be
/// fixed while a DIFFERENT function supplied a finding, and the handoff built to
/// demand a transition would have stayed green and demanded nothing. An
/// assertion whose predicate is broader than its property is the same defect as
/// a check whose scope is.
fn declared_match_finding(text: &str, func: &str) -> Result<(), String> {
    let findings: Vec<&str> = text.lines().filter(|l| l.starts_with("FINDING ")).collect();
    // Exactly one. A second finding is a NEW defect sheltering behind an old
    // excuse — what scripts/conformance.sh's diagnostic fingerprints exist to
    // prevent, applied here.
    if findings.len() != 1 {
        return Err(format!(
            "expected exactly one finding (the tail `match` in `{}`), got {}",
            func,
            findings.len()
        ));
    }
    let f = findings[0];
    if !f.contains("may fall off its end") {
        return Err("exit 1 must be corroborated by the well-formed finding, not \
                    by arbitrary output"
            .to_string());
    }
    if !f.contains(&format!(" {}(", func)) {
        return Err(format!(
            "the finding must name `{}`, the function whose tail `match` this \
             expectation is declared for; it names something else",
            func
        ));
    }
    Ok(())
}

/// The control on the predicate above. Each case is a way the old, broader
/// predicate stayed green while the property it claimed had stopped holding.
#[test]
fn the_declared_match_finding_predicate_is_exact() {
    let real = "FINDING build_output/d3b_match.c:315: non-void function may fall \
                off its end (no return on every path): long long area(struct Shape s) {\n\
                ACCOUNTED build_output/d3b_match.c: 45 definition(s) analysed\n";
    assert!(declared_match_finding(real, "area").is_ok());

    // The declared defect is fixed, but another function falls through. The old
    // predicate ("contains FINDING and the words") accepted this.
    let other = "FINDING x.c:9: non-void function may fall off its end (no return \
                 on every path): long long unrelated(long long n) {\n";
    assert!(
        declared_match_finding(other, "area").is_err(),
        "a finding about another function must not satisfy this expectation"
    );

    // Two findings: the declared one plus a new defect hiding behind it.
    let two = format!("{}{}", real, other);
    assert!(
        declared_match_finding(&two, "area").is_err(),
        "a second finding must not shelter behind the declared one"
    );

    // Exit 1 with no well-formed finding at all.
    assert!(declared_match_finding("FINDING nonsense\n", "area").is_err());
    assert!(declared_match_finding("", "area").is_err());
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
/// infinite BY THE PARSER, which reads the AST before the optimizer runs, so
/// `1 == 1` is still a comparison when it looks. The program is refused.
///
/// AN EARLIER VERSION OF THIS COMMENT went on to say that this loop emits a
/// comparison in the C and that neither side calls it infinite. That is false.
/// It was measured false in this same file —
/// `codegen_spellings_the_generated_c_invariant_depends_on` counts TWO
/// `while (1) {` in one program — and the contradiction survived a prose audit
/// whose receipt was "I looked". The audit is now a check
/// (scripts/test-gate-probe.sh, "retracted claims may not be reasserted"),
/// which is what found this line.
///
/// What actually happens: constant folding rewrites the comparison to `true`
/// (src/optimizer/constant_folding.rs:154, the `BinOp::Eq` arm, which assigns
/// `*expr = Expr::Bool(*l == *r)`) before code generation, so
/// the emitted C is `while (1)` and `scripts/check-c-returns.py` WOULD call it
/// infinite. The two analyses disagree here — in the SAFE direction, because
/// the parser is the stricter one and refuses the program before any C exists.
/// A refusal costs a valid program; the reverse costs a miscompile.
///
/// The limit is deliberate: teaching the parser to fold constants, or the
/// checker to distrust the folded spelling, moves one side without the other,
/// and that is how the two drift apart.
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
        NetA::StillFindsTheOpenMatchDefect("area"),
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

/// THE THREE BOUNDARIES OF THE UNIT RETURN TYPE, PINNED IN THE EMITTED C.
///
/// The two spellings of "returns nothing" used to generate different C, and one
/// of them generated C that is not legal:
///
///     fn f() { print_int(7) }        ->  void f() { __pd_print_int(7); }
///     fn f() -> () { print_int(7) }  ->  void f() { return __pd_print_int(7); }
///
/// `return <expression>;` in a `void` function is a C11 6.8.6.4p1 constraint
/// violation that gcc and clang accept as an extension, so nothing objected —
/// and no `.pd` file in this repository wrote `-> ()` BEFORE the regression
/// fixture added for it (tests/misc/unit_return_spellings.pd), which is why it
/// survived eight rounds of review of this exact area. That fixture is now the
/// one tracked user of the spelling, and it is in the conformance corpus.
///
/// The test below asserts the C, not the exit code, for all three cases: an
/// OMITTED return type, an explicit `-> ()`, and a NON-UNIT return. Asserting
/// the exit code is what let the divergence hide — both spellings compiled.
#[test]
fn the_two_spellings_of_the_unit_return_type_generate_the_same_shape() {
    let omitted = compile_to_c(
        "fn f() { print_int(7) }\n\nfn main() { f(); }\n",
        "d3b_unit_omitted",
    )
    .expect("an omitted return type must compile");
    let annotated = compile_to_c(
        "fn f() -> () { print_int(7) }\n\nfn main() { f(); }\n",
        "d3b_unit_annotated",
    )
    .expect("an explicit `-> ()` must compile");

    for (what, c) in [("omitted", &omitted), ("-> ()", &annotated)] {
        assert!(
            c.contains("void f() {"),
            "`{}` must give f a void C return type:\n{}",
            what,
            c
        );
        // THE DEFECT, stated as the assertion: no void function may return a
        // value-bearing expression.
        assert!(
            !c.contains("return __pd_print_int"),
            "`{}` emits `return <void expression>;` from a void function, a C \
             constraint violation gcc merely tolerates:\n{}",
            what,
            c
        );
        assert!(
            c.contains("    __pd_print_int(7);"),
            "`{}` must still EVALUATE the tail expression — it is there for its \
             effect:\n{}",
            what,
            c
        );
    }

    // What still differs, said exactly rather than claimed away: the annotated
    // form keeps the `Stmt::Return` the lowering produced, so codegen emits a
    // bare `return;` after the expression. That is inert C and it is the price
    // of keeping the type-checker diagnostic in the third case below.
    assert!(
        annotated.contains("    __pd_print_int(7);\n    return;\n"),
        "the annotated form should evaluate then return nothing:\n{}",
        annotated
    );

    // AND `main`, WHICH WAS A NAME-KEYED EXCEPTION TO THE FIX.
    //
    // `Type::Unit` maps to C `void`, but `main` is then rewritten to `int`, and
    // the first version of the fix set its flag with `&& name != "main"` — so
    // the one function every program has kept emitting
    // `return <void expression>;`, now from a function returning `int`, which
    // gcc REFUSES rather than tolerates:
    //
    //     error: returning 'void' from a function with incompatible result
    //            type 'int'
    //
    // Both spellings of `main` must give the same shape, and it is `return 0;`
    // because that is what C `main` returns.
    let main_omitted = compile_to_c(
        "fn main() { print_int(7) }\n",
        "d3b_unit_main_omitted",
    )
    .expect("an omitted return type on main must compile");
    let main_annotated = compile_to_c(
        "fn main() -> () { print_int(7) }\n",
        "d3b_unit_main_annotated",
    )
    .expect("`fn main() -> ()` must compile — it did not; gcc refused the C");
    for (what, c) in [("main omitted", &main_omitted), ("main -> ()", &main_annotated)] {
        assert!(
            !c.contains("return __pd_print_int"),
            "`{}` returns a void expression from `int main`, which gcc refuses \
             outright:\n{}",
            what,
            c
        );
        assert!(
            c.contains("    __pd_print_int(7);\n    return 0;\n"),
            "`{}` must evaluate the tail and return 0 from C main:\n{}",
            what,
            c
        );
    }

    // The same one step down: a bare `return;` written in a unit `main` was
    // emitted as `return;` from `int main` — measured, three gcc errors.
    let main_bare_return = compile_to_c(
        "fn main() { return; }\n",
        "d3b_unit_main_bare_return",
    )
    .expect("a bare return in main must compile");
    assert!(
        !main_bare_return.contains("    return;\n"),
        "a bare `return;` from C `int main` is a constraint violation:\n{}",
        main_bare_return
    );

    // A NON-UNIT return type is untouched: its tail is still lowered to a
    // value-bearing return.
    let valued = compile_to_c(
        "fn g(n: i64) -> i64 { n + 1 }\n\nfn main() { print_int(g(1)); }\n",
        "d3b_unit_nonunit",
    )
    .expect("a non-unit return type must compile");
    assert!(
        valued.contains("return (n + 1);"),
        "a non-unit tail must still be lowered to a value-bearing return:\n{}",
        valued
    );
}

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

/// `-> ()` with a NON-unit tail. The lowering produces `return 5;` and the
/// SEMANTIC PASS refuses it — not the C compiler.
///
/// THIS IS WHY THE LOWERING IS NOT GUARDED ON `Some(Type::Unit)`, which is the
/// obvious repair and the one review proposed. Measured with that guard in
/// place: `fn f() -> () { 5 }` compiles clean and emits `5;`, discarding the
/// value in silence. The type checker only sees the mismatch through the
/// `Stmt::Return` the lowering creates. So the lowering stays and code
/// generation handles the void case; see the comment at the lowering site in
/// src/parser/mod.rs.
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

/// `async fn main` is refused, and the refusal says what would have happened.
///
/// MEASURED at d0eebbf: it compiled, linked, ran and exited 0 having printed
/// nothing, because the entry point was emitted as `main_Future main()` with
/// the body inside a `main_poll` that nothing calls. That is a program which
/// does nothing while reporting success — the family this whole branch exists
/// to remove — at the entry point.
///
/// Refused rather than fixed, and the scope call is deliberate: making it work
/// needs an async runtime to drive the future, and the specification says there
/// is none (§N7). When this compiler cannot honour a construct it refuses it
/// with the reason and a workaround (`?`, `.await`, the LLVM backend); it does
/// not emit something shaped like a program.
#[test]
fn async_main_is_refused_rather_than_compiled_into_a_program_that_does_nothing() {
    let err = compile_to_c(
        "async fn main() {\n    print_int(7)\n}\n",
        "d3b_async_main",
    )
    .expect_err("`async fn main` has no entry point anything can call");
    assert!(
        err.contains("`async fn main`"),
        "the refusal must name the construct, got:\n{}",
        err
    );
    assert!(
        err.contains("main_Future main()"),
        "the note must say what WOULD have been emitted — that is the whole \
         reason this is refused rather than accepted:\n{}",
        err
    );
    assert!(
        err.contains("fn main()"),
        "the workaround must name the ordinary spelling:\n{}",
        err
    );
}

/// `async fn main` is refused whatever the spelling — and REFUSED ONLY WHEN IT
/// IS THE ENTRY POINT.
///
/// The previous version of this test documented imported coverage and exercised
/// none: it compiled single files, so breaking the import path would not have
/// failed it. Verifying by hand is not pinning, which is the distinction this
/// branch has enforced on everything else. These cases really import.
///
/// The controls matter more than the refusals. Refusing "an `async fn main` was
/// declared anywhere" instead of "the effective entry point is async" rejected
/// two valid programs — measured at fbcfc39 — and over-approximating a refusal
/// fails closed onto working code, which is the same defect as accepting what
/// cannot be honoured, pointing the other way.
#[test]
fn async_main_is_refused_only_when_it_is_the_entry_point() {
    // REFUSED: the imported one IS the entry point.
    let err = compile_and_run_with_import(
        "pub async fn main() { print_int(1) }\n",
        "import lib;\n\nfn helper() { print_int(2); }\n",
        "d3b_imported_async_main",
    )
    .expect_err("an imported async main is still an async entry point");
    assert!(
        err.contains("`async fn main`"),
        "an imported async main must be refused, got:\n{}",
        err
    );

    // CONTROL: shadowed by a perfectly good local `main`. The imported one can
    // never run, so there is nothing to refuse — and the local entry point must
    // still be the one that is emitted and executed.
    let out = compile_and_run_with_import(
        "pub async fn main() { print_int(1) }\n",
        "import lib;\n\nfn main() { print_int(7) }\n",
        "d3b_shadowed_async_main",
    )
    .expect("a local `main` shadows an imported one; this program is valid");
    assert_eq!(
        out.trim(),
        "7",
        "the LOCAL main must be the entry point, and the imported one must not \
         also be emitted"
    );

    // CONTROL: a PRIVATE imported `async fn main` is never registered, so it
    // can never be called or become the entry point.
    let out = compile_and_run_with_import(
        "async fn main() { print_int(1) }\n",
        "import lib;\n\nfn main() { print_int(7) }\n",
        "d3b_private_async_main",
    )
    .expect("a private imported function is not even registered");
    assert_eq!(out.trim(), "7");

    // The `pub` spelling of a LOCAL async main: visibility is not part of the
    // contract.
    let err = compile_to_c(
        "pub async fn main() { print_int(7) }",
        "d3b_async_main_pub",
    )
    .unwrap_err();
    assert!(err.contains("`async fn main`"), "{}", err);

    // The GENERIC spelling, with its EXACT current diagnostic pinned rather
    // than an alternation that would accept the transition it warns about.
    // `async fn main<T>` is registered as a generic function, so it never
    // satisfies the main-existence check and is refused with "No main function
    // found" BEFORE the async refusal is reached. If a change ever makes a
    // generic `main` an entry point, this line fails and the async case has to
    // be re-examined — which the previous `A || B` assertion would have let
    // through in silence.
    let err = compile_to_c(
        "async fn main<T>() { print_int(7) }",
        "d3b_async_main_generic",
    )
    .unwrap_err();
    assert!(
        err.contains("No main function found"),
        "the generic spelling is currently refused by main-existence, not by \
         the async rule; got:\n{}",
        err
    );
}

/// EVERY offending import is validated, not the last one recorded.
///
/// `deferred_async_value_return` was an `Option`, so each qualifying import
/// overwrote the previous one and `check` validated only the survivor. Measured
/// at 37004bf with two bad exports whose SECOND is locally shadowed: the first
/// escaped diagnosis entirely and gcc choked on the emitted C instead.
#[test]
fn every_offending_imported_async_export_is_validated_not_just_the_last() {
    let err = compile_and_run_with_import(
        "fn g() -> Future<()> { panic(\"x\"); }\n\
         pub async fn bad1() -> () { g() }\n\
         pub async fn bad2() -> () { g() }\n",
        "import lib;\n\nfn bad2() { print_int(2); }\nfn main() { bad2(); }\n",
        "d3b_two_offenders",
    )
    .expect_err("bad1 is not shadowed and must still be refused");
    assert!(
        err.contains("`return` with a value inside an `async fn`"),
        "the FIRST offender must be diagnosed even though the second is \
         shadowed; got:\n{}",
        err
    );
}

/// An imported GENERIC async violation that is NEVER INSTANTIATED is not
/// diagnosed.
///
/// The condition is instantiation, not genericity. This test used to claim the
/// wider reason — "code generation never emits an imported generic" — and could
/// not have caught that being false, because it never calls `gen`. It does not
/// call it now either; that is the point of this fixture, and
/// `an_instantiated_imported_generic_async_violation_is_diagnosed` is the other
/// half. Together they say: an uninstantiated generic is not in the output, so
/// refusing it would reject a declaration the output cannot contain — the
/// mirror of the shadowing case in the other axis.
///
/// Whatever this program fails for, it must not be the async rule.
#[test]
fn an_uninstantiated_imported_generic_async_violation_is_not_diagnosed() {
    let err = compile_and_run_with_import(
        "fn g() -> Future<()> { panic(\"x\"); }\n\
         pub async fn gen<T>() -> () { g() }\n\
         pub fn ok() { print_int(1); }\n",
        "import lib;\n\nfn main() { ok(); }\n",
        "d3b_imported_generic_async",
    )
    .err()
    .unwrap_or_default();
    assert!(
        !err.contains("`return` with a value inside an `async fn`"),
        "an uninstantiated generic is never emitted, so refusing it rejects a \
         declaration the output cannot contain; got:\n{}",
        err
    );
}

/// An imported `pub async fn` with a value-carrying return is refused too.
///
/// Same route as the entry-point case, one function over: only local functions
/// reach `check_function`, so `set_imported_modules` has to perform the same
/// validation. Unlike the entry-point case this does NOT depend on which
/// function wins — nothing can honour the declaration wherever it sits.
#[test]
fn an_imported_async_value_return_is_refused() {
    let err = compile_and_run_with_import(
        "fn g() -> Future<()> { panic(\"x\"); }\npub async fn af() -> () { g() }\n",
        "import lib;\n\nfn main() { print_int(7) }\n",
        "d3b_imported_async_value_return",
    )
    .expect_err("an imported async fn may not carry a value return either");
    assert!(
        err.contains("`return` with a value inside an `async fn`"),
        "got:\n{}",
        err
    );
}

/// THE TYPE SPACE IS WIDER THAN THE SPELLINGS — which is how I got this wrong.
///
/// Round 11 claimed the async unit replacement "cannot be fixtured" and listed
/// the async SPELLINGS whose bodies would contain a `Stmt::Return`, all of
/// which typeck refuses. That enumerated what I could write, not what the types
/// admit. An ORDINARY function may declare `Future<()>`, and typeck gives an
/// async function the return type `Future<declared>`, so
///
/// ```text
/// fn g() -> Future<()> { panic("x"); }
/// async fn f() -> () { g() }
/// ```
///
/// type-checks, reaches code generation, and — measured at 7d2fc0d — emitted a
/// DUPLICATE `return 1; // Ready` into the poll function while the value was
/// evaluated and dropped. With a non-unit output the same shape emitted
/// `return <struct>;` from an `int` function and gcc refused the C.
///
/// It is now refused. The value has nowhere to live: the poll function returns
/// an `int` readiness flag, and giving it a home means a future with a result
/// slot and something to drive it — the async runtime §N7 says does not exist.
#[test]
fn a_value_carrying_return_inside_an_async_fn_is_refused() {
    for (n, src) in [
        // the shape review found: an ordinary function supplying the Future
        "fn g() -> Future<()> { panic(\"x\"); }\nasync fn f() -> () { g() }",
        // and the non-unit output, whose emitted C did not compile at all
        "fn g() -> Future<i64> { panic(\"x\"); }\nasync fn f() -> i64 { g() }",
    ]
    .iter()
    .enumerate()
    {
        let program = format!("{}\nfn main() {{ f(); }}\n", src);
        let err = compile_to_c(&program, &format!("d3b_async_value_return_{}", n))
            .expect_err("a value-carrying return inside an async fn has nowhere to go");
        assert!(
            err.contains("`return` with a value inside an `async fn`"),
            "the refusal must name the construct, got:\n{}",
            err
        );
        assert!(
            err.contains("readiness flag"),
            "the note must say WHY there is nowhere to put it:\n{}",
            err
        );
    }

    // NESTED value returns, one per traversal arm of `has_value_return`. The
    // tail lowering puts the return in whichever branch was the tail, so a
    // walker that only looked at the top level would miss every one of these;
    // deleting any arm turns this red.
    for (n, body) in [
        // if / else
        "if n > 0 { g() } else { g() }",
        // match
        "match n { _ => g(), }",
        // while, with the return inside the loop body
        "while n > 0 { return g(); }",
        // for
        "for i in 0..n { return g(); }",
        // unsafe
        "unsafe { return g(); }",
    ]
    .iter()
    .enumerate()
    {
        let program = format!(
            "fn g() -> Future<()> {{ panic(\"x\"); }}\n\
             async fn f(n: i64) -> () {{ {} }}\n\
             fn main() {{ f(1); }}\n",
            body
        );
        let err = compile_to_c(&program, &format!("d3b_async_nested_{}", n))
            .expect_err(&format!(
                "a value return nested in `{}` must still be refused — the \
                 traversal arm for it is missing",
                body
            ));
        assert!(
            err.contains("`return` with a value inside an `async fn`")
                || err.contains("Type mismatch"),
            "nested in `{}`, got:\n{}",
            body,
            err
        );
    }

    // TRANSITIONED, NOT DELETED. This block used to read "the shape that
    // remains accepted has no `Stmt::Return` at all" and asserted that
    // `async fn f() { print_int(1) }` COMPILED, with exactly one
    // `return 1; // Ready` in its poll function. That was the last async shape
    // reaching code generation, and emitting a poll function for it is the
    // N7-18 normative violation: §N7 says effect tracking has no runtime
    // representation, and an `f_Future` struct with a `state` field is one.
    //
    // It is refused now, by the general `async fn` arm rather than by this
    // file's value-return arm, so the assertion inverts. The property this
    // block still carries for THIS test is the second one: the value-return
    // refusal must not be what fires, because this body has no value to
    // return. `tests/m2_async_producer.rs` owns the rest.
    let err = compile_to_c(
        "async fn f() { print_int(1) }\n\nfn main() { f(); }\n",
        "d3b_async_accepted",
    )
    .expect_err("the unannotated async unit function is the N7-18 producer");
    assert!(
        err.contains("`async fn` is not implemented"),
        "the plainest async spelling must be refused as the construct itself, \
         not as one of its sub-cases:\n{}",
        err
    );
    assert!(
        !err.contains("`return` with a value"),
        "this body has no value return; naming one would be a diagnostic about \
         something that is not in the program:\n{}",
        err
    );
}

/// What the language does with a user-written `return 0;` in a unit function:
/// it REFUSES it. Neither accepted nor silently rewritten.
///
/// Worth pinning because the codegen change rewrites `Stmt::Return(Some(expr))`
/// in unit functions, and a reader could reasonably wonder whether a
/// deliberate `return 0;` is being quietly turned into `return;`. It is not —
/// it never reaches code generation.
#[test]
fn a_user_written_return_zero_in_a_unit_function_is_refused() {
    let err = compile_to_c(
        "fn f() { return 0; }\n\nfn main() { f(); }\n",
        "d3b_unit_return_zero",
    )
    .expect_err("a unit function cannot return a value");
    assert!(
        err.contains("expected ()") && err.contains("found Int"),
        "the type checker must refuse it, got:\n{}",
        err
    );
}

// ---------------------------------------------------------------------------
// Module-system defects this branch MEASURED but does not own
// ---------------------------------------------------------------------------
//
// THE SCOPE CALL, stated once for both. This branch is about tail returns and
// the family of "compiles, runs, does nothing" defects. It reached the module
// system because refusing `async fn main` forced the question "which `main`
// wins", and answering that exposed two further disagreements between the
// resolver, the type checker and code generation about WHAT THE IMPORTED
// PROGRAM IS.
//
// Designing module semantics inside a defect branch is how a branch stops
// being reviewable — this one is already thirteen rounds long. So the two
// defects below are DECLARED with an owner and a failing test rather than
// fixed here: M4 owns modules. Each says what a program can currently do that
// it should not, because a declaration that only names a mechanism is a note,
// not a debt.
//
// What was NOT deferred: the entry-point question the refusal actually needs,
// and the typeck/codegen agreement about shadowing — both fixed, because both
// are consequences of a diagnostic this branch added.

/// THE BORROW CHECKER NEVER HEARS ABOUT THE IMPORTED PROGRAM, SO CALLING AN
/// IMPORTED FUNCTION IS "USE OF UNINITIALIZED VALUE".
///
/// THIS IS UPSTREAM OF THE TWO ROWS BELOW, and it is declared first for that
/// reason: every module-system test that CALLS an imported function stops here,
/// so a row further down could stay green on a failure that has nothing to do
/// with what it declares. (Whether one actually did is now a measurement rather
/// than a worry — see the diagnostic column in tests/rust-debt-manifest.txt.)
///
/// "STOPS HERE" IS ORDERED, NOT UNCONDITIONAL, and the two rows below differ on
/// it. `a_local_fn_shadowing_an_imported_async_fn…` calls only its LOCAL `f`, so
/// the wall is not on its path at all. `selective_import_excludes_a_symbol…`
/// does call an imported `helper`, so the wall IS on its path — it simply never
/// arrives, because src/driver/mod.rs:109 (`type_checker.check(&ast)?`) returns
/// Err before :137-138 constructs the borrow checker. Its honest reason is
/// therefore "the wall is downstream of this refusal", not "the wall is
/// irrelevant": move the refusal that fires first and that row's diagnostic
/// becomes this one's.
///
/// The chain, read in the code rather than inferred from the message:
///
///   src/driver/mod.rs:104-107   the resolver's output goes to the type
///                               checker via `set_imported_modules`.
///   src/driver/mod.rs:137-138   `BorrowChecker::new()` is constructed and
///                               `check_program(&ast)` is called.
///                               `resolved_modules` is LIVE in that scope and
///                               is not passed. That omission is the whole
///                               mechanism.
///   src/ownership/borrow_checker.rs:134-138
///                               `functions` is seeded from `BUILTINS` and
///                               nothing else.
///   src/ownership/borrow_checker.rs:335-355
///                               `check_program` walks `program.items` only.
///                               `Program.imports` (src/ast/mod.rs:9) is never
///                               read, and `Item` (src/ast/mod.rs:24-32) has no
///                               `Import` variant, so nothing in the local AST
///                               could have carried the imported signatures
///                               either.
///   src/ownership/borrow_checker.rs:889 -> :502 -> :527
///                               `Expr::Call` checks its callee expression;
///                               `Expr::Ident` misses `functions`, falls
///                               through to the ownership table, finds no
///                               place, and returns `UseOfUninitializedValue`.
///
/// Measured on this tree: `pub fn helper() -> i64 { return 5; }` in lib.pd,
/// imported by `fn main() { print_int(helper()); }`, is refused with
/// "Use of uninitialized value: helper" AFTER the type checker has said "All
/// types check out". A program that type-checks is rejected by the pass that
/// exists to check ownership, for a name that is not a value at all.
///
/// NOT FIXED HERE, and deliberately not: the fix is a driver-level decision
/// about what the imported program is — the same shared definition the two rows
/// below need — and it is being made on its own branch. What this row buys is
/// that the wall is DECLARED, with its mechanism, so it cannot go on being the
/// silent explanation for somebody else's green.
#[test]
fn an_imported_function_is_visible_to_the_borrow_checker() {
    let out = compile_and_run_with_import(
        "pub fn helper() -> i64 { return 5; }\n",
        "import lib;\n\nfn main() { print_int(helper()); }\n",
        "d3b_import_borrowck",
    );
    // The compiler's own output is the payload, not a bare "it failed": this
    // row's job is to name WHICH refusal, and the debt manifest matches on it.
    assert_eq!(
        out.as_deref().map(str::trim),
        Ok("5"),
        "calling an imported function must reach code generation; the borrow \
         checker rejects it instead:\n{}",
        out.clone().err().unwrap_or_default()
    );
}

/// A LOCAL `fn f` SHADOWING AN IMPORTED `pub async fn f` STILL TYPES THE CALL
/// AS A FUTURE.
///
/// What a program can do today that it should not: emit `f_Future v = f();`
/// beside `long long f()`. `CodeGenerator.async_functions`
/// (src/codegen/mod.rs:188-201) is INSERT-ONLY — unlike `functions`, which the
/// main-program pass overwrites entry by entry — so an imported `pub async fn f`
/// leaves `f` in the set even when a local ordinary `fn f` replaces it, and
/// `try_infer_expr_type` (src/codegen/mod.rs:328-328) reads the set rather than
/// asking `crate::ast::local_definition_shadows_import`.
///
/// Measured: gcc reports `use of undeclared identifier 'f_Future'` against C
/// the programmer never wrote, after the compiler has already printed
/// "Compilation successful".
///
/// WHY IT IS A ROW AND NOT A FIX, and why the owed count moved. This is the
/// same class as the prototype-loop defect — a decision about the imported
/// program made without asking the shared predicate — one container over, and
/// it belongs to the module system. It was previously recorded only as a
/// comment on the field, because declaring it would move an `owed=43` that
/// several receipts had already quoted. A receipt is a measurement, not a
/// budget: preserving the number by omitting known debt is precisely what a
/// closed inventory exists to prevent, so the row is here and the number moved.
#[test]
#[ignore = "XFAIL: CodeGenerator.async_functions (src/codegen/mod.rs:188-201) is insert-only, so an imported `pub async fn f` shadowed by a local ordinary `fn f` leaves `f` in the set and try_infer_expr_type (src/codegen/mod.rs:328-328) types the call to the LOCAL f as `f_Future`. Measured: the emitted C carries `f_Future v = f();` beside `long long f()` and gcc reports `use of undeclared identifier 'f_Future'` after the compiler reported success. Needs the set to ask crate::ast::local_definition_shadows_import, as the imported body and prototype loops now do (owned by M4)"]
fn a_local_fn_shadowing_an_imported_async_fn_is_not_typed_as_a_future() {
    let out = compile_and_run_with_import(
        "pub async fn f() { print_int(1); }\n",
        "import lib;\n\nfn f() -> i64 { return 3; }\n\nfn main() { let v = f(); print_int(v); }\n",
        "d3b_shadowed_async",
    );
    assert_eq!(
        out.as_deref().map(str::trim),
        Ok("3"),
        "the local `fn f` is the one that is called and the one that is \
         emitted, so its call must be typed as `long long`:\n{}",
        out.clone().err().unwrap_or_default()
    );
}

/// SELECTIVE IMPORT DOES NOT EXCLUDE ANYTHING FROM THE CONSUMERS.
///
/// What a program can do today that it should not: `import lib::{helper}` names
/// one item, and the resolver filters `ModuleInfo.exports` accordingly
/// (src/resolver/mod.rs:105-118) — but the type checker
/// (src/typeck/mod.rs, `set_imported_modules`) and code generation
/// (src/codegen/mod.rs, the imported-function loops) both iterate the PUBLIC
/// FUNCTIONS OF THE UNCHANGED MODULE AST and never read `exports`. So every
/// public item of the module is registered and emitted regardless of what the
/// import named.
///
/// Measured: a module exporting `helper` and `main`, imported as
/// `import lib::{helper}`, is still rejected with "`async fn main` is not
/// implemented" — for a symbol the import deliberately excluded. The same route
/// registers signatures and emits bodies for excluded functions.
///
/// The fix is one shared definition of the effective imported symbol set, read
/// by both consumers. That is module-system work, not tail-return work.
#[test]
#[ignore = "XFAIL: selective import (`import lib::{item}`) filters only ModuleInfo.exports (src/resolver/mod.rs:105-118); src/typeck/mod.rs set_imported_modules and src/codegen/mod.rs's imported-function loops both iterate the unchanged module AST's public functions instead, so excluded items are still registered, still emitted, and still reach entry-point rejection. Measured: `import lib::{helper}` from a module that also declares `pub async fn main` is rejected for that `main`. Needs one shared definition of the effective imported symbol set consumed by both passes (owned by M4). The import wall (an_imported_function_is_visible_to_the_borrow_checker) is DOWNSTREAM of this refusal rather than irrelevant to it: this fixture does call an imported `helper`, so the wall IS on its path, but src/driver/mod.rs:109 `type_checker.check(&ast)?` returns Err before :137-138 ever constructs the borrow checker. The day the async-main refusal moves, this row's failure becomes the wall's and this diagnostic names nothing that happened"]
fn selective_import_excludes_a_symbol_from_the_consumers() {
    let err = compile_and_run_with_import(
        "pub fn helper() { print_int(2); }\npub async fn main() { print_int(1) }\n",
        "import lib::{helper};\n\nfn other() { helper(); }\n",
        "d3b_selective_import",
    )
    .err()
    .unwrap_or_default();
    // With no local `main` this program has no entry point, so it must fail —
    // but with "No main function found", because the module's `main` was NOT
    // imported. Today it fails with the async-main refusal instead, which is
    // the leak: a symbol the import excluded still reached entry-point
    // resolution.
    assert!(
        !err.contains("`async fn main`"),
        "`main` was not imported, so it must not be considered at all:\n{}",
        err
    );
    assert!(
        err.contains("No main function found"),
        "the excluded `main` must not supply an entry point:\n{}",
        err
    );
}

/// TWO MODULES EXPORTING ONE NAME HAVE NO RULE, AND THE OUTPUT IS
/// NONDETERMINISTIC.
///
/// What a program can do today that it should not: import two modules that each
/// export `dup`, and get a program whose EMITTED C DIFFERS BETWEEN IDENTICAL
/// RUNS. `set_imported_modules` overwrites the unqualified name in a `HashMap`
/// and code generation iterates `self.imported_modules.values()` — a `HashMap`,
/// so in arbitrary order.
///
/// Nondeterministic output from identical input is worse than a wrong answer,
/// because no transcript can pin it. There is no defined semantics to appeal to
/// — reject the collision, first-wins, or require qualification are all
/// defensible, and choosing is module-system design.
///
/// WHAT THE DECLARATION USED TO SAY, AND WHY IT NO LONGER DOES. This row was
/// written around a measurement — "six identical compilations produced
/// `int dup()` three times and `long long dup()` three times" — that the test
/// cannot observe and the compiler no longer produces. Both `dup` bodies are
/// now emitted into one translation unit, so gcc refuses the program with
/// `conflicting types for 'dup'` and `out.status.success()` is false on every
/// run: `compiled` stays 0 and the assertion that fires is the PRECONDITION,
/// not the claim. The row was green on a failure it did not describe, which is
/// what the manifest's diagnostic column now makes impossible to repeat. The
/// nondeterminism is still real — which body is registered and which is emitted
/// is still HashMap order — it is simply no longer the first thing that goes
/// wrong.
///
/// THE SCOPE OF THIS ROW ALSO COVERS DECLARATION IDENTITY, and it is bounded
/// here rather than fixed. Imported generics are stored by BARE NAME
/// (`TypeChecker.generic_functions`), and the deferred-refusal lists that
/// src/typeck/mod.rs:2193-2214 filters carry `(name, span)` and nothing else.
/// So with two same-named imported generic `async fn`s, the refusal is raised
/// off whichever declaration was RECORDED and the body that would have been
/// emitted is whichever won a `HashMap`: THE REFUSAL MAY NAME A DECLARATION
/// THAT IS NOT THE ONE IN THE OUTPUT, and its message carries no module
/// identity with which a reader could tell. That is exhibited by
/// `the_generic_async_refusal_carries_no_declaration_identity` below, which is
/// an ordinary test because the behaviour it pins is what happens TODAY.
/// Fixing it means deciding the same collision semantics this row is about, so
/// it is one debt, not two.
#[test]
#[ignore = "XFAIL: two imported modules exporting the same name have no defined semantics — src/typeck/mod.rs set_imported_modules overwrites the unqualified name in a HashMap and src/codegen/mod.rs iterates imported_modules.values() (also a HashMap), so both the registered signature and the emitted body are chosen by iteration order. Measured today: both bodies are emitted and gcc refuses the program with `conflicting types for 'dup'`, so the fixture never links and the nondeterminism sits behind that. Also bounded here: imported generics are keyed by bare name and the deferred refusals carry only (name, span), so with two same-named imported generic async fns the refusal may name a declaration other than the one that would have been emitted, with no module identity in the message — exhibited by the_generic_async_refusal_carries_no_declaration_identity. Needs a decision — reject the collision, first-wins, or require qualification — and both layers honouring it (owned by M4)"]
fn two_modules_exporting_one_name_are_deterministic() {
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut compiled = 0;
    // WHY THE COMPILER'S OUTPUT IS KEPT. Without it the precondition failed
    // with "the collision fixture never compiled, so this proves nothing" —
    // true, and useless: it named no mechanism, so the row could not say which
    // defect it was failing on, and neither could the debt manifest.
    let mut last_refusal = String::new();
    for i in 0..6 {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.pd"), "pub fn dup() -> i64 { 1 }\n").unwrap();
        fs::write(dir.path().join("b.pd"), "pub fn dup() -> bool { true }\n").unwrap();
        fs::write(
            dir.path().join("app.pd"),
            "import a;\nimport b;\n\nfn main() { print_int(9); }\n",
        )
        .unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_pdc"))
            .args(["compile", "app.pd", "-o", &format!("d3b_dup_{}", i)])
            .current_dir(dir.path())
            .output()
            .expect("failed to run pdc");
        if !out.status.success() {
            last_refusal = format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            continue;
        }
        compiled += 1;
        let c = fs::read_to_string(dir.path().join("build_output").join("app.c"))
            .unwrap_or_default();
        // Every line mentioning `dup`, prototype and definition alike: the
        // whole point is that WHICH module's `dup` is emitted varies.
        let shape: Vec<&str> = c
            .lines()
            .filter(|l| l.contains("dup("))
            .map(|l| l.trim())
            .collect();
        seen.insert(shape.join(" | "));
    }
    assert!(
        compiled > 0,
        "the collision fixture never compiled, so the nondeterminism this row \
         declares cannot even be reached. The compiler said:\n{}",
        last_refusal
    );
    assert_eq!(
        seen.len(),
        1,
        "identical input produced {} different emitted shapes across {} runs: {:#?}",
        seen.len(),
        compiled,
        seen
    );
}

/// CONTROL for the identity bound recorded on
/// `two_modules_exporting_one_name_are_deterministic`: the generic-async
/// refusal names a SYMBOL, not a DECLARATION.
///
/// WHY THIS IS AN ORDINARY TEST AND NOT AN XFAIL. It does not assert what the
/// compiler ought to do — it pins what it DOES, so that the bound written into
/// that row is a measurement a reader can re-run rather than a caveat they have
/// to take on trust. When declaration identity is carried, this test goes red
/// and points at the row that has to be transitioned.
///
/// THE SHAPE. Two imported modules both export a generic `async fn agen<T>`.
/// Only `a.pd`'s returns a value, so only `a.pd`'s is recorded in
/// `deferred_generic_async_value_returns` (src/typeck/mod.rs:1313-1324), and the
/// refusal is raised for it at src/typeck/mod.rs:2153-2159 once the call site
/// has instantiated the name. But `generic_functions` is keyed by BARE NAME and
/// `set_imported_modules` iterates a `HashMap`, so WHICH module's body that key
/// holds — and therefore which body `get_instantiations` would have handed to
/// code generation — is iteration order. The refusal fires either way, and:
///
///   * it names `agen` and nothing else — no module, no path, so the message
///     cannot distinguish the two declarations;
///   * its span renders against `app.pd`, which declares neither of them.
///
/// WHAT THE SIX RUNS BELOW ESTABLISH, AND WHAT THEY DO NOT. They establish the
/// two bullets above: every run refuses, every refusal names `agen` alone, and
/// every span renders against `app.pd`. They do NOT observe which module's body
/// held the key on any run — nothing in the output says, which is the whole
/// finding. An earlier version of this comment went further and described "the
/// runs where `b.pd`'s harmless body won the key" as a measured event; it is
/// not measured, here or anywhere, and a claim about a run nobody can read is
/// the same defect this file exists to remove.
///
/// So the exhibit is exactly this: the refusal is not wrong to exist — an
/// offending declaration IS reachable — but which declaration it is about is
/// not something it can say, and a reader cannot recover it from the message.
#[test]
fn the_generic_async_refusal_carries_no_declaration_identity() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("a.pd"),
        "pub async fn agen<T>(x: T) -> i64 { return 42; }\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("b.pd"),
        "pub async fn agen<T>(x: T) { print_int(1); }\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("app.pd"),
        "import a;\nimport b;\n\nfn main() { agen(1); }\n",
    )
    .unwrap();

    // Six runs, because the verdict must be pinned across whatever the HashMap
    // does — `set_imported_modules` iterates one, so the winning body may differ
    // between runs and one run cannot show that the verdict does not. Note the
    // asymmetry, since it is the whole reason this is a control and not a proof:
    // six identical verdicts are consistent with the order having varied and
    // with its never having varied at all. What is asserted is only the former's
    // consequence — the message is the same either way, and says nothing that
    // would let a reader tell which.
    for i in 0..6 {
        let out = Command::new(env!("CARGO_BIN_EXE_pdc"))
            .args(["compile", "app.pd", "-o", &format!("d3b_agen_{}", i)])
            .current_dir(dir.path())
            .output()
            .expect("failed to run pdc");
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            !out.status.success(),
            "run {}: an imported generic `async fn` with a value return is \
             refused, whichever module supplied the body:\n{}",
            i,
            text
        );
        assert!(
            text.contains("a `return` with a value inside an `async fn` (imported: `agen`)"),
            "run {}: the refusal must be the imported-async one:\n{}",
            i,
            text
        );
        // THE POINT. `a.pd` is the file that declares the offending `agen`, and
        // `b.pd` the one that does not. Neither is named, so the diagnostic
        // cannot say which declaration it refused — and the span it does print
        // belongs to `app.pd`, which declares neither.
        assert!(
            !text.contains("a.pd") && !text.contains("b.pd"),
            "run {}: the refusal names a module, so declaration identity is \
             now carried — transition the bound recorded on \
             `two_modules_exporting_one_name_are_deterministic`:\n{}",
            i,
            text
        );
        // The `-->` marker and the path are separated by colour escapes, so the
        // path alone is what can be matched without a terminal-aware reader.
        assert!(
            text.contains("app.pd:1:"),
            "run {}: the span is expected to render against the importing \
             file, which declares neither `agen`:\n{}",
            i,
            text
        );
    }
}

// ---------------------------------------------------------------------------
// The obligation that outlives this branch, tracked rather than commented
// ---------------------------------------------------------------------------

/// THE SEQUENCING CONSTRAINT, as an ordinary invariant rather than a reminder.
///
/// The XFAIL below records that the flag is owed. It does not FORCE anything:
/// fixing the exhaustive-match defect leaves it failing exactly as before, so
/// someone could land the final `else` and never think about the flag again. A
/// gated reminder is not a dependency.
///
/// This is the dependency, and it is checkable today: EITHER the match defect
/// is still open, OR the shared linker carries the flag. Today the first arm
/// holds, so this passes. The moment somebody promotes
/// `tests/stdlib/DRIVERS.tsv`'s `stdlib_tail_match` row from
/// `known_violation:…` to `clean` — the first arm stops holding and this test
/// fails until the flag is added. It is an ordinary test, so it runs in
/// `make test-honest` and in `make m1-exit` inventory 3.
///
/// THE CROSS-GATE HALF, cited rather than asserted: the promotion is not
/// optional. `scripts/stdlib-gate.sh:379` enters the `known_violation:*` arm
/// for that row; `scripts/stdlib-gate.sh:389-391` turns a CLEAN result into
/// `note "XPASS: … is recorded known_violation:… but its C is now CLEAN …
/// promote it to 'clean'"`, and `note` is what makes that gate red. So the
/// moment codegen emits the final `else`, `make stdlib-gate` fails until the
/// row is promoted, and promoting it makes THIS test fail until the flag is
/// added. Neither gate can be satisfied by ignoring the other.
///
/// What this does NOT establish: that `stdlib_tail_match` is the ONLY pin that
/// would have to move. It is the one the handoff names and the one this test
/// reads; a second fixture pinned on the same defect would need its own row
/// here.
#[test]
fn the_missing_else_may_not_be_fixed_without_arming_the_linker() {
    let drivers = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/stdlib/DRIVERS.tsv"),
    )
    .expect("tests/stdlib/DRIVERS.tsv is the pin for the open match defect");
    // COLUMN 3, not "somewhere in the row". Scanning the whole line for the
    // token keeps this green forever once someone writes the word into the
    // note column — which is exactly where a future reader would explain what
    // the old `known_violation:` pin used to say.
    let match_defect_open = drivers
        .lines()
        .filter(|l| l.starts_with("stdlib_tail_match\t"))
        .filter_map(|l| l.split('\t').nth(2))
        .any(|verdict| verdict.starts_with("known_violation:"));

    let cmd = link_command(
        Path::new("/tmp/x.c"),
        Path::new("/tmp/x"),
        OptLevel::Default,
    )
    .expect("runtime should resolve in a dev checkout");
    let linker_armed = cmd
        .get_args()
        .any(|a| a.to_string_lossy() == "-Werror=return-type");

    assert!(
        match_defect_open || linker_armed,
        "the exhaustive-match defect is no longer pinned in \
         tests/stdlib/DRIVERS.tsv, so codegen now emits a final `else` — which \
         is the one thing that was keeping -Werror=return-type out of \
         src/linker.rs. Add it to link_command in this change, and transition \
         the debt row for \
         `the_linker_will_ask_gcc_to_reject_a_function_that_falls_off_its_end` \
         in tests/rust-debt-manifest.txt from `owed` to `paid`."
    );
}

/// `-Werror=return-type` belongs in the shared gcc invocation, and does not
/// belong there YET.
///
/// WHY IT IS ABSENT: the open missing-`else` defect (see
/// `NetA::StillFindsTheOpenMatchDefect`) makes gcc reject the C emitted for any
/// tail `match`, so turning the flag on today breaks every such program. That
/// is a temporary position.
///
/// WHY IT IS A TEST AND NOT A COMMENT: a comment in a handoff cannot make
/// anybody do anything. When someone flips that expectation to `Accepts`, the
/// missing `else` is fixed and this flag is the next step — and nothing was
/// mechanically asking for it. This branch built a closed debt inventory for
/// exactly this shape of obligation, so the obligation goes in it:
/// `tests/rust-debt-manifest.txt` declares this row, `make test-xfail` requires
/// the row and the `#[ignore]` to agree, and deleting either is a gate failure
/// rather than a tidy-up.
///
/// WHEN IT PASSES: delete the `#[ignore]` and transition the manifest row to
/// `paid`. Net A does not go away — its role changes, from the primary
/// structural boundary to ATTRIBUTION (it names the function and the line
/// without a compiler, and reads C that never links).
#[test]
#[ignore = "XFAIL: the shared gcc invocation (src/linker.rs:73-86) omits -Werror=return-type, so a generated function that falls off its end links silently and only scripts/check-c-returns.py objects. It cannot be added until codegen emits a final `else` for `match` — that defect makes gcc reject every tail-`match` program today (pinned by tests/stdlib/DRIVERS.tsv:31, known_violation:area_code,sides, and by NetA::StillFindsTheOpenMatchDefect in this file). Add the flag in the same change that lands the `else`. (owned by unscheduled: it is sequenced behind the exhaustive-match defect, which no milestone currently owns)"]
fn the_linker_will_ask_gcc_to_reject_a_function_that_falls_off_its_end() {
    let cmd = link_command(
        Path::new("/tmp/x.c"),
        Path::new("/tmp/x"),
        OptLevel::Default,
    )
    .expect("runtime should resolve in a dev checkout");
    let args: Vec<String> = cmd
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert!(
        args.iter().any(|a| a == "-Werror=return-type"),
        "gcc is invoked without -Werror=return-type, so C that falls off the \
         end of a non-void function links silently: {:?}",
        args
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
    // (src/optimizer/constant_folding.rs:154, `BinOp::Eq`). The two analyses
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

/// A local definition SHADOWS an imported one at the PROTOTYPE too, not only at
/// the body.
///
/// The imported-function loop asked `local_definition_shadows_import`; the
/// imported-PROTOTYPE loop, in `generate_function_prototypes`, did not, and it
/// is the loop that wins: `seen` is first-wins and imports are visited before
/// the main program, so the imported signature took the name and the local
/// prototype was dropped as a duplicate. MEASURED at 2fe30e7 with exactly this
/// program: the C carried
///
/// ```text
/// long long f(long long x);        // from the import
/// long long f() { return 5; }      // the local definition
/// ```
///
/// and gcc refused it ("conflicting types for 'f'", then "too few arguments").
/// A same-named local function with a DIFFERENT signature is the whole of the
/// exposure, so the fixture gives them different arities on purpose — same
/// arity would link and run and prove nothing.
#[test]
fn a_local_definition_shadows_the_imported_prototype_too() {
    let out = compile_and_run_with_import(
        "pub fn f(x: i64) -> i64 { return x; }\n",
        "import lib;\n\nfn f() -> i64 { return 5; }\n\nfn main() {\n    let v = f();\n    print_int(v);\n}\n",
        "d3b_shadowed_prototype",
    )
    .expect("the local definition is the only `f`, so the C must compile and run");
    assert_eq!(
        out.trim(),
        "5",
        "the local `f` is the one that runs; an imported prototype beside it \
         is a C that does not compile"
    );
}

/// A local function generic in only LIFETIMES still shadows an import.
///
/// The control on `local_definition_shadows_import`'s exact test. Its comment
/// used to say "a local GENERIC does not shadow", which is wider than
/// `type_params.is_empty()`: `Function` also carries `lifetime_params` and
/// `const_params`, and nothing defers a function generic in only those axes —
/// typeck registers it as an ordinary signature and codegen emits it under its
/// own name. So it DOES replace the import.
///
/// This test fails on the change that would make the code match the old
/// sentence. MEASURED: widening the predicate to also require
/// `const_params.is_empty() && lifetime_params.is_empty()` emits the imported
/// body as well, and gcc reports `redefinition of 'f'`.
#[test]
fn a_lifetime_generic_local_still_shadows_an_import() {
    let out = compile_and_run_with_import(
        "pub fn f() -> i64 { return 1; }\n",
        "import lib;\n\nfn f<'a>() -> i64 { return 5; }\n\nfn main() {\n    let v = f();\n    print_int(v);\n}\n",
        "d3b_lifetime_generic_shadow",
    )
    .expect("one definition of `f` must be emitted, not two");
    assert_eq!(out.trim(), "5", "the local definition is the one that runs");
}

/// The same, for a local generic in only CONST parameters.
#[test]
fn a_const_generic_local_still_shadows_an_import() {
    let out = compile_and_run_with_import(
        "pub fn f() -> i64 { return 1; }\n",
        "import lib;\n\nfn f<const N: u64>() -> i64 { return 5; }\n\nfn main() {\n    let v = f();\n    print_int(v);\n}\n",
        "d3b_const_generic_shadow",
    )
    .expect("one definition of `f` must be emitted, not two");
    assert_eq!(out.trim(), "5", "the local definition is the one that runs");
}

/// EVERY unshadowed offender is NAMED, in an order that is a function of the
/// program.
///
/// `every_offending_imported_async_export_is_validated_not_just_the_last`
/// establishes that a second offender is still VALIDATED. It does not establish
/// that it is REPORTED: the raise loop returned at the first one it found, and
/// the list it walked came out of `imported_modules`, a `HashMap`, so which
/// offender supplied the single diagnostic was not even a function of the
/// program. Both names must appear, and they must appear in the same order
/// every time — the fixture spells them `zeta` before `alpha` in the source so
/// that source order and sorted order disagree, which is what makes the sort
/// observable.
#[test]
fn all_offending_imported_async_exports_are_named_in_a_stable_order() {
    let mut seen: Vec<String> = Vec::new();
    for i in 0..5 {
        let err = compile_and_run_with_import(
            "fn g() -> Future<()> { panic(\"x\"); }\n\
             pub async fn zeta() -> () { g() }\n\
             pub async fn alpha() -> () { g() }\n",
            "import lib;\n\nfn main() { print_int(7); }\n",
            &format!("d3b_all_offenders_{}", i),
        )
        .expect_err("neither offender is shadowed, so both must be refused");
        assert!(
            err.contains("`alpha`") && err.contains("`zeta`"),
            "both offenders must be named, not just whichever the hash order \
             put first; got:\n{}",
            err
        );
        let line = err
            .lines()
            .find(|l| l.contains("(imported:"))
            .unwrap_or_default()
            .to_string();
        seen.push(line);
    }
    assert!(
        seen.windows(2).all(|w| w[0] == w[1]),
        "the diagnostic must be a function of the program, not of hash order; got:\n{:#?}",
        seen
    );
    assert!(
        seen[0].find("`alpha`") < seen[0].find("`zeta`"),
        "sorted by name, so `alpha` precedes `zeta` even though the source \
         declares `zeta` first; got:\n{}",
        seen[0]
    );
}

/// An imported GENERIC async violation IS diagnosed once it is instantiated.
///
/// The reason the validation was dropped — "code generation never emits an
/// imported generic" — was false, and the test that stood in for it never
/// instantiated the generic, so it could not have caught that. `generic_functions`
/// holds imported generics, `check_call` consults it BEFORE `functions`, and
/// `get_instantiations` hands the imported body to codegen. MEASURED at 2fe30e7
/// with this program: the emitted C carried `long long agen__i64(long long x)`
/// beside `agen_Future v = agen__i64(7);`, and clang reported
/// `use of undeclared identifier 'agen_Future'` — exactly the class refused for
/// a non-generic import, reached through the generic path.
#[test]
fn an_instantiated_imported_generic_async_violation_is_diagnosed() {
    let err = compile_and_run_with_import(
        "pub async fn agen<T>(x: T) -> i64 { return 42; }\n",
        "import lib;\n\nfn main() {\n    let v = agen(7);\n    print_int(v);\n}\n",
        "d3b_instantiated_imported_generic_async",
    )
    .expect_err("an instantiated imported generic IS part of the emitted program");
    assert!(
        err.contains("`return` with a value inside an `async fn`"),
        "the refusal must be the async rule, not whatever the broken C says \
         downstream; got:\n{}",
        err
    );
    assert!(
        err.contains("`agen`"),
        "the offender must be named; got:\n{}",
        err
    );
}
