//! Char literals as PATTERNS (N6-02's literal set, N6-03's ranges), end to end.
//!
//! WHY THIS FILE EXISTS BESIDE `tests/06_char_patterns.pd`. The conformance
//! fixture pins the transcript; these tests pin the two things a transcript
//! cannot show. The first is the LOWERING: a `char` is a `long long` holding a
//! Unicode scalar, so a char pattern must become a numeric comparison against
//! that scalar — and a fixture that prints the right number cannot tell that
//! from a C character constant, which would be the execution charset's encoding
//! rather than the language's. The second is the REFUSALS' shape.
//!
//! COMPILE AND RUN, NEVER COMPILE ALONE. The WT-01 census's standing lesson is
//! that front-end approval is not an artifact: every positive test here links
//! and executes, because the defect class this feature sits in is C that the
//! front end was happy to emit.

mod common;

use common::unique_module_name;
use palladium::linker::{link_command, OptLevel};
use palladium::Driver;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Compile, link, run — and return stdout. Any failure on the way is the test's
/// failure, named at the step it happened.
fn run(source: &str, name: &str) -> String {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    let exe = dir.path().join(name);
    fs::write(&src, source).unwrap();

    let c_file = Driver::new()
        .compile_file(&src)
        .unwrap_or_else(|e| panic!("the front end refused a legal program: {}", e));
    let out = link_command(&c_file, &exe, OptLevel::Default)
        .expect("link_command")
        .output()
        .expect("gcc");
    assert!(
        out.status.success(),
        "the front end accepted this and gcc refused the C it emitted: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let run = Command::new(&exe).output().expect("run");
    assert!(
        run.status.success(),
        "the program did not exit 0: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    String::from_utf8_lossy(&run.stdout).to_string()
}

/// The emitted C for the same source, for the lowering assertions.
fn emitted_c(source: &str, name: &str) -> String {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    fs::write(&src, source).unwrap();
    let c_file = Driver::new().compile_file(&src).expect("compile");
    fs::read_to_string(&c_file).expect("read emitted C")
}

fn refusal(source: &str, name: &str) -> String {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    fs::write(&src, source).unwrap();
    match Driver::new().compile_file(&src) {
        Ok(_) => panic!("this was supposed to be refused, and it compiled"),
        Err(e) => e.to_string(),
    }
}

const DISPATCH: &str = r#"
fn classify(c: char) -> i64 {
    match c {
        '0'..='9' => { return 1; }
        'a'..='z' => { return 2; }
        ' ' | '\t' => { return 3; }
        '!' => { return 4; }
        _ => { return 0; }
    }
}
fn main() {
    print_int(classify('7'));
    print_int(classify('q'));
    print_int(classify(' '));
    print_int(classify('\t'));
    print_int(classify('!'));
    print_int(classify('Z'));
}
"#;

/// The byte dispatcher the WT-01 census found unwritable, written.
#[test]
fn a_char_dispatcher_compiles_links_and_runs() {
    let out = run(DISPATCH, &unique_module_name("charpat_dispatch"));
    assert_eq!(
        out, "1\n2\n3\n3\n4\n0\n",
        "the arms did not fire as written"
    );
}

/// THE LOWERING, which the transcript cannot distinguish from a C char constant.
///
/// `'a'` must reach C as the scalar 97, not as `'a'`: a C character constant is
/// an `int` in the *execution charset*, which is not required to be ASCII, and
/// no C character constant exists for a scalar above 0xFF at all.
#[test]
fn a_char_pattern_lowers_to_its_scalar_not_to_a_c_character_constant() {
    let c = emitted_c(DISPATCH, &unique_module_name("charpat_lower"));
    assert!(
        c.contains("== 33 /* '!' */"),
        "the literal arm did not lower to its scalar:\n{}",
        c
    );
    assert!(
        c.contains(">= 97 /* 'a' */") && c.contains("<= 122 /* 'z' */"),
        "the range arm did not lower to scalar bounds:\n{}",
        c
    );
    assert!(
        c.contains("== 9 /* '\\t' */"),
        "the escape in an or-pattern did not survive to the C:\n{}",
        c
    );
    assert!(
        !c.contains("== 'a'") && !c.contains(">= 'a'"),
        "a C character constant reached the output; the scalar is the value \
         this language defines:\n{}",
        c
    );
}

/// A binding over a char range, read back through `as i64`.
#[test]
fn a_binding_over_a_char_range_binds_the_matched_char() {
    let out = run(
        r#"
fn main() {
    let c: char = 'k';
    match c {
        d @ 'a'..='z' => { print_int(d as i64); }
        _ => { print_int(0); }
    }
}
"#,
        &unique_module_name("charpat_bind"),
    );
    assert_eq!(out, "107\n", "`k` is code point 107");
}

/// A char match in VALUE position, which is a different lowering path.
#[test]
fn a_char_match_in_value_position_runs() {
    let out = run(
        r#"
fn main() {
    let c: char = 'm';
    let v: i64 = match c {
        'a'..='z' => { 10 }
        _ => { 20 }
    };
    print_int(v);
}
"#,
        &unique_module_name("charpat_value"),
    );
    assert_eq!(out, "10\n");
}

/// N4-04 in pattern position: in C both sides are a `long long`, so this would
/// compile and silently test `n == 97` if the type checker let it through.
#[test]
fn a_char_pattern_against_an_int_scrutinee_is_refused() {
    let e = refusal(
        "fn main() { let n: i64 = 5; match n { 'a' => { print(\"x\"); } _ => { print(\"y\"); } } }",
        &unique_module_name("charpat_scrut"),
    );
    assert!(
        e.contains("a pattern of type Int") && e.contains("the Char literal `'a'`"),
        "the refusal did not name both sides: {}",
        e
    );
}

/// One end of each ordered kind. Refused by name rather than ordered through a
/// conversion N4-04 forbids in both directions.
#[test]
fn a_range_with_one_char_end_and_one_int_end_is_refused() {
    let e = refusal(
        "fn main() { let c: char = 'a'; match c { 'a'..=9 => { print(\"x\"); } _ => { print(\"y\"); } } }",
        &unique_module_name("charpat_mixed"),
    );
    assert!(
        e.contains("the same kind of literal"),
        "the refusal did not name the mixed endpoints: {}",
        e
    );
}

/// The empty-range rule applies to char by CODE POINT, and says so.
#[test]
fn an_empty_char_range_is_refused_and_names_the_order_it_used() {
    let e = refusal(
        "fn main() { let c: char = 'a'; match c { 'z'..='a' => { print(\"x\"); } _ => { print(\"y\"); } } }",
        &unique_module_name("charpat_empty"),
    );
    assert!(
        e.contains("which is empty") && e.contains("code point"),
        "the refusal did not say which order it applied: {}",
        e
    );
}

/// A string endpoint stays refused: `char` joining the ordered kinds did not
/// widen the rule to every literal.
#[test]
fn a_string_endpoint_is_still_refused() {
    let e = refusal(
        "fn main() { let v = match 4 { \"a\"..\"z\" => 1, _ => 0 }; print_int(v); }",
        &unique_module_name("charpat_str"),
    );
    assert!(
        e.contains("integer or `char` literals"),
        "the refusal did not name the two kinds it accepts: {}",
        e
    );
}

/// Char is not enumerable, so a char match without a wildcard is non-exhaustive
/// — the same answer an integer match gets, and the reason `PatternKind` needed
/// no new variant.
#[test]
fn a_char_match_without_a_wildcard_is_non_exhaustive() {
    let e = refusal(
        "fn main() { let c: char = 'a'; match c { 'a' => { print(\"x\"); } } }",
        &unique_module_name("charpat_exh"),
    );
    assert!(
        e.contains("Non-exhaustive"),
        "a char match with one arm was accepted: {}",
        e
    );
}
