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
//! THE THIRTY-EIGHT CONDITIONS su3 ADDS, and what is different about them
//!
//! su3 wires the TYPE-CHECKER family: thirty-eight conditions at fifty-two
//! construction sites under `src/typeck/`. Two things change at this scale.
//!
//! POSITIONS OUTNUMBER WITNESSES. Fourteen of these rules are stated at more
//! than one position — PD0001 at four, PD0005 at four, PD0009 at three, PD0046
//! at three — and the corpus witnesses only some of them. The positions it does
//! not witness are compiled here from programs written for the purpose, because
//! a registry row that says "four positions" and a test that proves one is a
//! claim resting on a comment.
//!
//! THE SHARED HELPER IS THE su2a MUTANT AGAIN, one layer up.
//! `TypeErrorHelper::type_mismatch` is called by the annotated-`let` arm and by
//! the ASSIGNMENT arm, which is a different rule with no code; `missing_main`
//! is called by one predicate. Codes are attached at the CALL and never inside
//! the helper, and the assignment program below is the control that would go RED
//! under the cheap wiring.
//!
//! ONE DISPOSITION IS RECORDED RATHER THAN SMOOTHED OVER. The locked map held
//! two numbers for calling a `&mut self` method through a receiver that is not
//! one; both name a single `Err` whose `detail` the caller chooses, so su3 mints
//! PD0021 and drops the second unminted rather than tombstoning it. The registry
//! row says what that costs.
//!
//! WHAT IS NOT CLAIMED. Nothing here says the manifest pins codes: it does not,
//! and will not until the cutover. These are the vertical proof that it CAN.

use std::collections::HashSet;
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

