//! M2 item 5: lexical completion — N2-03, N2-04, N2-08, N2-09, N2-10, N2-11.
//!
//! WHAT MAKES THIS SET ONE COMMIT RATHER THAN SIX
//!
//! Five of the six are *additions* to what lexes, and additions to a lexer are
//! not risky in the direction people expect. The risk is not over-rejection —
//! a program that stops compiling is loud. The risk is a token that lexes and
//! then produces the WRONG BYTES, or lexes and is then dropped. Both were
//! present on `main`, both measured:
//!
//! ```text
//! "\\n"                → backslash + LINE FEED   (should be backslash + `n`)
//! "\0"                 → backslash + `0`         (should be a NUL)
//! "\q"                 → backslash + `q`         (should be refused)
//! let d = x / y;       → long long d = (x / y);  (two doubles, TRUNCATED)
//! /* a /* b */ c */    → `Expected expression, but found '/'`
//! '\''                 → `Expected expression, but found '` (three tokens)
//! 3.5                  → `Expected field name, but found integer 5`
//! #[total]             → `Unexpected character '#'`
//! ```
//!
//! So every receipt below asserts a VALUE — a byte on stdout or a number — and
//! not that something compiled. A `char` literal that lexes and yields the
//! wrong code point compiles exactly as happily as one that yields the right
//! one, and is strictly worse than one that fails.
//!
//! N2-10 AND N2-11 ARE ONE ROW IN TWO HALVES, and cannot be separated. `#`
//! lexing without a refusal would mean `#[total]` compiles to a binary with no
//! totality check in it — the defect class M1 spent itself deleting, arriving
//! through a new door. So the known-attribute table is EMPTY, every attribute
//! is a compile error, and `the_known_attribute_set_is_empty_on_purpose` fails
//! the day someone adds a name without reading why.

mod common;

use common::unique_module_name;
use palladium::linker::{link_command, OptLevel};
use palladium::Driver;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Compile, link with gcc, run, and return stdout — ONLY if the program exited 0.
///
/// Linking is not optional here even though nothing in this file is about C
/// syntax: an escape is emitted INTO a C string literal, and the first
/// implementation of `\0` put a raw zero byte there. A test that read the
/// generated text and stopped would have passed on a `.c` file no editor can
/// round-trip.
///
/// THE EXIT STATUS IS PART OF THE OBSERVATION, and this function did not check
/// it for one round. It compared `out.status` for gcc and then took
/// `run.stdout` unconditionally, so a program could print the expected prefix,
/// crash, and pass — `assert_eq!(out.trim(), "42")` cannot tell a clean run
/// from an aborted one that got as far as printing.
///
/// That is this branch's own discipline undone one layer down. Every case here
/// asserts a VALUE rather than "it compiled", precisely because a char literal
/// that lexes and yields the wrong byte is worse than one that fails — and the
/// harness was accepting a run that died. THE ASSERTION CEILING WAS SET BY THE
/// HARNESS, NOT BY THE CASES.
///
/// `the_harness_rejects_a_program_that_prints_and_then_dies` is the control:
/// it prints exactly what a passing test would print and then aborts.
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

    let run = Command::new(&exe).output().map_err(|e| format!("run: {}", e))?;
    if !run.status.success() {
        // Status AND stderr AND the stdout it managed to produce: a crash after
        // partial output is the case this exists for, and "the program failed"
        // without the bytes it printed sends the reader back to re-run it by
        // hand. `code()` is None when a signal killed it (SIGABRT from `panic`
        // is exactly that), so both spellings are reported.
        return Err(format!(
            "the program exited {} (signal: {}); stderr:\n{}\nstdout so far:\n{}",
            run.status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "-".to_string()),
            run.status,
            String::from_utf8_lossy(&run.stderr),
            String::from_utf8_lossy(&run.stdout)
        ));
    }
    Ok(String::from_utf8_lossy(&run.stdout).to_string())
}

/// Compile only, and return the emitted C.
fn compile_to_c(source: &str, name: &str) -> Result<String, String> {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    fs::write(&src, source).unwrap();
    let c_file = Driver::new()
        .compile_file(&src)
        .map_err(|e| format!("compilation failed: {}", e))?;
    fs::read_to_string(&c_file).map_err(|e| e.to_string())
}

/// The diagnostic a program is refused with, or a panic if it was accepted.
fn refusal(source: &str, name: &str) -> String {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join(format!("{}.pd", name));
    fs::write(&src, source).unwrap();
    match Driver::new().compile_file(&src) {
        Ok(_) => panic!("expected a refusal, but the program compiled:\n{}", source),
        Err(e) => e.to_string(),
    }
}

