//! N4-04 / N14-04: the two runtime refusals the char type rests on.
//!
//! Both replace a SILENT WRONG ANSWER, and neither can be asserted from inside
//! the language — a trap ends the process, so no Palladium fixture can observe
//! it. The conformance manifest has no runtime-trap class either (its classes
//! are run / untranscribed / vacuous / xfail / reject / skip), so these live
//! here, where a test can read an exit status and a stderr line.
//!
//! Until this file existed the traps were unpinned. Both were reachable, both
//! fired, and nothing would have gone red if either had been reverted.

mod common;

use common::unique_module_name;
use palladium::linker::{link_command, OptLevel};
use palladium::Driver;
use std::fs;
use std::process::{Command, Output};
use tempfile::TempDir;

/// Compile, link and run, returning the raw `Output` — including a failing
/// status, which is the whole point here.
fn compile_and_capture(source: &str, name: &str) -> Output {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    let exe = dir.path().join(name);
    fs::write(&src, source).unwrap();

    let c_file = Driver::new()
        .compile_file(&src)
        .unwrap_or_else(|e| panic!("compilation failed: {}", e));
    let out = link_command(&c_file, &exe, OptLevel::Default)
        .expect("link_command")
        .output()
        .expect("gcc");
    assert!(
        out.status.success(),
        "gcc rejected the C: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Command::new(&exe).output().expect("run")
}

fn assert_trapped(out: &Output, needle: &str) {
    assert!(
        !out.status.success(),
        "the program exited 0; it was supposed to trap.\nstdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains(needle),
        "expected {:?} on stderr, got: {}",
        needle,
        stderr
    );
}

/// N14-04. `string_char_at` returns a `char`, and -1 is not one — so an index
/// with no character behind it traps instead of answering a sentinel nobody
/// read. MEASURED before the type split: the call returned -1, and
/// `string_from_char(-1)` wrote the low byte.
#[test]
fn string_char_at_traps_past_the_end() {
    let out = compile_and_capture(
        "fn main() { print(string_from_char(string_char_at(\"abc\", 3))); }",
        &unique_module_name("char_at_len"),
    );
    assert_trapped(&out, "string_char_at index 3 is outside a string of length 3");
}

/// The other edge, and the one an off-by-one lands on from the other side.
#[test]
fn string_char_at_traps_on_a_negative_index() {
    let out = compile_and_capture(
        "fn main() { print(string_from_char(string_char_at(\"abc\", 0 - 1))); }",
        &unique_module_name("char_at_neg"),
    );
    assert_trapped(&out, "string_char_at index -1 is outside a string of length 3");
}

/// The last valid index is NOT a trap — the control that stops the guard from
/// being widened into a refusal of ordinary programs.
#[test]
fn the_last_valid_index_still_answers() {
    let out = compile_and_capture(
        "fn main() { print(string_from_char(string_char_at(\"abc\", 2))); }",
        &unique_module_name("char_at_last"),
    );
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "c");
}

/// N4-04. `as char` is the only door into the type from a number, so it is
/// where the domain is enforced. A UTF-16 surrogate is not a Unicode scalar.
/// MEASURED before the check: `55296 as char` compiled, and
/// `string_from_char` printed an EMPTY LINE.
///
/// THE VALUE IS COMPUTED, not written down, and that is the point of the test.
/// A literal operand is refused by the type checker
/// (`tests/reject/char_from_non_scalar.pd`); this is the half that only exists
/// at run time, which is why the trap has to be there as well.
#[test]
fn as_char_traps_on_a_surrogate() {
    let out = compile_and_capture(
        "fn main() { let n: i64 = 55000 + 296; let c: char = n as char; \
         print(string_from_char(c)); }",
        &unique_module_name("char_surrogate"),
    );
    assert_trapped(&out, "55296 is not a Unicode scalar");
}

/// Past U+10FFFF, again computed. MEASURED before the check: `99999999 as char`
/// compiled and printed a garbage byte, because `string_from_char` writes
/// `(char)c` and asks nothing about the other 56 bits.
#[test]
fn as_char_traps_above_the_last_scalar() {
    let out = compile_and_capture(
        "fn main() { let n: i64 = 99999999 + 0; let c: char = n as char; \
         print(string_from_char(c)); }",
        &unique_module_name("char_too_big"),
    );
    assert_trapped(&out, "99999999 is not a Unicode scalar");
}

/// And the controls: the boundaries that are legal must stay legal, or the
/// guard has been widened into a refusal of correct programs. U+10FFFF is the
/// last scalar; U+E000 is the first one above the surrogate block.
#[test]
fn the_scalar_boundaries_are_not_trapped() {
    let out = compile_and_capture(
        "fn main() {\n\
             let hi: char = 1114111 as char;\n\
             let after: char = 57344 as char;\n\
             let zero: char = 0 as char;\n\
             let computed: char = (55000 + 3000) as char;\n\
             print_int(computed as i64);\n\
             print_int(hi as i64);\n\
             print_int(after as i64);\n\
             print_int(zero as i64);\n\
         }",
        &unique_module_name("char_bounds"),
    );
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("1114111"), "{}", stdout);
    assert!(stdout.contains("57344"), "{}", stdout);
}