/// Compile a refusal witness named either way the corpus names one.
///
/// A bare `name.pd` is a `tests/reject/` row, which is where most of them live;
/// a name with a `/` is a repo-relative path, which is what the rest need — the
/// su3 family's witnesses include two `tests/xfail/` rows, a `tests/misc/` row
/// and a package module under `tests/projects/`. Routing on the shape rather
/// than duplicating the tables keeps the fixture column readable.
fn compile_witness(name: &str) -> (Refusal, TempDir) {
    if name.contains('/') {
        compile_path(name)
    } else {
        compile_fixture(name)
    }
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
            let (r, _dir) = compile_witness(fixture);
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
/// THE CONTROL MOVED TWICE, AND THAT IS THE POINT. It was `ref_parameter.pd`,
/// which su2b coded as PD0030, then `at_binding_shadows_item.pd`, which su3
/// coded as PD0004. A control has to be a refusal NOTHING has judged yet, so it
/// is replaced rather than kept: `mut_borrow_of_immutable.pd` is a BORROW-CHECKER
/// refusal — the locked map's PD0012, which su3's type-checker family does not
/// reach — owned by a later slice. The day that slice lands, this control moves
/// again; the moving is what keeps it a control.
#[test]
fn an_unwired_refusal_carries_no_code_rather_than_a_fallback() {
    let (r, _dir) = compile_fixture("mut_borrow_of_immutable.pd");
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
    fixtures.push("mut_borrow_of_immutable.pd");
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

// ---------------------------------------------------------------------------
// su3 — THE TYPE-CHECKER FAMILY
// ---------------------------------------------------------------------------

/// The fourteen su3 conditions with SEVERAL witnesses and a particular that
/// tells them apart.
///
/// Same contract as `CAST` and `CONST_INIT`: one code, and a fragment that
/// appears in its own fixture's payload and in no sibling's. Two entries are
/// worth reading before the table.
///
/// PD0007's `static` pair is told apart only by the ITEM NAME, because the
/// noun the message prints for `static` and for `static mut` is the same word.
/// That is a corpus fact rather than a choice: the rule does not distinguish
/// them, so nothing in the sentence can.
///
/// PD0022's second and third witnesses are `tests/xfail/` rows, not `reject`
/// rows. The annotated-`let` site refuses them too — an alias that is not
/// expanded before the comparison is this rule refusing a program it should
/// accept — and that is what makes them xfail. They are listed because they are
/// what the compiler DOES, and a table that hid them would be describing a
/// compiler that does not exist.
const SU3_FAMILIES: &[Family] = &[
    // PD0004. Four binder kinds, ONE `Err` in `refuse_global_shadow`; the kind
    // is passed in by the caller, so it is the parameter and the fragment.
    Family {
        code: "PD0004",
        rows: &[
            ("at_binding_shadows_item.pd", "the `@` binding `LIMIT`"),
            ("const_local_shadows_item.pd", "the local `LIMIT`"),
            ("for_binder_shadows_item.pd", "the loop variable `LIMIT`"),
            (
                "pattern_binder_shadows_item.pd",
                "the pattern binding `LIMIT`",
            ),
        ],
    },
    // PD0005. ONE NAMESPACE, CHECKED IN BOTH DIRECTIONS — three positions in
    // `register_global` and one in `refuse_global_collision`. Which direction
    // was written is the particular.
    Family {
        code: "PD0005",
        rows: &[
            ("const_collides_with_enum.pd", "as an enum and"),
            ("const_collides_with_function.pd", "as a function and"),
            ("const_collides_with_type_alias.pd", "as a type alias and"),
            (
                "function_collides_with_const.pd",
                "declared as a top-level `const` and as a function",
            ),
        ],
    },
    // PD0007. `const`, `static` and `static mut`, one predicate over
    // `Visibility::Public`; the last two print the same noun (see above).
    Family {
        code: "PD0007",
        rows: &[
            ("pub_const_item.pd", "`pub` on a top-level `const`"),
            ("pub_static_item.pd", "nothing exports `LIMIT`"),
            ("pub_static_mut_item.pd", "nothing exports `COUNTER`"),
        ],
    },
    // PD0009. The LITERAL position of the three that state this rule; the
    // literal that was written is the particular.
    Family {
        code: "PD0009",
        rows: &[
            ("char_pattern_on_int_scrutinee.pd", "found the Char literal"),
            ("enum_payload_literal_type.pd", "the String literal `\"a\"`"),
            (
                "literal_pattern_type_mismatch.pd",
                "the String literal `\"two\"`",
            ),
        ],
    },
    // PD0010. One predicate over the two endpoints; the computed bounds in the
    // payload are fixture data.
    Family {
        code: "PD0010",
        rows: &[
            ("char_range_pattern_empty.pd", "found `'z'..='a'`"),
            ("range_pattern_empty.pd", "found `5..=1`"),
            ("range_pattern_empty_exclusive.pd", "found `3..3`"),
        ],
    },
    // PD0011. One `Err` over `first_binder`; the alternative it names is the
    // particular.
    Family {
        code: "PD0011",
        rows: &[
            ("at_pattern_over_alternatives.pd", "the alternative `n @ 1`"),
            (
                "enum_payload_or_binds.pd",
                "the alternative `x`, which binds",
            ),
            ("or_pattern_binds.pd", "the alternative `P::Num(x)`"),
        ],
    },
    // PD0018. The entry point and the general `is_async` predicate, which the
    // source calls a named sub-case of each other; the spelling is the
    // parameter.
    Family {
        code: "PD0018",
        rows: &[
            (
                "tests/misc/async_main_is_refused.pd",
                "`async fn main` is not implemented",
            ),
            ("async_producer.pd", "`async fn` is not implemented"),
        ],
    },
    // PD0019. One `Err` in `refuse_builtin_definition`; `reason` varies by
    // whether the name is CALLABLE.
    Family {
        code: "PD0019",
        rows: &[
            (
                "shadow_builtin.pd",
                "a function is declared under that name",
            ),
            (
                "shadow_builtin_type.pd",
                "a struct is declared under that name",
            ),
        ],
    },
    // PD0021. One `Err`, `detail` chosen by the CALLER's receiver form — and
    // the reason the locked map's second number for the by-value spelling was
    // dropped rather than minted.
    Family {
        code: "PD0021",
        rows: &[
            (
                "call_mut_method_through_by_value_receiver.pd",
                "a by-value `self` receiver is a COPY, and the callee takes",
            ),
            (
                "call_mut_method_through_chained_shared_receiver.pd",
                "`D::bump`",
            ),
            (
                "call_mut_method_through_shared_receiver.pd",
                "`C::bump` through `self`: `&self` is a SHARED borrow",
            ),
        ],
    },
    // PD0022. The annotated arm of `Stmt::Let`; the type pair is fixture data.
    Family {
        code: "PD0022",
        rows: &[
            ("int_is_not_a_char.pd", "expected Char, found Int"),
            (
                "tests/xfail/alias_as_array_element.pd",
                "expected [Edge; 2]",
            ),
            (
                "tests/xfail/alias_nested_in_tuple_annotation.pd",
                "expected (NodeId, NodeId, Int)",
            ),
        ],
    },
    // PD0042. One `Err` in the `Expr::Call` argument loop; the expected type is
    // the particular, and the const-generic spelling is on the same path (which
    // is what retired PD0047 into this code).
    Family {
        code: "PD0042",
        rows: &[
            ("char_is_not_an_int.pd", "expected Int, found Char"),
            ("const_generic_param.pd", "expected [Int; N]"),
        ],
    },
    // PD0046. The struct-literal position and the assignment-base position; the
    // third (a field read) has no corpus row and is written below.
    Family {
        code: "PD0046",
        rows: &[
            ("try_block.pd", "Unknown struct type: try"),
            (
                "tests/xfail/alias_struct_behind_reference.pd",
                "Unknown struct type: Graph",
            ),
        ],
    },
    // PD0055. Both directions of one rule — a bare `break` out of a value
    // `loop`, and a valued `break` out of a statement loop — which is what
    // retired PD0065 into this code.
    Family {
        code: "PD0055",
        rows: &[
            ("value_loop_bare_break.pd", "found a `break` with no value"),
            (
                "valued_break_in_nested_while.pd",
                "found `break` carrying a Int",
            ),
        ],
    },
    // PD0060. One `Err` guarding `Stmt::Assign`; `detail` is chosen by the
    // `SelfReceiver` kind, which is what retired PD0061 into this code.
    Family {
        code: "PD0060",
        rows: &[
            (
                "self_write_through_by_value_receiver.pd",
                "a by-value `self` receiver is a COPY, and not a `mut` binding",
            ),
            (
                "self_write_through_shared_receiver.pd",
                "`&self` is a SHARED borrow of the receiver. Take",
            ),
        ],
    },
];

/// The nineteen su3 conditions with ONE witness, as (fixture, code, fragment).
///
/// The fragment is not a manifest pin — a code with one witness needs no
/// discriminator — it answers WHICH refusal is wearing the code. That question
/// is live here because several of these sit next to a sibling rule in the same
/// function: PD0057 next to PD0058, PD0072 next to PD0059 next to PD0010,
/// PD0062 next to PD0063.
const SU3_SINGLE_WITNESS: &[(&str, &str, &str)] = &[
    (
        "char_arithmetic_is_not_an_int.pd",
        "PD0043",
        "expected Int, Float or String, found Char",
    ),
    (
        "tests/projects/hello_pdm/src/math.pd",
        "PD0044",
        "No main function found",
    ),
    (
        "pattern_omitted_field_is_unbound.pd",
        "PD0045",
        "Undefined variable: 'y'. Did you mean 'x'?",
    ),
    (
        "generic_enum_constructor.pd",
        "PD0048",
        "constructs a variant of a GENERIC enum",
    ),
    (
        "deref_self_is_not_a_place.pd",
        "PD0050",
        "`*self` is not a place",
    ),
    (
        "method_mut_parameter.pd",
        "PD0052",
        "`mut` parameters on methods are not implemented",
    ),
    ("char_from_non_scalar.pd", "PD0053", "found `55296 as char`"),
    (
        "async_fn.pd",
        "PD0054",
        "a `return` with a value inside an `async fn` is not implemented",
    ),
    (
        "const_string_type.pd",
        "PD0056",
        "may only have a numeric or `bool` type",
    ),
    (
        "value_if_without_else.pd",
        "PD0057",
        "an `if` used as a value to have an `else` branch",
    ),
    (
        "value_if_branch_types.pd",
        "PD0058",
        "both branches of this `if` to have type Int",
    ),
    (
        "range_pattern_mixed_endpoints.pd",
        "PD0059",
        "both endpoints of a range pattern to be the same kind of literal",
    ),
    (
        "static_assign_without_mut.pd",
        "PD0062",
        "a top-level item is read-only unless it is declared `static mut`",
    ),
    (
        "self_is_not_reassignable.pd",
        "PD0063",
        "the receiver binding is not reassignable",
    ),
    (
        "zero_length_array_self_reference.pd",
        "PD0070",
        "recursive type `Z` has no layout",
    ),
    (
        "question_mark_operator.pd",
        "PD0071",
        "the `?` operator is not implemented",
    ),
    (
        "range_pattern_endpoint_type.pd",
        "PD0072",
        "the endpoints of a range pattern to be integer or `char` literals",
    ),
    (
        "shadow_builtin_parameter.pd",
        "PD0075",
        "the parameter `print_int` has the name of a built-in",
    ),
    (
        "value_block_without_tail.pd",
        "PD0076",
        "this block to end in an expression",
    ),
];

/// The four su3 pairs whose two witnesses have CHARACTER-IDENTICAL payloads.
///
/// Listed apart from the families because the family assertion would be false
/// of them by construction: a fragment cannot select one of two identical
/// payloads. The map records no discriminator for these, and the test below
/// asserts the identity rather than inventing one.
const SU3_IDENTICAL_PAIRS: &[(&str, &str, &str, &str)] = &[
    (
        "PD0014",
        "field_shorthand_needs_a_struct_variant.pd",
        "tuple_variant_braces_explicit.pd",
        "Pattern structure doesn't match variant M::Pair",
    ),
    (
        "PD0015",
        "pattern_unknown_field.pd",
        "pattern_unknown_field_explicit.pd",
        "Unknown field z in P::At",
    ),
    (
        "PD0016",
        "bool_split_after_completion.pd",
        "enum_payload_dead_arm.pd",
        "Unreachable pattern detected",
    ),
    (
        "PD0017",
        "generic_method_call.pd",
        "generic_method_path_call.pd",
        "is a generic method, and generic methods are not implemented",
    ),
];

/// PD0001's ten witnesses, as (fixture, fragment).
///
/// FOUR POSITIONS under `src/typeck/`, one rule, and the missing pattern is the
/// parameter. Eight of the ten payloads are distinct; the remaining two PAIRS
/// are character-identical, and the test below names them rather than pretending
/// a fragment tells them apart — `Q::W` is missing from two different programs
/// for the same reason, and the `_`-arm sentence says nothing about the program
/// at all.
const SU3_EXHAUSTIVENESS: &[(&str, &str)] = &[
    ("bool_match_missing_false.pd", "missing patterns false"),
    ("bool_match_true_guarded.pd", "missing patterns true"),
    ("bool_split_ignores_guarded.pd", "missing patterns F::On"),
    ("bool_split_is_not_nested.pd", "missing patterns Q::W"),
    (
        "bool_split_needs_irrefutable_rest.pd",
        "missing patterns E::Pair",
    ),
    (
        "bool_split_values_not_positions.pd",
        "missing patterns E::P —",
    ),
    (
        "enum_payload_collective_coverage.pd",
        "missing patterns Q::W",
    ),
    ("enum_payload_not_exhaustive.pd", "missing patterns P::Num"),
    (
        "guarded_wildcard_only.pd",
        "missing patterns a `_` or binding arm",
    ),
    (
        "nonexhaustive_int_match.pd",
        "missing patterns a `_` or binding arm",
    ),
];

/// The fourteen multi-witness conditions, each one code told apart by its own
/// particular.
///
/// One test over the table rather than fourteen tests: the assertion is
/// `check_family`'s, identical for every row, and the panic names the fixture.
#[test]
fn each_multi_witness_condition_of_su3_is_one_code_told_apart_by_its_particular() {
    for family in SU3_FAMILIES {
        check_family(family);
    }
}

/// The nineteen one-witness conditions, each on the refusal the registry names.
#[test]
fn each_one_witness_condition_of_su3_is_on_the_refusal_the_registry_names() {
    for (fixture, want_code, fragment) in SU3_SINGLE_WITNESS {
        let (r, _dir) = compile_witness(fixture);
        assert_eq!(
            r.code,
            Some(1),
            "{} is a refusal witness and must exit 1; it exited {:?}\n{}",
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

/// ONE CODE OVER TWO IDENTICAL PAYLOADS, four times.
///
/// The pair is asserted to be identical, not merely to share a code: if a later
/// rewording splits one of these sentences, the map owes an answer about whether
/// the rule split with it, and this is where that question surfaces.
#[test]
fn the_identical_payload_pairs_of_su3_take_one_code_and_no_discriminator() {
    for (want_code, first, second, fragment) in SU3_IDENTICAL_PAIRS {
        let (a, _d1) = compile_fixture(first);
        let (b, _d2) = compile_fixture(second);
        let (a_code, a_payload) = a.sole_coded_header(first);
        let (b_code, b_payload) = b.sole_coded_header(second);
        assert_eq!(a_code, *want_code, "{} carries {}", first, a_code);
        assert_eq!(b_code, *want_code, "{} carries {}", second, b_code);
        assert_eq!(
            a_payload, b_payload,
            "{} and {} no longer print the same sentence; the map records them as IDENTICAL",
            first, second
        );
        assert!(
            a_payload.contains(fragment),
            "{} landed on a different refusal: {}",
            first,
            a_payload
        );
    }
}

/// PD0001 AT FOUR POSITIONS, over ten witnesses and eight distinct payloads.
///
/// The two identical pairs are asserted BY NAME. A test that only counted eight
/// groups would stay green if a rewording moved a fixture from one pair to the
/// other, and the pairs are the part of this family a reader is most likely to
/// mistake for two rules.
#[test]
fn the_exhaustiveness_rule_is_one_code_over_ten_witnesses_and_eight_payloads() {
    let mut measured: Vec<(&str, String)> = Vec::new();
    for (fixture, fragment) in SU3_EXHAUSTIVENESS {
        let (r, _dir) = compile_fixture(fixture);
        let (code, payload) = r.sole_coded_header(fixture);
        assert_eq!(code, "PD0001", "{} carries {}", fixture, code);
        assert!(
            payload.contains(fragment),
            "{} carries PD0001 but not on the refusal the map names.\n  want: {}\n  got:  {}",
            fixture,
            fragment,
            payload
        );
        measured.push((fixture, payload));
    }

    let same = |a: &str, b: &str| {
        let get = |n: &str| {
            measured
                .iter()
                .find(|(f, _)| *f == n)
                .map(|(_, p)| p.clone())
                .expect("compiled above")
        };
        assert_eq!(
            get(a),
            get(b),
            "{} and {} were character-identical when the map was locked",
            a,
            b
        );
    };
    same(
        "bool_split_is_not_nested.pd",
        "enum_payload_collective_coverage.pd",
    );
    same("guarded_wildcard_only.pd", "nonexhaustive_int_match.pd");

    let distinct: HashSet<&str> = measured.iter().map(|(_, p)| p.as_str()).collect();
    assert_eq!(
        distinct.len(),
        8,
        "PD0001's ten witnesses printed {} distinct payloads, not the 8 the map records",
        distinct.len()
    );
}

/// A RULE STATED AT SEVERAL TYPE-CHECKER POSITIONS CARRIES THE CODE AT ALL OF
/// THEM — including the positions the corpus does not witness.
///
/// Six positions, each reached by a program written here because no `.pd` in
/// the tree reaches it. Without this the registry rows that say "three
/// positions" or "four positions" would be prose: the witness compile proves
/// one of them and a refactor could leave the rest uncoded.
#[test]
fn the_su3_positions_the_corpus_does_not_witness_carry_the_code_too() {
    let cases: &[(&str, &str, &str, &str)] = &[
        (
            "PD0005, a global declared against a TYPE",
            "type Meters = i64;\nconst Meters: i64 = 1;\nfn main() {\n    print_int(0);\n}\n",
            "PD0005",
            "as a top-level `const` and as a type",
        ),
        (
            "PD0005, the same name declared twice at the top level",
            "const A: i64 = 1;\nconst A: i64 = 2;\nfn main() {\n    print_int(A);\n}\n",
            "PD0005",
            "is declared twice at the top level",
        ),
        (
            "PD0009, a TUPLE pattern against a non-tuple scrutinee",
            "fn main() {\n    let x: i64 = 1;\n    match x {\n        (a, b) => {\n            print_int(a);\n        }\n    }\n}\n",
            "PD0009",
            "found a tuple pattern",
        ),
        (
            "PD0009, a RANGE pattern whose endpoints are another type",
            "fn main() {\n    let c: char = 'a';\n    match c {\n        1..=3 => {\n            print_int(1);\n        }\n        _ => {\n            print_int(0);\n        }\n    }\n}\n",
            "PD0009",
            "found a range pattern, which matches Int",
        ),
        (
            "PD0046, the FIELD READ position",
            "struct GraphData {\n    count: i64,\n}\n\ntype Graph = GraphData;\n\nfn read(g: &Graph) -> i64 {\n    return g.count;\n}\n\nfn main() {\n    let g: Graph = GraphData { count: 41 };\n    print_int(read(&g));\n}\n",
            "PD0046",
            "Unknown struct type: Graph",
        ),
        (
            "PD0053, the OUT-OF-RANGE disjunct rather than the surrogate one",
            "fn main() {\n    let n: i64 = 1114112 as char as i64;\n    print_int(n);\n}\n",
            "PD0053",
            "found `1114112 as char`",
        ),
    ];

    for (what, source, want_code, fragment) in cases {
        let (r, _dir) = compile_source(source);
        assert_eq!(
            r.code,
            Some(1),
            "{}: the program compiled\n{}",
            what,
            r.stdout
        );
        let (code, payload) = r.sole_coded_header(what);
        assert_eq!(code, *want_code, "{}: carries {}", what, code);
        assert!(
            payload.contains(fragment),
            "{}: reached a different refusal.\n  want fragment: {}\n  got payload:   {}",
            what,
            fragment,
            payload
        );
    }
}

/// ONE HELPER IS NOT ONE RULE — the type-checker's twin of the `refuse`-closure
/// mutant above.
///
/// `TypeErrorHelper::type_mismatch` is called from the annotated-`let` arm
/// (PD0022) and from the ASSIGNMENT arm, which states a different rule and has
/// no code. Attaching the code inside the helper — the cheap wiring — would give
/// the assignment refusal PD0022, and every other assertion in this file would
/// still pass. The assignment program below is the control that refuses it.
#[test]
fn the_assignment_arm_sharing_the_type_mismatch_helper_stays_uncoded() {
    let (annotated, _d1) = compile_fixture("int_is_not_a_char.pd");
    let (assigned, _d2) = compile_source(
        "fn main() {\n    let mut n: i64 = 1;\n    n = 'a';\n    print_int(n);\n}\n",
    );

    let (annotated_code, _) = annotated.sole_coded_header("int_is_not_a_char.pd");
    assert_eq!(annotated_code, "PD0022");

    assert_eq!(assigned.code, Some(1), "the assignment program compiled");
    assert!(
        assigned.coded_headers().is_empty(),
        "the assignment arm carries a code, so `with_code` went into the shared helper: {:?}",
        assigned.coded_headers()
    );
    assert!(
        strip_ansi(&assigned.stderr).starts_with("error: "),
        "the uncoded assignment refusal must still print a bare primary header:\n{}",
        assigned.stderr
    );
}

/// THREE RECEIVER RULES, THREE CODES.
///
/// `self` is not reassignable (PD0063), a non-`&mut self` receiver may not be
/// WRITTEN through (PD0060), and a `&mut self` method may not be CALLED through
/// one (PD0021). The three sit within thirty lines of each other, print
/// overlapping advice — all three say "Take `&mut self`" — and the map allocated
/// three numbers. A collapse would pass every per-fixture assertion above.
#[test]
fn the_three_receiver_rules_keep_the_codes_the_map_allocated_them() {
    let trio = [
        ("self_is_not_reassignable.pd", "PD0063"),
        ("self_write_through_shared_receiver.pd", "PD0060"),
        ("call_mut_method_through_shared_receiver.pd", "PD0021"),
    ];
    let mut seen: Vec<String> = Vec::new();
    for (fixture, want) in trio {
        let (r, _dir) = compile_fixture(fixture);
        let (code, _) = r.sole_coded_header(fixture);
        assert_eq!(code, want, "{} carries {}", fixture, code);
        assert!(
            !seen.contains(&code),
            "{} reused {}, which a neighbouring receiver rule already carries",
            fixture,
            code
        );
        seen.push(code);
    }
}

/// THREE RANGE-PATTERN RULES, THREE CODES, IN THE ORDER THE CHECKER APPLIES
/// THEM.
///
/// The endpoint KIND is tested first (PD0072), then that the two endpoints
/// AGREE (PD0059), then that the range can match SOMETHING (PD0010). Each is
/// reachable only when the one before it passed, which is exactly the shape that
/// invites one code for "the range pattern rule".
#[test]
fn the_three_range_pattern_rules_keep_the_codes_the_map_allocated_them() {
    let trio = [
        ("range_pattern_endpoint_type.pd", "PD0072"),
        ("range_pattern_mixed_endpoints.pd", "PD0059"),
        ("range_pattern_empty.pd", "PD0010"),
    ];
    let mut seen: Vec<String> = Vec::new();
    for (fixture, want) in trio {
        let (r, _dir) = compile_fixture(fixture);
        let (code, _) = r.sole_coded_header(fixture);
        assert_eq!(code, want, "{} carries {}", fixture, code);
        assert!(
            !seen.contains(&code),
            "{} reused {}, which a neighbouring range-pattern rule already carries",
            fixture,
            code
        );
        seen.push(code);
    }
}

/// The acceptance side of su3, run to three values.
///
/// Thirty-eight refusals were wired into the type checker in one unit. Without a
/// program that exercises their legal neighbours — a `&mut self` method called
/// through a `&mut self` receiver, an exhaustive `match` with a range arm, an
/// `if` used as a value — every assertion above would be satisfied by a compiler
/// that had started refusing all of them.
#[test]
fn the_legal_neighbours_of_the_type_checker_family_still_compile_link_and_run() {
    let out = compile_link_run(
        "struct C {\n    n: i64,\n}\
         \n\nimpl C {\n    fn bump(&mut self) {\n        self.n = self.n + 1;\n    }\
         \n\n    fn drive(&mut self) {\n        self.bump();\n    }\n}\
         \n\nfn classify(k: i64) -> i64 {\n    match k {\n        0..=9 => {\
         \n            return 1;\n        }\n        _ => {\n            return 2;\
         \n        }\n    }\n}\
         \n\nfn main() {\n    let mut c: C = C { n: 41 };\n    c.drive();\
         \n    print_int(c.n);\
         \n\n    let picked: i64 = if c.n > 41 { 7 } else { 8 };\
         \n    print_int(picked);\n    print_int(classify(3));\n}\n",
    );
    assert_eq!(
        out, "42\n7\n1\n",
        "the legal neighbours of the type-checker family did not produce their values"
    );
}