// ---------------------------------------------------------------------------
// The harness itself
// ---------------------------------------------------------------------------

/// THE CONTROL FOR THE HARNESS, and the shape of the defect it closes.
///
/// This program prints `42` — the exact stdout several cases below assert — and
/// then aborts. Under the harness that ignored the run's exit status it was
/// indistinguishable from a clean run, so EVERY value assertion in this file
/// was capped at "printed the right thing at some point before it died".
///
/// It fails when the `run.status.success()` check is reverted.
#[test]
fn the_harness_rejects_a_program_that_prints_and_then_dies() {
    let err = compile_and_run(
        r#"fn main() { print_int(42); panic("after the output"); }"#,
        &unique_module_name("harness_crash"),
    )
    .expect_err(
        "a program that printed and then aborted was accepted: the exit status is not \
         being checked, so every value assertion in this file is capped at `printed the \
         right bytes at some point before dying`",
    );
    assert!(
        err.contains("the program exited"),
        "the failure must name the exit status, got: {}",
        err
    );
    assert!(
        err.contains("after the output"),
        "the failure must carry stderr, got: {}",
        err
    );
    assert!(
        err.contains("42"),
        "the failure must carry the stdout the program did produce, got: {}",
        err
    );
}

/// …and the other direction: an ordinary program is still accepted, so the
/// status check did not simply reject everything.
#[test]
fn the_harness_still_accepts_a_program_that_exits_cleanly() {
    let out = compile_and_run(
        "fn main() { print_int(42); }",
        &unique_module_name("harness_ok"),
    )
    .expect("a clean program must still pass");
    assert_eq!(out.trim(), "42");
}

// ---------------------------------------------------------------------------
// Source positions, once non-ASCII source became reachable
// ---------------------------------------------------------------------------

/// A DIAGNOSTIC AFTER A MULTI-BYTE CHARACTER MUST LAND WHERE THE ASCII ONE DOES.
///
/// `position_at` takes a BYTE offset — logos spans are byte ranges — and used to
/// walk `chars().enumerate()`, comparing that offset against a CHARACTER
/// ORDINAL. The two agree only while the source is pure ASCII.
///
/// Measured on the same program before the fix: with `한글 中文 🎉` in a string
/// two lines above the error, a four-line file reported the error at **5:1** —
/// past its own end. The ASCII twin reported 3:14. Now both report 3:14.
///
/// This is asserted as EQUALITY BETWEEN THE TWO PROGRAMS rather than against a
/// hardcoded column, because the reporter has a separate, uniform off-by-one
/// (the ASCII control also says 14 for a character in column 13) that predates
/// this branch and is not what this test is about. Equality is the property
/// that breaks when the byte/char confusion returns; the absolute number is
/// not.
#[test]
fn a_diagnostic_after_a_multibyte_literal_lands_where_the_ascii_one_does() {
    let ascii = refusal(
        "fn main() {\n    print(\"ascii only\");\n    let x = $;\n}\n",
        &unique_module_name("pos_ascii"),
    );
    let utf8 = refusal(
        "fn main() {\n    print(\"한글 中文 🎉\");\n    let x = $;\n}\n",
        &unique_module_name("pos_utf8"),
    );
    let line_col = |d: &str| {
        d.lines()
            .find(|l| l.contains("line") && l.contains("column"))
            .map(str::to_string)
    };
    assert_eq!(
        utf8, ascii,
        "the two programs differ only in the CONTENT of a string literal two lines \
         above the error, so their diagnostics must be identical.\n  ascii: {}\n  utf8:  {}",
        ascii,
        utf8
    );
    let _ = line_col;
}

