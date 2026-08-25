//! N6-11: a `match` that falls through no arm TRAPS rather than continuing.
//!
//! # Why this is a Rust observable and not a `.pd` fixture
//!
//! There is no route from Palladium source to a run-time fall-through. N6-10
//! makes a non-exhaustive `match` a compile error for every scrutinee type, the
//! type checker refuses a pattern that cannot match its scrutinee, and the
//! language has no raw pointers, no transmute and no unchecked cast with which
//! to hand a `match` a value outside its type. `unsafe` parses and enforces
//! nothing, so it cannot express one either — measured, not assumed: N7's unsafe
//! row is `vacuous` in the conformance manifest for exactly that reason.
//!
//! That is the good outcome, and it is also why the trap is not dead weight: it
//! defends the gap between what the checker PROVES and what a process can HOLD —
//! a corrupted tag, a future checker bug, a backend that grows a hole. A defence
//! that only exists where the checker is already right defends nothing.
//!
//! So the behavioural proof is built at the level where the fall-through is
//! expressible: the emitted C. `unmatched_scrutinee_traps` compiles a real
//! program, corrupts the scrutinee's tag IN THE GENERATED C, and runs it — the
//! process must die on the trap rather than walk out of the `match`. Everything
//! else in the file is the same C the compiler emits for that program.

use palladium::linker::{link_command, OptLevel};
use palladium::Driver;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

/// Compile `source` with the real driver and hand back the generated C.
fn generate_c(dir: &Path, name: &str, source: &str) -> String {
    let src = dir.join(format!("{}.pd", name));
    fs::write(&src, source).unwrap();
    let c_file = Driver::new()
        .compile_file(&src)
        .unwrap_or_else(|e| panic!("`{}` must compile: {:?}", name, e));
    fs::read_to_string(&c_file).unwrap()
}

/// Build a C file with the shared gcc invocation and run it.
///
/// The SHARED invocation, not a hand-rolled one: it carries
/// `-Werror=return-type` since N6-11, and a test that quietly dropped the flag
/// would be proving something about C nobody ships.
fn build_and_run(dir: &Path, name: &str, c_source: &str) -> std::process::Output {
    let c_file = dir.join(format!("{}.c", name));
    let exe = dir.join(name);
    fs::write(&c_file, c_source).unwrap();
    let out = link_command(&c_file, &exe, OptLevel::Default)
        .expect("runtime should resolve in a dev checkout")
        .output()
        .expect("gcc should run");
    assert!(
        out.status.success(),
        "gcc rejected the C for `{}`:\n{}",
        name,
        String::from_utf8_lossy(&out.stderr)
    );
    Command::new(&exe).output().expect("the program should run")
}

const TWO_VARIANTS: &str = r#"
enum E {
    A,
    B,
}

fn main() {
    let e = E::A;
    match e {
        E::A => { print("took A"); }
        E::B => { print("took B"); }
    }
    print("after the match");
}
"#;

/// THE ROW'S OBSERVABLE. A `match` handed a value no arm matches must stop the
/// program.
///
/// The corruption is one assignment inserted into the generated C, right after
/// the scrutinee is read into its temporary: `_match_expr.tag = 99;`. Nothing
/// else is touched — the arms, the trap and the runtime are the compiler's own
/// output. Before the trap existed this program printed "after the match" and
/// exited 0, having silently done nothing.
#[test]
fn unmatched_scrutinee_traps() {
    let dir = TempDir::new().unwrap();
    let c = generate_c(dir.path(), "n6_trap", TWO_VARIANTS);

    let anchor = "struct E _match_expr = e;";
    assert!(
        c.contains(anchor),
        "the generated C no longer reads the scrutinee into `_match_expr`, so this \
         test is corrupting nothing:\n{}",
        c
    );
    let corrupted = c.replacen(
        anchor,
        &format!("{}\n        _match_expr.tag = 99;", anchor),
        1,
    );

    let run = build_and_run(dir.path(), "n6_trap", &corrupted);
    let stdout = String::from_utf8_lossy(&run.stdout);
    let stderr = String::from_utf8_lossy(&run.stderr);

    assert!(
        !run.status.success(),
        "a `match` that took no arm exited successfully — it fell through instead \
         of trapping.\nstdout: {}\nstderr: {}",
        stdout,
        stderr
    );
    assert!(
        stderr.contains("no match arm was taken"),
        "the trap must SAY what happened; a bare abort is a crash the reader has to \
         guess at.\nstderr: {}",
        stderr
    );
    assert!(
        stderr.contains("main"),
        "the trap must name where it fired, or it cannot be attributed in a program \
         with more than one `match`.\nstderr: {}",
        stderr
    );
    assert!(
        !stdout.contains("after the match"),
        "the program continued past the `match` it fell out of, which is the exact \
         behaviour N6-11 forbids.\nstdout: {}",
        stdout
    );
}

/// The unguarded shape ends in an `else`, and that is what makes it total.
#[test]
fn the_chain_form_ends_in_a_trapping_else() {
    let dir = TempDir::new().unwrap();
    let c = generate_c(dir.path(), "n6_chain", TWO_VARIANTS);
    assert!(
        c.contains("} else {\n            __pd_match_trap("),
        "the if/else-if chain no longer ends in a trapping `else`:\n{}",
        c
    );
}

/// The GUARDED shape is the one that armed the linker, and it earns that with a
/// label rather than a flag.
///
/// With the `int done` flag this form used to carry, the trap sat behind
/// `if (!done)` and gcc could not prove the end of a tail `match` was
/// unreachable — so `-Werror=return-type` would have rejected this very program.
/// The `goto` past the trap is what makes the fall-through path unconditional.
#[test]
fn the_guarded_form_traps_before_its_label() {
    let dir = TempDir::new().unwrap();
    let c = generate_c(
        dir.path(),
        "n6_guarded",
        r#"
fn describe(n: i64) -> i64 {
    match n {
        m if m > 10 => 1,
        m if m > 5 => 2,
        _ => 3,
    }
}

fn main() {
    print_int(describe(12));
}
"#,
    );
    assert!(
        c.contains("__pd_match_trap(") && c.contains("goto _match_end"),
        "the guarded form should trap and then be jumped past by its arms:\n{}",
        c
    );
    let trap = c.find("__pd_match_trap(\"describe").expect("trap in describe");
    let label = c.find("_match_end0: ;").expect("the arms' target label");
    assert!(
        trap < label,
        "the trap must sit BEFORE the label, so the fall-through path reaches it \
         unconditionally; behind a flag or after the label it proves nothing to gcc"
    );
    assert!(
        !c.contains("int _match_done"),
        "the `done` flag is back — it is what kept -Werror=return-type out of the \
         linker:\n{}",
        c
    );
}

/// N6-10 is why the trap should never fire, and the two rows are one story: the
/// checker refuses what it cannot prove complete, and the trap catches what the
/// checker could not have known.
#[test]
fn the_checker_refuses_what_the_trap_would_catch() {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join("n6_nonexhaustive.pd");
    fs::write(
        &src,
        r#"
fn main() {
    let v = match 4 {
        1 => 10,
        2 => 20,
    };
    print_int(v);
}
"#,
    )
    .unwrap();
    let err = Driver::new()
        .compile_file(&src)
        .expect_err("a non-exhaustive int match must be refused (N6-10)");
    let text = format!("{}", err);
    assert!(
        text.contains("Non-exhaustive match"),
        "expected the non-exhaustive refusal, got: {}",
        text
    );
    assert!(
        text.contains("_"),
        "the refusal must say what to add, not only that something is missing: {}",
        text
    );
}
