//! Stable diagnostic codes, end to end — GI-12's seeds and the parser family.
//!
//! WHAT IS BEING PROVED, AND WHY IT NEEDS THE REAL BINARY
//!
//! A code is worth having only if a manifest row can pin it INSTEAD OF a phrase
//! and lose nothing. That is two claims, and they fail in different ways:
//!
//!   1. the code REACHES the wire — attached at the construction site, carried
//!      on `CompileError`, preserved through `to_diagnostic()`, rendered by the
//!      single choke point as `error[PD####]: ` at column 0;
//!   2. the code is not ENOUGH by itself — both seeds have several witnesses,
//!      so the pin needs a message fragment to say WHICH, and that fragment has
//!      to select its own fixture and no sibling.
//!
//! Claim 1 is a property of the whole pipeline, so these tests drive the `pdc`
//! BINARY this run built rather than calling `to_diagnostic()` in process. A
//! unit test on the carrier would have stayed green through the entire class of
//! defect this unit exists to remove: a code that is attached and then dropped
//! one layer down. Measured before the choke point existed, `to_diagnostic()`
//! was silently discarding the missing-pattern particulars in exactly that way.
//!
//! Claim 2 is checked here the same way `check-fragments.py` checks it over the
//! whole map: a fragment must appear in its own fixture's primary payload and in
//! NO sibling's. A fragment that also matches a sibling is an accepting pin
//! wearing a discriminating fragment's clothes.
//!
//! THE TWO SEEDS, AND WHY THESE TWO
//!
//!   PD0003, the cast relation, 4 witnesses / ONE construction site. The hardest
//!   shape available: one predicate over a pair, whose middle arm is a symmetric
//!   or-pattern, so direction is never a branch. Four codes would be four names
//!   for one predicate; one code plus the formatted `found` clause is the whole
//!   D1 argument in miniature.
//!
//!   PD0002, the const/static initialiser, 6 witnesses / ONE `refuse` closure.
//!   The merge the su0 map review argued about longest: six faults detected at
//!   six places inside one closure that states one rule. If the rule is one, the
//!   fault is a parameter, and the parameter has to be carried by the payload —
//!   which is exactly what this file measures.
//!
//! THE TEN CONDITIONS su2a ADDS, and what each one is here to catch
//!
//! su2a wires the parser/macro/lexer family. Four of its conditions have several
//! witnesses (PD0008, PD0020, PD0049, PD0067) and are checked by exactly the
//! measurement above: one code, and a fragment that selects its own fixture and
//! no sibling. Six have one witness each, and their fragments are NOT the map's
//! — the map asks for no discriminator where there is no sibling to discriminate
//! from. They are here for a different question: that the code landed on the
//! refusal the registry names, and not on a neighbouring one that happens to be
//! raised from the same function.
//!
//! Two shapes in this family are worth naming, because they are the two ways a
//! per-condition code goes wrong and they point in OPPOSITE directions:
//!
//!   ONE FORMATTER, TWO RULES. `token_to_ast_token`'s `refuse` closure raises
//!   both PD0008 (a literal has no kind in the macro token stream) and PD0074
//!   (a `Punct` is one `char`, so `==` cannot be written down). Attaching a code
//!   inside that closure — the obvious thing — would say the two were one rule
//!   because they share a sentence template.
//!
//!   TWO SITES, ONE RULE. PD0049 is refused in item position by the parser and
//!   in invocation position by the expander; PD0067 is refused over a macro BODY
//!   and over a macro ARGUMENT. Giving either pair two codes would mint a number
//!   per construction site, which is the thing D1 says a code is not — and it is
//!   what the su0 review already undid, retiring PD0051 and PD0073.
//!
//! THE FOURTEEN CONDITIONS su2b ADDS, and why their sites had to be MEASURED
//!
//! su2b wires the grammar positions whose refusal renders as
//! `Expected ..., but found ...`. Six of the fourteen could not be attributed by
//! reading the source: their sentence is split across Rust line continuations,
//! or it is assembled by `consume()` from a message that a hundred other callers
//! pass too — `Expected ')'` is written at eleven call sites. So the sites were
//! named by a throwaway marker build: a unique marker per message literal, one
//! release build, every witness compiled, the markers read off the headers, and
//! every marker reverted before a single code was attached. The readings are in
//! the unit's `su2b-attribution.tsv`; what is asserted BELOW is the same claim
//! re-derived from the shipped binary, which is the form that survives.
//!
//! Thirteen of the fourteen have one witness each, and the shapes worth naming:
//!
//!   ONE SENTENCE, TWO RULES. `parse_let`'s name position and the `for` loop's
//!   both print `Expected variable name, but found ...`. PD0041 is the let rule
//!   only; the `for` position stays uncoded, and a test below writes the `for`
//!   program to prove the code went to a site and not to a wording.
//!
//!   ONE RULE, SEVERAL POSITIONS. A parameter list is closed by `)` in two
//!   places (function, method) and a generic parameter list by `>` in five
//!   (function, trait, trait method, impl, type alias). All of them carry the
//!   one code, as PD0049 does — and only the function position has a corpus
//!   witness, so the others are proven here by programs written for the purpose.
//!
//!   TWO NEAR-IDENTICAL SENTENCES, TWO CODES. The tuple arity rule is stated
//!   over values (PD0038) and over patterns (PD0037); the range-pattern rule is
//!   refused at the low end (PD0035) and at the high end (PD0034). The locked
//!   map keeps each pair apart, so a test below refuses their collapse.
//!
//! PD0013 is the fourteenth and the only one with two witnesses: a `package.pd`
//! manifest and an `extern` block, whose payloads are CHARACTER-IDENTICAL. That
//! pair takes no `msg~` — there is nothing to tell apart, and the test asserts
//! the identity rather than inventing a discriminator.
//!
//! WHAT IS NOT CLAIMED. Nothing here says the manifest pins codes: it does not,
//! and will not until the cutover. These are the vertical proof that it CAN.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// One run of `pdc compile`, with the streams kept APART.
///
/// Separate captures, not a merged one, for the reason the shared shell parser
/// keeps them apart too: a fixture's own text can reach stdout, and a merged
/// stream cannot tell the two producers apart.
struct Refusal {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

impl Refusal {
    /// Every COLUMN-0 coded primary header in the capture, as (code, payload).
    ///
    /// Column 0 is the whole defence. Everything else the compiler writes is
    /// indented — the location is `  --> `, the echoed source is `N | `, notes
    /// are `  = note:` — so text the FIXTURE chose can never be at column 0.
    fn coded_headers(&self) -> Vec<(String, String)> {
        strip_ansi(&self.stderr)
            .lines()
            .filter_map(|l| {
                let rest = l.strip_prefix("error[")?;
                let (code, payload) = rest.split_once("]: ")?;
                let is_code = code.len() == 6
                    && code.starts_with("PD")
                    && code[2..].chars().all(|c| c.is_ascii_digit());
                is_code.then(|| (code.to_string(), payload.to_string()))
            })
            .collect()
    }