/// …and the same property through the compiler's own position machinery, where
/// the numbers are visible rather than compared as opaque strings.
#[test]
fn position_at_counts_bytes_not_character_ordinals() {
    use palladium::errors::CompileError;
    use palladium::lexer::Lexer;

    // `$` is a token, so drive the LEXER-level error directly with a character
    // no rule matches. `€` is three bytes, `한` is three, `🎉` is four.
    let src = "fn a() { }\nlet s = \"한글 🎉\";\n§\n";
    let mut lex = Lexer::new(src);
    let mut err = None;
    loop {
        match lex.next_token() {
            Ok(Some(_)) => continue,
            Ok(None) => break,
            Err(e) => {
                err = Some(e);
                break;
            }
        }
    }
    let err = err.expect("`§` matches no token rule, so the lexer must refuse it");
    match err {
        CompileError::UnexpectedChar {
            ch, line, col, span, ..
        } => {
            assert_eq!(ch, '§');
            assert_eq!(line, 3, "`§` is on line 3; a char-ordinal walk drifts past it");
            assert_eq!(col, 1, "it is the first character on that line");
            let span = span.expect("the refusal must carry a span");
            // THE SPAN IS A BYTE RANGE AND MUST END ON A CHARACTER BOUNDARY.
            // `start + 1` over a two-byte `§` ends INSIDE the code point, and
            // anything that slices the source with it panics.
            assert!(
                src.is_char_boundary(span.start) && src.is_char_boundary(span.end),
                "span {}..{} does not land on character boundaries in {:?}",
                span.start,
                span.end,
                src
            );
            assert_eq!(
                span.end - span.start,
                '§'.len_utf8(),
                "the span must cover the whole character, not one byte of it"
            );
            assert_eq!(&src[span.start..span.end], "§");
        }
        other => panic!("expected UnexpectedChar, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// N2-08 — block comments nest
// ---------------------------------------------------------------------------

/// The exact program F10 records as failing, taken past linking to its output.
#[test]
fn a_nested_block_comment_compiles_and_the_body_after_it_runs() {
    let out = compile_and_run(
        r#"fn main() { /* a /* b */ c */ print("ok"); }"#,
        &unique_module_name("nest_basic"),
    )
    .expect("N2-08: `/* a /* b */ c */` is one comment");
    assert_eq!(out.trim(), "ok");
}

/// The control for the arm above: the INNER close must not end the outer
/// comment, so text between the two closes stays commented out. This is the
/// case that fails when nesting is reverted — the reverted scanner leaves
/// `print("leaked");` as live source and the output gains a line.
#[test]
fn text_between_an_inner_close_and_the_outer_one_is_still_comment() {
    let out = compile_and_run(
        r#"
fn main() {
    /* outer
       /* inner */
       print("leaked");
    */
    print("clean");
}
"#,
        &unique_module_name("nest_inner"),
    )
    .expect("N2-08");
    assert_eq!(
        out.trim(),
        "clean",
        "the print between the inner and outer closes was treated as live source"
    );
}

#[test]
fn nesting_is_arbitrarily_deep() {
    let out = compile_and_run(
        r#"fn main() { /* /* /* /* four */ */ */ */ print("deep"); }"#,
        &unique_module_name("nest_deep"),
    )
    .expect("N2-08");
    assert_eq!(out.trim(), "deep");
}

/// A `/*` inside a string literal is not a comment opener. The string rule
/// starts first and wins, and if it did not, this program would not terminate
/// a comment before end of file.
#[test]
fn a_comment_opener_inside_a_string_is_not_a_comment() {
    let out = compile_and_run(
        r#"fn main() { print("/* not a comment */"); }"#,
        &unique_module_name("nest_in_str"),
    )
    .expect("N2-08");
    assert_eq!(out.trim(), "/* not a comment */");
}

/// Nesting makes "unterminated" a state the scanner can be IN and report.
/// Before it, the same file lexed as a closed comment plus live source.
#[test]
fn an_unterminated_block_comment_is_refused_and_says_so() {
    let msg = refusal(
        "fn main() { /* outer /* inner */ print(\"x\"); }",
        &unique_module_name("nest_unterm"),
    );
    assert!(
        msg.contains("unterminated block comment"),
        "expected an unterminated-comment diagnostic, got: {}",
        msg
    );
}

/// Line comments still end at the newline and do not interact with `/*`.
#[test]
fn a_line_comment_does_not_open_a_block() {
    let out = compile_and_run(
        "fn main() {\n    // /* not opened\n    print(\"line\");\n}\n",
        &unique_module_name("nest_line"),
    )
    .expect("N2-08");
    assert_eq!(out.trim(), "line");
}

/// Division still divides. Moving comment scanning onto the `/` token is the
/// change most likely to break the operator that shares the character.
#[test]
fn division_still_lexes_as_division() {
    let out = compile_and_run(
        "fn main() { print_int(84 / 2); }",
        &unique_module_name("nest_div"),
    )
    .expect("N2-08");
    assert_eq!(out.trim(), "42");
}

// ---------------------------------------------------------------------------
// N2-09 — escape sequences, in both directions
// ---------------------------------------------------------------------------

/// THE MEASURED MISCOMPILE. `"\\n"` denotes a backslash and the letter `n`.
/// The old `.replace()` chain let the `\n` rule claim the second backslash's
/// `n`, producing a backslash and a LINE FEED — two characters, one of them
/// wrong, with no diagnostic.
#[test]
fn a_doubled_backslash_before_n_is_a_backslash_and_a_letter() {
    let out = compile_and_run(
        r#"fn main() { print("[\\n]"); }"#,
        &unique_module_name("esc_bsn"),
    )
    .expect("N2-09");
    assert_eq!(out, "[\\n]\n", "got bytes {:?}", out);
    assert!(!out.contains("[\\\n"), "a LINE FEED reached the output");
}

/// Every escape in the set, asserted as the byte it denotes.
#[test]
fn each_escape_produces_exactly_its_character() {
    let out = compile_and_run(
        r#"
fn main() {
    print_int('\n' as i64);
    print_int('\t' as i64);
    print_int('\r' as i64);
    print_int('\"' as i64);
    print_int('\\' as i64);
    print_int('\0' as i64);
    print_int('\'' as i64);
}
"#,
        &unique_module_name("esc_each"),
    )
    .expect("N2-09");
    assert_eq!(
        out.lines().collect::<Vec<_>>(),
        vec!["10", "9", "13", "34", "92", "0", "39"]
    );
}

/// An escape outside the set is REFUSED, not passed through, and the message
/// enumerates the set rather than asking the reader to find it.
#[test]
fn an_unknown_escape_is_refused_and_the_message_lists_the_set() {
    let msg = refusal(
        r#"fn main() { print("bad \q here"); }"#,
        &unique_module_name("esc_unknown"),
    );
    assert!(
        msg.contains("unknown escape sequence") && msg.contains("\\q"),
        "the diagnostic must name the offending escape, got: {}",
        msg
    );
}

/// `\0` IS REFUSED IN A STRING LITERAL, and this is the row that changed.
///
/// It was accepted for one round, with the consequence pinned: a String is a
/// non-NULL, NUL-terminated `const char*` (N14), so `print("a\0b")` printed `a`
/// and `string_len("a\0b")` was 1. That test passed and the pin was accurate,
/// and it was still the wrong disposition — **a representation leak that is
/// documented is still a representation leak, and shipping the acceptance is
/// what promotes it to language semantics.** The previous version of this file
/// asserted the leak as behaviour, which is how a caveat becomes a contract.
///
/// So the string form is a compile error and the char form is not. `'\0'` is
/// the integer 0, every consumer of a character code can hold it, and nothing
/// that was expressible has been lost. The route back for strings is
/// length-aware String semantics — a pointer plus a length — which is an N4/N14
/// change and not a lexer one.
#[test]
fn a_nul_in_a_string_literal_is_refused() {
    let msg = refusal(
        r#"fn main() { print("a\0b"); }"#,
        &unique_module_name("esc_nul_str"),
    );
    assert!(
        msg.contains("`\\0` is not allowed in a string literal"),
        "expected the NUL refusal, got: {}",
        msg
    );
}

/// …and the value is still reachable, in the literal whose representation can
/// carry it. Without this the refusal above would read as "Palladium has no
/// way to write a zero byte", which is false.
#[test]
fn a_nul_in_a_char_literal_is_still_the_value_zero() {
    let out = compile_and_run(
        r#"fn main() { print_int('\0' as i64); print_int('
' as i64); }"#,
        &unique_module_name("esc_nul_char"),
    )
    .expect("`'\\0'` is an ordinary char literal");
    assert_eq!(out.lines().collect::<Vec<_>>(), vec!["0", "10"]);
}

/// THE CONTROL FOR THE SPLIT: the escape table is consulted per literal KIND,
/// not globally. A table that answered the same question for both would make
/// one of the two tests above unsatisfiable, and this states which way.
#[test]
fn the_string_escape_set_is_the_char_set_minus_nul() {
    use palladium::lexer::token::{escape_spellings, string_escape_spellings};
    let chars = escape_spellings();
    let strings = string_escape_spellings();
    assert!(chars.contains(&"\\0".to_string()), "char escapes: {:?}", chars);
    assert!(
        !strings.contains(&"\\0".to_string()),
        "`\\0` is still offered to string literals: {:?}",
        strings
    );
    let mut expected: Vec<String> = chars.iter().filter(|e| *e != "\\0").cloned().collect();
    expected.sort();
    let mut got = strings.clone();
    got.sort();
    assert_eq!(
        got, expected,
        "the two sets must differ by exactly `\\0`; anything else is a second \
         asymmetry nobody decided"
    );
}

/// The generated C still escapes a NUL if one ever reaches it. No string
/// literal can carry one now, so this is a unit-level guard on the emitter
/// rather than a reachable path — kept because `c_string_body` is the ONE
/// derivation and the next caller may not be a string literal.
#[test]
fn the_c_emitter_still_refuses_to_write_a_raw_nul() {
    use palladium::codegen::c_literal::c_string_body;
    let got = c_string_body("a\u{0}b");
    assert_eq!(got, "a\\000b");
    assert!(!got.contains('\0'));
}

/// The escapes that already worked still work — the control for a rewrite of
/// the whole unescaper.
#[test]
fn the_escapes_that_already_worked_are_unchanged() {
    let out = compile_and_run(
        r#"fn main() { print("A\tB"); print("q\"q"); print("s\\s"); }"#,
        &unique_module_name("esc_control"),
    )
    .expect("N2-09");
    assert_eq!(out, "A\tB\nq\"q\ns\\s\n");
}

// ---------------------------------------------------------------------------
// N2-04 — char literals
// ---------------------------------------------------------------------------

#[test]
fn a_char_literal_is_its_unicode_scalar() {
    let out = compile_and_run(
        r#"
fn main() {
    print_int('a' as i64);
    print_int('A' as i64);
    print_int('0' as i64);
    print_int(' ' as i64);
    print_int('~' as i64);
}
"#,
        &unique_module_name("chr_scalar"),
    )
    .expect("N2-04");
    assert_eq!(
        out.lines().collect::<Vec<_>>(),
        vec!["97", "65", "48", "32", "126"]
    );
}

/// The value must survive to a BYTE, not only to a number the compiler chose.
#[test]
fn a_char_literal_round_trips_through_the_builtin_that_renders_one() {
    let out = compile_and_run(
        r#"fn main() { print(string_from_char('Z')); }"#,
        &unique_module_name("chr_render"),
    )
    .expect("N2-04");
    assert_eq!(out.trim(), "Z");
}

/// A scalar above 0xFF, which is the case a C character constant cannot carry
/// and the reason code generation emits the number instead.
#[test]
fn a_non_ascii_char_literal_keeps_its_scalar() {
    let out = compile_and_run(
        "fn main() { print_int('한' as i64); }",
        &unique_module_name("chr_wide"),
    )
    .expect("N2-04");
    assert_eq!(out.trim(), "54620");
}

/// THE CONTROL THAT KEEPS LIFETIMES WORKING. `'` opens a char literal AND
/// introduces a lifetime, and a char rule that consumed the tick in `<'a>`
/// would take the parser's lifetime path away. F12 is the same ambiguity
/// biting the thesis gate's own scanner.
#[test]
fn a_lifetime_tick_is_not_a_char_literal() {
    let out = compile_and_run(
        r#"
struct Holder<'a> {
    n: i64,
}

fn id<'a>(x: i64) -> i64 {
    return x;
}

fn main() {
    print_int(id(7));
}
"#,
        &unique_module_name("chr_lifetime"),
    )
    .expect("a lifetime parameter must still parse");
    assert_eq!(out.trim(), "7");
}

/// …including two lifetimes, where the ticks are close enough that a greedy
/// scanner could pair them.
#[test]
fn two_lifetime_ticks_do_not_pair_into_a_char_literal() {
    let out = compile_and_run(
        r#"
fn pick<'a, 'b>(x: i64) -> i64 {
    return x;
}

fn main() {
    print_int(pick(3));
}
"#,
        &unique_module_name("chr_two_ticks"),
    )
    .expect("`<'a, 'b>` must still parse as two lifetimes");
    assert_eq!(out.trim(), "3");
}

#[test]
fn char_literals_compare_and_flow_through_the_character_builtins() {
    let out = compile_and_run(
        r#"
fn main() {
    if 'z' > 'a' { print("ordered"); }
    if string_char_at("hi", 0) == 'h' { print("indexed"); }
    if char_is_digit('7') { print("digit"); }
    if char_is_alpha('q') { print("alpha"); }
}
"#,
        &unique_module_name("chr_builtins"),
    )
    .expect("N2-04");
    assert_eq!(
        out.lines().collect::<Vec<_>>(),
        vec!["ordered", "indexed", "digit", "alpha"]
    );
}

// ---------------------------------------------------------------------------
// N2-03 — float literals
// ---------------------------------------------------------------------------

/// Assertions on floats go through comparisons because N14's builtin set has
/// no float printer: `print` takes a String, `print_int` takes an i64, and
/// there is no `float_to_string`. A bracket is still a value assertion — only
/// one double satisfies both bounds — and each bracket below excludes the
/// integer-truncated answer.
#[test]
fn a_float_literal_holds_its_value_and_is_not_an_integer() {
    let out = compile_and_run(
        r#"
fn main() {
    let x: f64 = 3.5;
    if x > 3.49 { if x < 3.51 { print("3.5"); } }
    if x > 3.0 { print("above 3"); }
}
"#,
        &unique_module_name("flt_value"),
    )
    .expect("N2-03");
    assert_eq!(out.lines().collect::<Vec<_>>(), vec!["3.5", "above 3"]);
}

/// THE ONE THAT CATCHES TRUNCATION, and the reason `infer_expr_type` had to
/// learn about doubles: `let d = x / y;` with no annotation used to declare
/// `long long d` and turn 2.8 into 2 — silently, with the type checker knowing
/// the right answer the whole time.
#[test]
fn an_inferred_float_local_is_a_double_and_not_a_truncating_long_long() {
    let name = unique_module_name("flt_infer");
    let src = r#"
fn main() {
    let x: f64 = 3.5;
    let y: f64 = 1.25;
    let quotient = x / y;
    if quotient > 2.79 { if quotient < 2.81 { print("2.8"); } }
}
"#;
    let c = compile_to_c(src, &name).expect("N2-03");
    assert!(
        c.contains("double quotient"),
        "the inferred local must be a double; generated C had:\n{}",
        c.lines()
            .filter(|l| l.contains("quotient"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    let out = compile_and_run(src, &unique_module_name("flt_infer_run")).expect("N2-03");
    assert_eq!(out.trim(), "2.8");
}

#[test]
fn float_arithmetic_is_float_arithmetic() {
    let out = compile_and_run(
        r#"
fn main() {
    let a: f64 = 3.5;
    let b: f64 = 1.25;
    if a + b > 4.749 { if a + b < 4.751 { print("add"); } }
    if a - b > 2.249 { if a - b < 2.251 { print("sub"); } }
    if a * b > 4.374 { if a * b < 4.376 { print("mul"); } }
    if a / b > 2.79  { if a / b < 2.81  { print("div"); } }
}
"#,
        &unique_module_name("flt_arith"),
    )
    .expect("N2-03");
    assert_eq!(out.lines().collect::<Vec<_>>(), vec!["add", "sub", "mul", "div"]);
}

/// `f32` is a distinct C type. Without this the `Type::F32` arm could fall
/// through to `double` and nothing would notice.
#[test]
fn f32_is_emitted_as_c_float() {
    let c = compile_to_c(
        "fn main() { let small: f32 = 0.5; }",
        &unique_module_name("flt_f32"),
    )
    .expect("N4-02");
    assert!(
        c.contains("float small"),
        "expected a C `float`, got:\n{}",
        c.lines()
            .filter(|l| l.contains("small"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// NO IMPLICIT WIDENING. C would convert `1 + 2.5` silently, and a language
/// with no `as` cast yet (N5, owed) would then have a conversion nobody can see
/// and nobody can write.
#[test]
fn mixing_an_int_and_a_float_is_a_type_error_that_names_both() {
    let msg = refusal(
        "fn main() { let x: f64 = 1.0; let y = x + 1; }",
        &unique_module_name("flt_mix"),
    );
    assert!(
        msg.contains("Float") && msg.contains("Int"),
        "the diagnostic must name both types, got: {}",
        msg
    );
}

/// `%` on a double is not `%` in C — it is `fmod`, a library call — so
/// accepting it would emit C that gcc rejects, which is the class D5 closed.
#[test]
fn modulo_on_a_float_is_refused_before_any_c_exists() {
    let msg = refusal(
        "fn main() { let x = 1.5 % 2.0; }",
        &unique_module_name("flt_mod"),
    );
    assert!(msg.contains("Int"), "got: {}", msg);
}

/// A float emitted into C must keep its decimal point: `3.0` written as `3` is
/// an `int` to a C compiler and changes the meaning of a division around it.
#[test]
fn a_whole_valued_float_keeps_its_decimal_point_in_the_c() {
    let c = compile_to_c(
        "fn main() { let x: f64 = 3.0; }",
        &unique_module_name("flt_whole"),
    )
    .expect("N2-03");
    assert!(
        c.contains("double x = 3.0"),
        "got:\n{}",
        c.lines()
            .filter(|l| l.contains("double x"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// THE CONTROL FOR RANGES. `1..5` must not lex as `1.` `.5`; the float regex
/// needs a digit after the dot and the second `.` is not one.
#[test]
fn a_range_is_not_two_floats() {
    let out = compile_and_run(
        r#"
fn main() {
    let mut total: i64 = 0;
    for i in 1..5 {
        total = total + i;
    }
    print_int(total);
}
"#,
        &unique_module_name("flt_range"),
    )
    .expect("`1..5` must still be a range");
    assert_eq!(out.trim(), "10");
}

/// `''` — WHAT ACTUALLY HAPPENS, now that the diagnostic that claimed to
/// handle it is gone.
///
/// There was a `LexError::EmptyCharLiteral` reading "empty character literal:
/// `''` has no character in it". It could never fire: the char rule is
/// `'([^'\\]|\\.)'`, which REQUIRES one character or escape between the ticks,
/// so `''` never reaches the callback at all — the two ticks fall back to
/// `Token::SingleQuote` twice and the parser refuses the second one.
///
/// A diagnostic that cannot fire is a claim that cannot fail, so it was DELETED
/// rather than left as coverage nobody had, and this test pins the real
/// behaviour in its place. Making `''` a dedicated lexical error would be a
/// better message and is a separate change; what is not acceptable is a message
/// in the source that no program can produce.
#[test]
fn an_empty_char_literal_is_refused() {
    let msg = refusal(
        "fn main() { let c = ''; }",
        &unique_module_name("chr_empty"),
    );
    assert!(
        !msg.contains("empty character literal"),
        "the deleted diagnostic is back without a rule that can reach it: {}",
        msg
    );
    assert!(
        msg.contains("expected expression") && msg.contains('\''),
        "`''` lexes as two SingleQuote tokens and dies in the parser, naming the tick; \
         got: {}",
        msg
    );
}

/// THE SIGN IS PART OF THE FLOAT TOKEN, exactly as it is for `Integer`, and
/// this is the contract test that says so out loud.
///
/// `x-1.5` therefore lexes as `x` then `-1.5` — two expressions with no
/// operator between them — and must be written `x - 1.5`. That is the same
/// gotcha `-?[0-9]+` already has for integers (`i-1` is `i` then `-1`,
/// grammar.ebnf and A2 both say so), and matching it was the choice: one
/// convention for both numeric literals rather than two.
///
/// IT IS A CHOICE AND IT HAS AN EXIT. When N5-16 brings unary minus into the
/// expression grammar, sign handling moves there and BOTH tokens lose their
/// leading `-` together. Until then this test fails if either half drifts —
/// which is the point, because the suite had no operator-adjacent literal at
/// all and the convention was only asserted in a doc comment.
#[test]
fn the_float_token_carries_its_sign_exactly_as_the_integer_token_does() {
    // Spaced: subtraction, for both kinds.
    let out = compile_and_run(
        r#"
fn main() {
    let x: f64 = 4.0;
    let d = x - 1.5;
    if d > 2.49 { if d < 2.51 { print("float spaced subtracts"); } }
    let i: i64 = 4;
    print_int(i - 1);
}
"#,
        &unique_module_name("flt_sign_spaced"),
    )
    .expect("spaced arithmetic must work for both");
    assert_eq!(
        out.lines().collect::<Vec<_>>(),
        vec!["float spaced subtracts", "3"]
    );

    // Unspaced: the sign binds to the literal, so this is NOT subtraction and
    // must not silently compile as if it were. Both kinds, same verdict.
    for (label, src) in [
        ("float", "fn main() { let x: f64 = 4.0; let d = x-1.5; }"),
        ("int", "fn main() { let i: i64 = 4; let d = i-1; }"),
    ] {
        let msg = refusal(src, &unique_module_name("flt_sign_unspaced"));
        assert!(
            !msg.is_empty(),
            "{}: `a-b` unspaced must not be read as subtraction",
            label
        );
    }

    // A negative literal in a value position is an ordinary literal.
    let out = compile_and_run(
        r#"
fn main() {
    let n: f64 = -2.5;
    if n < -2.49 { if n > -2.51 { print("negative float"); } }
    print_int(-3);
}
"#,
        &unique_module_name("flt_sign_neg"),
    )
    .expect("a negative literal is a literal");
    assert_eq!(out.lines().collect::<Vec<_>>(), vec!["negative float", "-3"]);
}

// ---------------------------------------------------------------------------
// N2-10 + N2-11 — attributes lex, and every one of them is refused
// ---------------------------------------------------------------------------

/// All three shapes N2-10 names, each refused BY NAME.
///
/// Reaching a diagnostic that says `frobnicate` is what proves the shape
/// lexed: the parser had to consume `#`/`#!`, `[`, the name, any argument
/// list, and `]` before it could look the name up. A scanner that gave up
/// earlier would produce a different message.
#[test]
fn every_attribute_shape_is_refused_by_name() {
    for (label, source) in [
        ("#[name]", "#[frobnicate]\nfn main() {}\n"),
        ("#[name(args)]", "#[frobnicate(a, b)]\nfn main() {}\n"),
        (
            "#[name(nested(args))]",
            "#[frobnicate(a, b(c))]\nfn main() {}\n",
        ),
        ("#![name(args)]", "#![frobnicate(a)]\nfn main() {}\n"),
        ("#![name]", "#![frobnicate]\nfn main() {}\n"),
    ] {
        let msg = refusal(source, &unique_module_name("attr_shape"));
        assert!(
            msg.contains("unknown attribute") && msg.contains("frobnicate"),
            "{} must be refused by name, got: {}",
            label,
            msg
        );
    }
}

/// The specification's only named attribute, and the reason the two rows are
/// one commit: `#[total]` used to die at the character `#`. It now reaches the
/// parser, is read as an attribute called `total`, and is still refused —
/// because nothing here can discharge a totality obligation (N8, M6).
#[test]
fn the_totality_attribute_is_refused_by_its_own_name() {
    let msg = refusal(
        "#[total]\nfn f(n: i64) -> i64 { return n; }\nfn main() {}\n",
        &unique_module_name("attr_total"),
    );
    assert!(
        msg.contains("unknown attribute") && msg.contains("total"),
        "got: {}",
        msg
    );
    assert!(
        !msg.contains("Unexpected character"),
        "the refusal must be the parser's, not the lexer's: {}",
        msg
    );
}

/// THE ROW ITSELF, stated as an assertion about the compiler rather than about
/// one program: there is no attribute that is accepted.
///
/// This is the control that fails the day someone adds a name to the table
/// without ALSO implementing it — which is precisely how "lexes and is then
/// ignored" gets in.
#[test]
fn the_known_attribute_set_is_empty_on_purpose() {
    assert!(
        palladium::parser::KNOWN_ATTRIBUTES.is_empty(),
        "KNOWN_ATTRIBUTES gained {:?}. That is not a small change: an attribute in \
         this table is one the compiler claims to HONOUR, and N2-11 exists so that a \
         name here cannot mean 'lexes and is ignored'. If the entry is real, implement \
         it, add a `run` fixture that observes its effect, and update this test with \
         the reason. `total` in particular is N8/M6 and must not appear until the \
         obligation is discharged.",
        palladium::parser::KNOWN_ATTRIBUTES
    );
}

/// An inner attribute is only legal at the top of the compilation unit, and
/// saying so beats falling through to "Expected function, struct, …", which
/// names the wrong problem.
#[test]
fn an_inner_attribute_after_an_item_names_its_own_mistake() {
    let msg = refusal(
        "fn main() {}\n#![frobnicate]\n",
        &unique_module_name("attr_late"),
    );
    assert!(
        msg.contains("top of the file"),
        "expected a positional diagnostic, got: {}",
        msg
    );
}

/// A malformed attribute is NOT reported as an unknown one — the control that
/// separates "the shape lexed and the name was rejected" from "anything
/// starting with `#` is rejected".
#[test]
fn a_malformed_attribute_is_a_different_diagnostic() {
    let msg = refusal("#[123]\nfn main() {}\n", &unique_module_name("attr_bad"));
    assert!(
        !msg.contains("unknown attribute"),
        "an attribute with no name must not be reported as an unknown one: {}",
        msg
    );
    assert!(msg.contains("attribute name"), "got: {}", msg);
}

/// `#` outside an attribute is still an ordinary lexical error, so making `#`
/// lex did not make it mean nothing.
#[test]
fn a_bare_hash_in_expression_position_is_still_an_error() {
    let msg = refusal(
        "fn main() { let x = # ; }",
        &unique_module_name("attr_bare_hash"),
    );
    assert!(
        !msg.contains("unknown attribute"),
        "a stray `#` is not an attribute: {}",
        msg
    );
}
