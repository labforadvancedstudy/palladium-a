//! Stable diagnostic codes, end to end — GI-12 su1's two seed conditions.
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
    let dir = TempDir::new().expect("tempdir");
    let out = Command::new(env!("CARGO_BIN_EXE_pdc"))
        .current_dir(dir.path())
        .arg("compile")
        .arg(repo_root().join("tests/reject").join(name))
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

#[test]
fn the_cast_relation_is_one_code_told_apart_by_its_found_clause() {
    check_family(&CAST);
}

#[test]
fn the_const_initialiser_rule_is_one_code_told_apart_by_its_fault() {
    check_family(&CONST_INIT);
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
#[test]
fn an_unwired_refusal_carries_no_code_rather_than_a_fallback() {
    let (r, _dir) = compile_fixture("ref_parameter.pd");
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
    fixtures.push("ref_parameter.pd");

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