    /// The one coded primary header, or a panic naming what was there instead.
    /// Cardinality-1 is asserted here rather than assumed by taking `[0]`.
    fn sole_coded_header(&self, what: &str) -> (String, String) {
        let hs = self.coded_headers();
        assert_eq!(
            hs.len(),
            1,
            "{}: expected exactly ONE coded primary header, found {}. \
             A refusal that prints two of them cannot be attributed to either.\n\
             stderr was:\n{}",
            what,
            hs.len(),
            self.stderr
        );
        hs.into_iter().next().unwrap()
    }
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Compile a corpus fixture with the binary this run built.
///
/// The working directory is a fresh temp dir, not the repo: `-o` resolves
/// `build_output/` relative to the CWD, and two tests compiling into the same
/// name would race. Nothing is written into the tree under test.
fn compile_fixture(name: &str) -> (Refusal, TempDir) {
    compile_path(&format!("tests/reject/{}", name))
}

/// The same, for a corpus fixture that does not live under `tests/reject`.
///
/// PD0013's second witness is `tests/projects/hello_pdm/package.pd`, a package
/// manifest whose refusal is what makes it a `skip` row rather than a program.
fn compile_path(rel: &str) -> (Refusal, TempDir) {
    let dir = TempDir::new().expect("tempdir");
    let out = Command::new(env!("CARGO_BIN_EXE_pdc"))
        .current_dir(dir.path())
        .arg("compile")
        .arg(repo_root().join(rel))
        .arg("-o")
        .arg("probe")
        .output()
        .expect("run pdc");
    (
        Refusal {
            code: out.status.code(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        },
        dir,
    )
}

/// Compile a program written HERE, and keep the refusal.
///
/// Needed because two of su2b's claims have no corpus witness by construction:
/// a rule stated in several grammar positions is witnessed at one of them, and
/// the position that must stay UNCODED has no fixture at all — the corpus only
/// holds programs someone already decided to pin.
fn compile_source(source: &str) -> (Refusal, TempDir) {
    let dir = TempDir::new().expect("tempdir");
    let src = dir.path().join("written.pd");
    fs::write(&src, source).expect("write source");
    let out = Command::new(env!("CARGO_BIN_EXE_pdc"))
        .current_dir(dir.path())
        .arg("compile")
        .arg(&src)
        .arg("-o")
        .arg("written")
        .output()
        .expect("run pdc");
    (
        Refusal {
            code: out.status.code(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        },
        dir,
    )
}

/// Compile, LINK and RUN a program written for this test, returning its stdout.
///
/// The acceptance side. Without it, every assertion in this file could be
/// satisfied by a compiler that refuses everything with the right code.
fn compile_link_run(source: &str) -> String {
    let dir = TempDir::new().expect("tempdir");
    let src = dir.path().join("accepted.pd");
    fs::write(&src, source).expect("write source");

    let out = Command::new(env!("CARGO_BIN_EXE_pdc"))
        .current_dir(dir.path())
        .arg("compile")
        .arg(&src)
        .arg("-o")
        .arg("accepted")
        .output()
        .expect("run pdc");
    assert!(
        out.status.success(),
        "the acceptance control did not compile:\n{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let binary: PathBuf = dir.path().join("build_output").join("accepted");
    assert!(
        binary.exists(),
        "pdc reported success and produced no executable at {}",
        binary.display()
    );
    let run = Command::new(&binary).output().expect("run the program");
    assert!(
        run.status.success(),
        "the accepted program exited {:?}",
        run.status.code()
    );
    String::from_utf8_lossy(&run.stdout).into_owned()
}

/// A code and the fixtures that witness it, with the fragment that is supposed
/// to select each one. The fragments are the locked semantic map's, verbatim.
struct Family {
    code: &'static str,
    rows: &'static [(&'static str, &'static str)],
}

/// PD0003. Four directions, ONE construction site, direction carried by the
/// formatted `found` clause.
const CAST: Family = Family {
    code: "PD0003",
    rows: &[
        ("bool_does_not_cast_to_char.pd", "a cast from Bool to Char"),
        (
            "float_does_not_cast_to_char.pd",
            "a cast from Float to Char",
        ),
        ("char_does_not_cast_to_bool.pd", "a cast from Char to Bool"),
        (
            "char_does_not_cast_to_float.pd",
            "a cast from Char to Float",
        ),
    ],
};

/// PD0002. Six faults, ONE `refuse` closure, fault carried after the colon.
///
/// The fragments are FULL payloads and not the short fault clause, and that is
/// measured rather than stylistic: `division by zero` is a substring of
/// `the remainder of a division by zero`, so the short form would let one
/// fixture's pin accept its sibling. `selects_its_own_row_and_no_sibling`
/// below is what refuses the short form.
const CONST_INIT: Family = Family {
    code: "PD0002",
    rows: &[
        (
            "const_divide_by_zero.pd",
            "the initialiser of `X` has no value: division by zero",
        ),
        (
            "const_overflows_i64.pd",
            "the initialiser of `X` has no value: 9223372036854775807 + 1 overflows i64",
        ),
        (
            "const_remainder_by_zero.pd",
            "the initialiser of `X` has no value: the remainder of a division by zero",
        ),
        (
            "const_shift_out_of_range.pd",
            "the initialiser of `X` has no value: a shift by 64 — the amount has to be between 0 and 63",
        ),
        (
            "const_shl_negative_left.pd",
            "the initialiser of `X` has no value: shifting the negative value -1 left, which C leaves u",
        ),
        (
            "const_shl_value_overflow.pd",
            "the initialiser of `X` has no value: 1 << 63 overflows i64",
        ),
    ],
};

/// Compile every witness of a family once, and return their primary payloads.
fn payloads(f: &Family) -> Vec<(&'static str, String)> {
    f.rows
        .iter()
        .map(|(fixture, _)| {
            let (r, _dir) = compile_fixture(fixture);
            assert_eq!(
                r.code,
                Some(1),
                "{} is a reject fixture and must exit 1; it exited {:?}\n{}",
                fixture,
                r.code,
                r.stderr
            );
            let (code, payload) = r.sole_coded_header(fixture);
            assert_eq!(
                code, f.code,
                "{} carries {} — the site was wired to the wrong condition",
                fixture, code
            );
            // The header must be at column 0 of STDERR and nowhere near stdout:
            // the banner and the phase lines are stdout, and a consumer that
            // read the merged stream would be reading the fixture's producer and
            // the compiler's through one hole.
            assert!(
                !strip_ansi(&r.stdout)
                    .lines()
                    .any(|l| l.starts_with("error[")),
                "{}: a coded header appeared on STDOUT",
                fixture
            );
            (*fixture, payload)
        })
        .collect()
}

fn check_family(f: &Family) {
    let measured = payloads(f);

    for (fixture, want) in f.rows {
        let (_, got) = measured
            .iter()
            .find(|(n, _)| n == fixture)
            .expect("every fixture was compiled");
        assert!(
            got.contains(want),
            "{}: the pin fragment is not in the primary payload.\n  want fragment: {}\n  got payload:   {}",
            fixture,
            want,
            got
        );
    }

    // SELECTS ITS OWN ROW AND NO SIBLING. This is the property `code=` alone
    // cannot have when a condition has several witnesses, and the reason the
    // compound pin exists at all.
    for (fixture, want) in f.rows {
        let hits: Vec<&str> = measured
            .iter()
            .filter(|(_, payload)| payload.contains(want))
            .map(|(n, _)| *n)
            .collect();
        assert_eq!(
            hits,
            vec![*fixture],
            "{}: its fragment {:?} also selects {:?} — an accepting pin",
            fixture,
            want,
            hits.iter().filter(|n| *n != fixture).collect::<Vec<_>>()
        );
    }
}

/// PD0008. Three witnesses, ONE `refuse` closure that it SHARES with PD0074,
/// so the code is a parameter of the closure rather than a line inside it.
const MACRO_LITERAL_KIND: Family = Family {
    code: "PD0008",
    rows: &[
        ("macro_body_bool_literal.pd", "`true`"),
        ("macro_body_float_literal.pd", "`3.5`"),
        (
            "macro_body_string_literal.pd",
            "a string literal may not appear in a macro body or in a macro argument: the token \
             stream s",
        ),
    ],
};

/// PD0020. Two witnesses, ONE `refuse` closure — and here the closure really is
/// the rule, so the code is attached inside it. The form that was written is the
/// parameter.
const TOP_LEVEL_INITIALISER: Family = Family {
    code: "PD0020",
    rows: &[
        ("const_initializer_calls.pd", "`main`"),
        ("const_reads_another_item.pd", "`A`"),
    ],
};

/// PD0049. ONE rule said in TWO POSITIONS — the parser's item position and the
/// expander's invocation position. The lead sentence differs and the rule clause
/// does not, which is why the fragments are the leads.
const MACRO_RULES: Family = Family {
    code: "PD0049",
    rows: &[
        ("macro_rules.pd", "is not a declaration in this language"),
        (
            "macro_rules_invocation.pd",
            "is not this language's macro syntax",
        ),
    ],
};

/// PD0067. ONE rule seen from two sides: an invocation produced by an expansion,
/// written in a macro BODY or passed as a macro ARGUMENT.
const SINGLE_PASS_EXPANSION: Family = Family {
    code: "PD0067",
    rows: &[
        (
            "macro_invokes_macro.pd",
            "in its body, and expansion is a single pass",
        ),
        (
            "macro_argument_invokes_macro.pd",
            "as an argument, and expansion is a single pass",
        ),
    ],
};

/// The six one-witness conditions of su2a, as (fixture, code, fragment).
///
/// THE FRAGMENT IS NOT A PIN HERE and is not the locked map's: a code with one
/// witness needs no discriminator, and the map records none. It answers the
/// question a bare `assert_eq!(code, "PD00nn")` cannot — WHICH refusal is
/// wearing the code. Three of these six live in functions that raise a
/// neighbouring refusal too (`returns_on_every_path`'s two arms,
/// `register_macro`'s three, the `LexError` match's four), so a code attached
/// one arm over would still be the right number on the wrong rule.
const SU2A_SINGLE_WITNESS: &[(&str, &str, &str)] = &[
    ("missing_return.pd", "PD0066", "may return without a value"),
    (
        "macro_unknown_substitution.pd",
        "PD0068",
        "which is not one of its parameters",
    ),
    (
        "macro_bare_parameter.pd",
        "PD0069",
        "is not a substitution: write `$x`",
    ),
    (
        "macro_body_two_char_operator.pd",
        "PD0074",
        "a macro body stores one character per punctuation token",
    ),
    ("unknown_escape.pd", "PD0077", "unknown escape sequence"),
    (
        "unterminated_block_comment.pd",
        "PD0078",
        "`/*` is never closed",
    ),
];

/// The thirteen one-witness conditions of su2b, as (fixture, code, fragment).
///
/// Same contract as su2a's table: the fragment is not a manifest pin — a code
/// with one witness needs no discriminator and the locked map records none — it
/// answers WHICH refusal is wearing the code. That question is sharper here than
/// anywhere else in this file, because seven of these thirteen are raised by
/// `consume()`, whose message is passed IN by the caller: the same sentence is
/// written at up to eleven call sites, so a code attached one call over would
/// print an identical header.
const SU2B_SINGLE_WITNESS: &[(&str, &str, &str)] = &[
    (
        "macro_invocation_bracket.pd",
        "PD0029",
        "Expected '(' after macro name!",
    ),
    ("ref_parameter.pd", "PD0030", "Expected ')' (Expected ')')"),
    (
        "let_needs_an_initializer.pd",
        "PD0031",
        "Expected '=' after variable name",
    ),
    (
        "generic_bound.pd",
        "PD0032",
        "Expected '>' after generic parameters",
    ),
    (
        "tuple_index_chained.pd",
        "PD0033",
        "a CHAINED tuple index has to be parenthesised",
    ),
    (
        "range_pattern_open_ended.pd",
        "PD0034",
        "a literal for the high end of this range pattern",
    ),
    (
        "range_pattern_open_low.pd",
        "PD0035",
        "a range pattern to have both endpoints",
    ),
    (
        "tuple_index_leading_zero.pd",
        "PD0036",
        "a tuple index written without leading zeros",
    ),
    (
        "tuple_pattern_one_element.pd",
        "PD0037",
        "a tuple pattern to have at least two elements",
    ),
    (
        "tuple_one_element.pd",
        "PD0038",
        "a tuple to have at least two elements",
    ),
    (
        "closure_literal.pd",
        "PD0039",
        "Expected expression, but found '|'",
    ),
    (
        "brace_pattern_needs_a_variant_path.pd",
        "PD0040",
        "Expected pattern, but found '{'",
    ),
    (
        "let_does_not_destructure.pd",
        "PD0041",
        "Expected variable name, but found '('",
    ),
];

#[test]
fn the_cast_relation_is_one_code_told_apart_by_its_found_clause() {
    check_family(&CAST);
}

#[test]
fn the_const_initialiser_rule_is_one_code_told_apart_by_its_fault() {
    check_family(&CONST_INIT);
}

#[test]
fn the_macro_literal_kinds_are_one_code_told_apart_by_the_kind_they_name() {
    check_family(&MACRO_LITERAL_KIND);
}

#[test]
fn the_top_level_initialiser_rule_is_one_code_told_apart_by_the_form_it_saw() {
    check_family(&TOP_LEVEL_INITIALISER);
}

#[test]
fn macro_rules_is_one_code_in_both_the_item_and_the_invocation_position() {
    check_family(&MACRO_RULES);
}

#[test]
fn single_pass_expansion_is_one_code_in_both_the_body_and_the_argument_position() {
    check_family(&SINGLE_PASS_EXPANSION);
}

/// The six one-witness conditions, each on its own refusal.
///
/// One test rather than six because the assertion is identical and the failure
/// message names the fixture; splitting it would buy parallelism over six
/// compiles and cost the reader the list.
#[test]
fn each_one_witness_condition_of_su2a_is_on_the_refusal_the_registry_names() {
    for (fixture, want_code, fragment) in SU2A_SINGLE_WITNESS {
        let (r, _dir) = compile_fixture(fixture);
        assert_eq!(
            r.code,
            Some(1),
            "{} is a reject fixture and must exit 1; it exited {:?}\n{}",
            fixture,
            r.code,
            r.stderr
        );
        let (code, payload) = r.sole_coded_header(fixture);
        assert_eq!(
            code, *want_code,
            "{} carries {} — the site was wired to the wrong condition",
            fixture, code
        );
        assert!(
            payload.contains(fragment),
            "{} carries {} but not on the refusal that condition names.\n  want fragment: {}\n  got payload:   {}",
            fixture,
            code,
            fragment,
            payload
        );
    }
}

/// ONE FORMATTER IS NOT ONE RULE.
///
/// `token_to_ast_token` has a single `refuse` closure and two conditions run
/// through it, so the cheap wiring — `with_code` inside the closure — is a
/// mutant this file has to exclude by measurement rather than by comment. Under
/// it both fixtures below would carry the same code and every other assertion in
/// this file would still pass, because each family only ever compares a code
/// against its own siblings.
///
/// The third arm of that closure (`other`, the tokens the macro stream carries
/// no representation for at all) is left UNCODED on purpose: it is a third rule
/// with no corpus witness, and D1's honest state for an unjudged site is no
/// code.
#[test]
fn two_conditions_sharing_one_refuse_closure_do_not_collapse_to_one_code() {
    let (literal, _d1) = compile_fixture("macro_body_string_literal.pd");
    let (operator, _d2) = compile_fixture("macro_body_two_char_operator.pd");

    let (literal_code, _) = literal.sole_coded_header("macro_body_string_literal.pd");
    let (operator_code, _) = operator.sole_coded_header("macro_body_two_char_operator.pd");

    assert_eq!(literal_code, "PD0008");
    assert_eq!(operator_code, "PD0074");
    assert_ne!(
        literal_code, operator_code,
        "the lost-kind rule and the one-`char`-`Punct` rule were given one code — \
         the code was attached inside the `refuse` closure they share"
    );
}

/// The acceptance side of both seeds, run to a value.
///
/// A legal cast and a legal const initialiser must still compile, link and
/// produce the right answer. Without this, a compiler that refused every cast
/// with `PD0003` and every const with `PD0002` would pass everything above.
#[test]
fn the_legal_neighbours_of_both_seeds_still_compile_link_and_run() {
    let out = compile_link_run(
        r#"const LIMIT: i64 = 1 << 62;
fn main() {
    let c: char = 'A';
    print_int(c as i64);
    print_int(LIMIT / (1 << 60));
}
"#,
    );
    assert_eq!(
        out, "65\n4\n",
        "the legal cast and the legal const initialiser did not produce their values"
    );
}

/// D1's honest NO_CODE state: a site that has not been wired says so.
///
/// There is no family fallback and no sentinel code. The alternative — every
/// refusal gets SOME code — is the shape the LSP bridge already has
/// (`_ => "E9999"`), and it makes "this refusal is attributable" unfalsifiable.
///
/// THE CONTROL MOVED, AND THAT IS THE POINT. It used to be `ref_parameter.pd`,
/// which su2b coded as PD0030. A control has to be a refusal NOTHING has judged
/// yet, so it is replaced rather than kept: `at_binding_shadows_item.pd` is a
/// type-checker refusal owned by a later slice. The day that slice lands, this
/// control moves again — the moving is what keeps it a control.
#[test]
fn an_unwired_refusal_carries_no_code_rather_than_a_fallback() {
    let (r, _dir) = compile_fixture("at_binding_shadows_item.pd");
    assert_eq!(r.code, Some(1));
    assert!(
        r.coded_headers().is_empty(),
        "an unwired site produced a code: {:?}",
        r.coded_headers()
    );
    assert!(
        strip_ansi(&r.stderr).starts_with("error: "),
        "an uncoded refusal must still print a bare primary header:\n{}",
        r.stderr
    );
}

/// CARDINALITY-1, over both seeds and the uncoded control.
///
/// The state this asserts against was the corpus's REALITY until this unit:
/// every reject fixture printed two primary headers, in different wording, from
/// two printers, and the manifest pinned whichever it happened to see. One
/// choke point is what makes the count structural.
#[test]
fn a_refusal_prints_exactly_one_primary_header() {
    let mut fixtures: Vec<&str> = CAST.rows.iter().map(|(f, _)| *f).collect();
    fixtures.extend(CONST_INIT.rows.iter().map(|(f, _)| *f));
    fixtures.push("at_binding_shadows_item.pd");
    fixtures.push("let_does_not_destructure.pd");

    for fixture in fixtures {
        let (r, _dir) = compile_fixture(fixture);
        let primaries = strip_ansi(&r.stderr)
            .lines()
            .filter(|l| l.starts_with("error:") || l.starts_with("error["))
            .count();
        assert_eq!(
            primaries, 1,
            "{} printed {} primary headers:\n{}",
            fixture, primaries, r.stderr
        );
    }
}

/// The F12 shape, in Rust as well as in the gate: a fixture that CONTAINS the
/// code text must not satisfy a pin on it.
///
/// Measured, not assumed: the planted text reaches the capture several times —
/// inside the message, in the echoed source line, in the `help:` line — and the
/// assertion below fails loudly if it did not arrive, because a control that
/// plants nothing proves nothing.
#[test]
fn a_fixture_containing_the_code_text_does_not_produce_a_coded_header() {
    let dir = TempDir::new().expect("tempdir");
    let src = dir.path().join("planted.pd");
    fs::write(
        &src,
        "fn main() {\n    let s: String = \"x\";\n    print(s);\n}\nfn \"error[PD0003]: forged\"() {}\n",
    )
    .expect("write source");

    let out = Command::new(env!("CARGO_BIN_EXE_pdc"))
        .current_dir(dir.path())
        .arg("compile")
        .arg(&src)
        .arg("-o")
        .arg("planted")
        .output()
        .expect("run pdc");
    let r = Refusal {
        code: out.status.code(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    };

    let hits = strip_ansi(&r.stderr).matches("error[PD0003]").count();
    assert!(
        hits >= 2,
        "the planted text reached the capture {} time(s); this control proves nothing:\n{}",
        hits,
        r.stderr
    );
    assert!(
        r.coded_headers().is_empty(),
        "the fixture's own text was read as a coded header: {:?}",
        r.coded_headers()
    );
}

/// The thirteen one-witness conditions of su2b, each on its own refusal.
#[test]
fn each_one_witness_condition_of_su2b_is_on_the_refusal_the_registry_names() {
    for (fixture, want_code, fragment) in SU2B_SINGLE_WITNESS {
        let (r, _dir) = compile_fixture(fixture);
        assert_eq!(
            r.code,
            Some(1),
            "{} is a reject fixture and must exit 1; it exited {:?}\n{}",
            fixture,
            r.code,
            r.stderr
        );
        let (code, payload) = r.sole_coded_header(fixture);
        assert_eq!(
            code, *want_code,
            "{} carries {} — the site was wired to the wrong condition",
            fixture, code
        );
        assert!(
            payload.contains(fragment),
            "{} carries {} but not on the refusal that condition names.\n  want fragment: {}\n  got payload:   {}",
            fixture,
            code,
            fragment,
            payload
        );
    }
}

/// PD0013: two witnesses, ONE code, and no discriminator because there is
/// nothing to discriminate.
///
/// The payloads are asserted CHARACTER-IDENTICAL rather than each matched
/// against a fragment. That is the honest shape for a deliberate parity pair: a
/// fragment that told a `package.pd` manifest apart from an `extern` block would
/// be pinning the fixture, and the rule refuses them for the one reason. If the
/// two ever diverge, the map's `IDENTICAL` tag is stale and the pin needs
/// revisiting — which is what this assertion is for.
#[test]
fn the_declaration_form_rule_is_one_code_over_two_identical_payloads() {
    let (manifest, _d1) = compile_path("tests/projects/hello_pdm/package.pd");
    let (extern_block, _d2) = compile_fixture("extern_block.pd");

    let (manifest_code, manifest_payload) = manifest.sole_coded_header("package.pd");
    let (extern_code, extern_payload) = extern_block.sole_coded_header("extern_block.pd");

    assert_eq!(manifest_code, "PD0013");
    assert_eq!(extern_code, "PD0013");
    assert_eq!(
        manifest_payload, extern_payload,
        "the parity pair's payloads have diverged; the map records them as IDENTICAL"
    );
    assert!(
        manifest_payload.contains("Expected function, struct, enum, trait, type, impl, or macro"),
        "PD0013 landed on a different refusal: {}",
        manifest_payload
    );
}

/// TWO NEAR-IDENTICAL SENTENCES ARE NOT ONE RULE.
///
/// The tuple arity rule is stated over values and over patterns; the
/// range-pattern rule is refused at the high end and at the low end. Each pair
/// prints almost the same sentence from adjacent parser functions, which is the
/// shape that invites one code — and the locked map allocated four. Under the
/// collapse every other assertion in this file still passes, because each single
/// witness only ever compares its code against the registry, never against its
/// neighbour's.
#[test]
fn the_arity_and_range_pairs_keep_the_codes_the_map_allocated_them() {
    let pairs = [
        ("tuple_one_element.pd", "PD0038"),
        ("tuple_pattern_one_element.pd", "PD0037"),
        ("range_pattern_open_ended.pd", "PD0034"),
        ("range_pattern_open_low.pd", "PD0035"),
    ];
    let mut seen: Vec<String> = Vec::new();
    for (fixture, want) in pairs {
        let (r, _dir) = compile_fixture(fixture);
        let (code, _) = r.sole_coded_header(fixture);
        assert_eq!(code, want, "{} carries {}", fixture, code);
        assert!(
            !seen.contains(&code),
            "{} reused {}, which a sibling rule already carries",
            fixture,
            code
        );
        seen.push(code);
    }
}

/// ONE RULE STATED IN SEVERAL POSITIONS CARRIES THE CODE AT ALL OF THEM.
///
/// The corpus witnesses the parameter-list rule at a free function and the
/// generic-parameter rule at a function, so the other positions are asserted
/// against programs written here. Without this, "all five positions carry the
/// code" would be a claim in a comment: the registry row would say it, the
/// witness compile would not test it, and a later refactor could quietly leave
/// four of them uncoded.
#[test]
fn a_rule_written_in_several_grammar_positions_carries_one_code_at_each() {
    let (method, _d1) = compile_source(
        "struct S {\n    x: i64,\n}\n\nimpl S {\n    fn f(y: ref String) {}\n}\n\nfn main() {}\n",
    );
    let (code, payload) = method.sole_coded_header("a method's parameter list");
    assert_eq!(
        code, "PD0030",
        "the METHOD position of the parameter-list rule carries {}",
        code
    );
    assert!(payload.contains("Expected ')'"), "payload: {}", payload);

    let (trait_generics, _d2) =
        compile_source("trait T<X: Clone> {\n    fn f(self);\n}\n\nfn main() {}\n");
    let (code, payload) = trait_generics.sole_coded_header("a trait's generic parameters");
    assert_eq!(
        code, "PD0032",
        "the TRAIT position of the generic-parameter rule carries {}",
        code
    );
    assert!(
        payload.contains("Expected '>' after generic parameters"),
        "payload: {}",
        payload
    );
}

/// THE SAME SENTENCE FROM A DIFFERENT RULE STAYS UNCODED.
///
/// `parse_let`'s name position and the `for` loop's print the identical string,
/// `Expected variable name, but found '('`. PD0041 is the `let` rule — there are
/// no `let` patterns — and the `for` position is a rule nothing has judged. If a
/// future edit attached the code by matching the message instead of by knowing
/// the site, this program is what goes red: a code that a phrase can earn is the
/// phrase pin GI-12 replaces, wearing four digits.
#[test]
fn the_for_loops_twin_sentence_is_a_different_rule_and_stays_uncoded() {
    let (r, _dir) = compile_source(
        "fn main() {\n    for (a, b) in 0..2 {\n        print_int(a);\n    }\n}\n",
    );
    assert_eq!(r.code, Some(1));
    let plain = strip_ansi(&r.stderr);
    assert!(
        plain.contains("Expected variable name, but found '('"),
        "the control did not reach the twin sentence:\n{}",
        r.stderr
    );
    assert!(
        r.coded_headers().is_empty(),
        "the `for` position was given a code by its wording: {:?}",
        r.coded_headers()
    );
}
