//! `pdc --version` must be the version in `Cargo.toml`, and no source file may
//! state that version by hand.
//!
//! It was not. `src/cli.rs` carried `#[command(version = "0.1.0-alpha")]` as a
//! string literal while the package was at 0.3.0, so the binary shipped with
//! milestone M1 answered `pdc 0.1.0-alpha`. `src/main.rs` carried a *third*
//! spelling, `v0.1-alpha`, in the banner printed on every compile.
//!
//! That is small and it is not cosmetic. This milestone's thesis is that the
//! compiler must not make claims it cannot back, and the version string is the
//! claim every bug report, every bisect and every "which build am I running"
//! rests on. Two independent hand-maintained copies of one fact drift; the fix
//! is to delete the copies, not to correct them.
//!
//! TWO GATES, AND NEITHER SUBSUMES THE OTHER.
//! `make version-gate` (scripts/version-gate.sh) RUNS every declared binary and
//! reads `--version`. That is the only thing that answers what a user is
//! actually running, and it is why the source-side check below does not try to
//! be it. But it can only read output shaped `<name> <version>` on one non-blank
//! line — hand it a banner and it reports "not `<name> <version>`", by design.
//! So the surface it structurally cannot see is exactly where two of the three
//! defects lived: the banner in `src/main.rs`, and a `pub const` in
//! `src/lib.rs` that no binary printed at all. This file covers the source
//! surface; the version gate covers the shipped behaviour; a hole in either is
//! not covered by the other.
//!
//! WHY THIS FILE STOPPED NAMING A SPELLING.
//! The previous version asserted that `src/cli.rs` literally contains
//! `env!("CARGO_PKG_VERSION")`. `main` fixed the same defect with
//! `#[command(version)]`, which is the same derivation and contains no `env!`
//! at all — so the two branches' correct code each made the other's test red,
//! and the merge had no resolution that was not a silent revert. A test must
//! assert the FACT ("the version is not a literal"), never one derivation's
//! spelling. That is the same correction already made to `declared_match_finding`
//! in tests/d3b_tail_if.rs.
//!
//! WHAT THE SOURCE SCAN IS, AT ITS TRUE SIZE.
//! Default-deny over every `*.rs` under `src/`: a string literal may not contain
//! a version-shaped token. The old check was default-ALLOW over a hand-written
//! list of two files, which is why `src/lib.rs:43-46` — a fourth spelling,
//! rendering `v0.3.0-alpha`, a version the package has never had — was invisible
//! to it for the whole round in which the other three were fixed. A new file is
//! now covered the day it is added; that is the property the file list could not
//! have.
//!
//! ITS COST, MEASURED BEFORE IT WAS WRITTEN, because "no version-shaped literal
//! anywhere in src/" was proposed as having "exactly one row to adjudicate" and
//! that is not what the tree says. Scanning all 68 files:
//!
//!   * 90 version-shaped tokens if comments and code count;
//!   * 64 of them inside string literals;
//!   * 46 if the shape is narrowed to what a package version actually looks
//!     like — `\d+.\d+.\d+`, or `v` followed by two or three components — which
//!     drops `§9.2`, `jsonrpc "2.0"`, `1000.0` and `const PI = 3.14`;
//!   * and all of those live under `src/package/`, the package manager, where a
//!     version literal is a fixture describing SOME OTHER package.
//!
//! THAT 46 IS A HAND COUNT FROM AN EARLIER ROUND AND THIS TEST NOW PRINTS 42.
//! The two were produced by different means and only the second is reproducible
//! — empty the table below and `cargo test --release --test
//! version_matches_cargo_toml` lists all 42, one per line — so 42 is what the
//! exemption table is built from. The 46 is left standing above rather than
//! quietly corrected, because it is what the previous round reported and the
//! delta is not re-derivable from what was written down.
//!
//! AND THE EXEMPTION IS NOT A DIRECTORY, WHICH IS THIS ROUND'S RETRACTION. It
//! was: one prefix, `src/package/`, plus an assertion that the prefix suppressed
//! SOMETHING. That reads like default-deny and is not — a new literal under the
//! prefix is suppressed AND makes the non-vacuity counter go up, so the check
//! becomes more satisfied by exactly the thing it exists to catch. The grain is
//! now the FINDING: 15 adjudicated `(path, token, exact count)` rows covering all
//! 42, and anything not on a row is red. What that still does not cover is
//! stated where the rows are.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The version cargo read out of `Cargo.toml` when it built this test. Note
/// that the test binary and `pdc` are built from the same manifest, so this is
/// the same fact the binary should be reporting.
const EXPECTED: &str = env!("CARGO_PKG_VERSION");

#[test]
fn version_flag_matches_cargo_package_version() {
    let out = Command::new(env!("CARGO_BIN_EXE_pdc"))
        .arg("--version")
        .output()
        .expect("failed to run pdc --version");

    assert!(
        out.status.success(),
        "pdc --version exited {:?}: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );

    let printed = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert_eq!(
        printed,
        format!("pdc {}", EXPECTED),
        "`pdc --version` reports a version the package does not have. \
         Cargo.toml says {}. It said `pdc 0.1.0-alpha` for two releases because \
         src/cli.rs held the string by hand.",
        EXPECTED
    );
}

// ---------------------------------------------------------------------------
// The source scan. ONE definition of the predicate, used by both the tree walk
// and the fault-injection rows below it — the copy-of-a-copy failure that
// scripts/test-gate-probe.sh was carrying (two hand-written spellings of one
// regex, only one of them in force) is not repeated here.
// ---------------------------------------------------------------------------

/// The directories a suppression may be granted in at all, with the reason,
/// because a suppression nobody has to justify is a hole nobody sees.
///
/// THIS IS THE OUTER BOUND, NOT THE EXEMPTION. It says where an adjudicated
/// finding is allowed to live; `ADJUDICATED` below says which findings those
/// are. A directory prefix alone was the previous version and it was NOT
/// default-deny, which is the defect this round retracts — see the comment on
/// `ADJUDICATED`.
const EXEMPT: &[(&str, &str)] = &[(
    "src/package/",
    "the package manager. Every version literal under it describes SOME OTHER \
     package: Version::parse(\"1.2.3\"), a lockfile entry, a registry response, \
     the default manifest `pdc new` writes. Nothing here prints this compiler's \
     own version.",
)];

/// EVERY suppressed finding, at FINDING GRAIN: `(path, token, exact count)`.
///
/// WHY THIS EXISTS, AND WHAT IT REPLACES. The previous version suppressed by
/// DIRECTORY PREFIX and then asserted that each prefix `suppressed` something.
/// That reads like default-deny and is not: plant a NEW compiler-identity
/// literal under `src/package/` — `println!("pdc v0.4.0")` — and `findings_in`
/// finds it, the prefix hides it, and the non-vacuity counter goes N to N+1, so
/// the assertion becomes MORE satisfied. It could only ever fire if the package
/// manager stopped holding version literals entirely, which for a package
/// manager will not happen. The check was true as written and vacuous as
/// operated, and it was defended by its stated intent rather than by its
/// mechanism.
///
/// So the grain moved to the finding. A suppression is now something a human
/// wrote down ONE AT A TIME, and anything not on this list is a finding — the
/// new literal above goes red until someone adds a row for it, which is the
/// adjudication the directory grant only claimed to be.
///
/// THE KEY IS (path, token) AND NOT (path, line, token), deliberately: moving a
/// literal within its file is not a new claim about this compiler's version, and
/// pinning line numbers would make every unrelated edit under `src/package/` red.
/// The COUNT is exact in both directions — one more occurrence is unadjudicated,
/// one fewer is a dead row to delete — so a swap (delete one literal, add
/// another with the same token in the same file) is the one move this table does
/// not see. That is stated rather than left to the next round: it is a smaller
/// hole than the directory grant by 42 literals to 1, not zero.
///
/// WHAT WAS ADJUDICATED, once, when this table was written: all 42 are semver
/// FIXTURES — `Version::parse("1.2.3")` in a resolver unit test, lockfile
/// entries for a package called `http`, a registry response, and the `0.1.0`
/// that `pdc new` writes into the manifest of the package it is CREATING. The
/// package's own version is 0.3.0 and no row states it.
///
/// AND THERE IS DELIBERATELY NO "no row may equal env!(CARGO_PKG_VERSION)"
/// CHECK. It would add no detection — a literal stating this compiler's version
/// is a NEW literal, so it is already unadjudicated and already red — while
/// guaranteeing a false alarm at the bump to 1.0.0, where thirteen unrelated
/// `http` fixtures would suddenly read as compiler identity.
const ADJUDICATED: &[(&str, &str, usize)] = &[
    ("src/package/dependency.rs", "0.1.0", 1),
    ("src/package/dependency.rs", "1.0.0", 3),
    ("src/package/dependency.rs", "1.1.0", 2),
    ("src/package/dependency.rs", "1.2.0", 2),
    ("src/package/dependency.rs", "1.2.3", 3),
    ("src/package/dependency.rs", "1.3.0", 1),
    ("src/package/dependency.rs", "2.0.0", 5),
    ("src/package/lockfile.rs", "1.0.0", 8),
    ("src/package/lockfile.rs", "1.1.0", 4),
    ("src/package/lockfile.rs", "2.0.0", 1),
    ("src/package/mod.rs", "0.1.0", 1),
    ("src/package/registry.rs", "0.1.0", 2),
    ("src/package/registry.rs", "1.0.0", 5),
    ("src/package/registry.rs", "1.1.0", 3),
    ("src/package/registry.rs", "2.0.0", 1),
];

/// A string literal that is nothing but a pre-release tail.
///
/// This is the shape that killed `VERSION_STRING` in `src/lib.rs`: the version
/// was derived correctly with `env!` and then `"-alpha"` was concatenated onto
/// it, producing `v0.3.0-alpha` — a version this package does not have, built
/// out of parts each of which looks innocent. A version-shaped scan cannot see
/// it, because the literal half has no digits in it.
///
/// WHAT IT DOES NOT CATCH: `format!("{}-alpha", VERSION)` (the tail is inside a
/// wider literal), a tail assembled from two literals, or anything built at
/// run time. Like every lexical guard in this repository it makes the ORDINARY
/// spelling visible, not every spelling.
const PRERELEASE_TAILS: &[&str] = &["-alpha", "-beta", "-rc", "-pre", "-dev", "-nightly"];

/// Every string-literal body in `c`, as (index of its first character, text).
///
/// Comments, char literals and lifetimes are stepped over rather than searched:
/// a version in a comment is prose, and `'"'` — which appears in three files
/// under `src/` — must not open a string that swallows the code after it. Raw
/// strings (`r"…"`, `r#"…"#`) are read as strings, because `src/cli.rs` and
/// `src/main.rs` both hold their long text that way and the banner defect lived
/// in one of them.
fn string_literals(c: &[char]) -> Vec<(usize, String)> {
    let n = c.len();
    let mut out: Vec<(usize, String)> = Vec::new();
    let mut i = 0usize;
    while i < n {
        // Line comment.
        if c[i] == '/' && i + 1 < n && c[i + 1] == '/' {
            while i < n && c[i] != '\n' {
                i += 1;
            }
            continue;
        }
        // Block comment. Rust nests them, so this counts depth.
        if c[i] == '/' && i + 1 < n && c[i + 1] == '*' {
            let mut depth = 1usize;
            i += 2;
            while i < n && depth > 0 {
                if c[i] == '/' && i + 1 < n && c[i + 1] == '*' {
                    depth += 1;
                    i += 2;
                } else if c[i] == '*' && i + 1 < n && c[i + 1] == '/' {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            continue;
        }
        // Raw string: r"…" or r#…#"…"#…#. The leading `r` must start a token,
        // or an identifier ending in `r` would open one.
        if c[i] == 'r' && !(i > 0 && (c[i - 1].is_alphanumeric() || c[i - 1] == '_')) {
            let mut j = i + 1;
            let mut hashes = 0usize;
            while j < n && c[j] == '#' {
                hashes += 1;
                j += 1;
            }
            if j < n && c[j] == '"' {
                let start = j + 1;
                let mut k = start;
                while k < n {
                    if c[k] == '"' {
                        let mut h = 0usize;
                        while h < hashes && k + 1 + h < n && c[k + 1 + h] == '#' {
                            h += 1;
                        }
                        if h == hashes {
                            break;
                        }
                    }
                    k += 1;
                }
                let end = k.min(n);
                out.push((start, c[start..end].iter().collect()));
                i = (end + 1 + hashes).min(n);
                continue;
            }
        }
        // Ordinary string. `\"` does not end it and `\\` does not escape the
        // quote after it.
        if c[i] == '"' {
            let start = i + 1;
            let mut k = start;
            while k < n {
                if c[k] == '\\' {
                    k += 2;
                } else if c[k] == '"' {
                    break;
                } else {
                    k += 1;
                }
            }
            let end = k.min(n);
            out.push((start, c[start..end].iter().collect()));
            i = (end + 1).min(n);
            continue;
        }
        // `'` is a char literal only in the shapes `'x'` and `'\…'`; otherwise
        // it is a lifetime and must not open anything.
        if c[i] == '\'' {
            if i + 1 < n && c[i + 1] == '\\' {
                let mut k = i + 2;
                while k < n && c[k] != '\'' {
                    k += 1;
                }
                i = (k + 1).min(n);
                continue;
            }
            if i + 2 < n && c[i + 2] == '\'' {
                i += 3;
                continue;
            }
            i += 1;
            continue;
        }
        i += 1;
    }
    out
}

/// `\d+.\d+` optionally followed by `.\d+`, starting at `i`. Returns the index
/// just past it and how many components it had.
fn numeric_version_at(c: &[char], i: usize) -> Option<(usize, usize)> {
    let n = c.len();
    let mut j = i;
    while j < n && c[j].is_ascii_digit() {
        j += 1;
    }
    if j == i || j >= n || c[j] != '.' {
        return None;
    }
    let second = j + 1;
    let mut k = second;
    while k < n && c[k].is_ascii_digit() {
        k += 1;
    }
    if k == second {
        return None;
    }
    if k < n && c[k] == '.' {
        let third = k + 1;
        let mut m = third;
        while m < n && c[m].is_ascii_digit() {
            m += 1;
        }
        if m > third {
            return Some((m, 3));
        }
    }
    Some((k, 2))
}

/// The version-shaped tokens in `s`.
///
/// TWO SHAPES, and the narrowing is what keeps this check usable: a bare
/// `\d+.\d+.\d+` (three components — `0.1.0-alpha` and `0.3.0` are both this),
/// or `v` followed by two or three (`v0.1-alpha`, `v0.4.0`). A bare two-component
/// number is NOT a version here: `"2.0"` is JSON-RPC, `3.14` is a code sample,
/// `§9.2` is a spec reference, and this tree holds 18 of them that no reviewer
/// wants to see in a failure.
///
/// It follows that both retired literals — `0.1.0-alpha` and `v0.1-alpha` —
/// are caught by SHAPE rather than by name. The list they used to be on
/// (`["0.1.0-alpha", "v0.1-alpha", EXPECTED]`) let a FUTURE literal through:
/// re-hardcoding the banner as `v0.4.0` today matched nothing on the list and
/// nothing in `--version`, and would have gone quiet again for exactly one bump.
fn version_tokens(s: &str) -> Vec<String> {
    let c: Vec<char> = s.chars().collect();
    let n = c.len();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < n {
        let starts_token =
            i == 0 || !(c[i - 1].is_ascii_alphanumeric() || c[i - 1] == '_' || c[i - 1] == '.');
        if starts_token && c[i] == 'v' {
            if let Some((end, _)) = numeric_version_at(&c, i + 1) {
                out.push(c[i..end].iter().collect());
                i = end;
                continue;
            }
        }
        if starts_token && c[i].is_ascii_digit() {
            if let Some((end, components)) = numeric_version_at(&c, i) {
                if components == 3 {
                    out.push(c[i..end].iter().collect());
                    i = end;
                    continue;
                }
                i = end;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// The pre-release tail this literal IS, if it is nothing else.
///
/// Whole-literal, not substring: `"2.0.0-alpha"` is a version (caught by shape),
/// `"-o"` and `"-O2"` are gcc flags the driver passes and must stay silent.
fn bare_prerelease_tail(s: &str) -> Option<&'static str> {
    for tail in PRERELEASE_TAILS {
        if let Some(rest) = s.strip_prefix(tail) {
            if rest.chars().all(|ch| ch.is_ascii_digit() || ch == '.') {
                return Some(tail);
            }
        }
    }
    None
}

fn line_of(c: &[char], idx: usize) -> usize {
    1 + c[..idx.min(c.len())]
        .iter()
        .filter(|&&ch| ch == '\n')
        .count()
}

fn short(s: &str) -> String {
    if s.chars().count() > 60 {
        format!("{}…", s.chars().take(60).collect::<String>())
    } else {
        s.to_string()
    }
}

/// One reason a source file may not be committed as it stands.
///
/// STRUCTURED RATHER THAN FORMATTED, because the exemption is now keyed on
/// `(path, token)` and a formatted string carries the line number inside it —
/// keying on that would make every unrelated edit under an exempt directory red.
#[derive(Clone)]
struct Finding {
    path: String,
    line: usize,
    /// The version-shaped token, or the bare pre-release tail, that was found.
    /// The two are lexically disjoint, so this alone says which rule fired.
    token: String,
    message: String,
}

impl Finding {
    fn describe(&self) -> String {
        format!("{}:{}: {}", self.path, self.line, self.message)
    }
}

/// Every reason `src` may not be committed as it stands.
fn findings_in(path: &str, src: &str) -> Vec<Finding> {
    let c: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    for (at, lit) in string_literals(&c) {
        let line = line_of(&c, at);
        for token in version_tokens(&lit) {
            out.push(Finding {
                path: path.to_string(),
                line,
                message: format!(
                    "the string literal \"{}\" states a version by hand ({}). \
                     Derive it — `#[command(version)]` or env!(\"CARGO_PKG_VERSION\") — \
                     or this drifts at the next bump, as 0.1.0-alpha did for two releases.",
                    short(&lit),
                    token
                ),
                token,
            });
        }
        if let Some(tail) = bare_prerelease_tail(&lit) {
            out.push(Finding {
                path: path.to_string(),
                line,
                token: tail.to_string(),
                message: format!(
                    "the string literal \"{}\" is a bare pre-release tail ({}). \
                     Glued onto a derived version it manufactures a version the package \
                     does not have — that is what src/lib.rs's VERSION_STRING rendered \
                     as v0.3.0-alpha until it was deleted.",
                    short(&lit),
                    tail
                ),
            });
        }
    }
    out
}

/// Split `findings` against `ADJUDICATED`, returning `(unadjudicated, stale)`.
///
/// `unadjudicated` = a finding no row covers, or the (N+1)th occurrence of a row
/// that adjudicated N. Those are the ones that must go red: a new version
/// literal is a new claim, and a claim nobody has looked at is the whole defect.
///
/// `stale` = a row that matched FEWER times than it declares, including zero. A
/// dead row is deleted rather than inherited — the one property the directory
/// grant did have, kept, and now at the grain where it means something.
fn adjudicate(findings: &[Finding]) -> (Vec<String>, Vec<String>) {
    let mut remaining: Vec<usize> = ADJUDICATED.iter().map(|(_, _, n)| *n).collect();
    let mut unadjudicated = Vec::new();
    for f in findings {
        match ADJUDICATED
            .iter()
            .position(|(path, token, _)| *path == f.path && *token == f.token)
        {
            Some(i) if remaining[i] > 0 => remaining[i] -= 1,
            Some(_) => unadjudicated.push(format!(
                "{}  [more occurrences than the adjudicated row (\"{}\", \"{}\", …) allows]",
                f.describe(),
                f.path,
                f.token
            )),
            None => unadjudicated.push(f.describe()),
        }
    }
    let stale = ADJUDICATED
        .iter()
        .zip(&remaining)
        .filter(|(_, left)| **left > 0)
        .map(|((path, token, want), left)| {
            format!(
                "(\"{}\", \"{}\", {}) matched only {} time(s) — delete it or fix the count. \
                 A suppression that hides nothing is hiding whatever lands there next.",
                path,
                token,
                want,
                want - left
            )
        })
        .collect();
    (unadjudicated, stale)
}

fn rust_sources(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) {
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", dir.display(), e))
        .map(|e| e.expect("directory entry").path())
        .collect();
    paths.sort();
    for p in paths {
        if p.is_dir() {
            rust_sources(&p, root, out);
        } else if p.extension().and_then(|e| e.to_str()) == Some("rs") {
            let rel = p
                .strip_prefix(root)
                .expect("under the manifest dir")
                .to_string_lossy()
                .replace('\\', "/");
            let text =
                fs::read_to_string(&p).unwrap_or_else(|e| panic!("cannot read {}: {}", rel, e));
            out.push((rel, text));
        }
    }
}

#[test]
fn no_source_file_hardcodes_the_version() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rust_sources(&root.join("src"), &root, &mut files);

    // The scan is worth nothing if the walk is empty or lost the two files the
    // defect actually shipped in, so the walk is checked before its verdict is.
    assert!(
        files.len() >= 60,
        "the walk found only {} .rs files under src/ — it is not reading the tree it reports on",
        files.len()
    );
    for required in ["src/cli.rs", "src/main.rs", "src/lib.rs"] {
        assert!(
            files.iter().any(|(p, _)| p == required),
            "the walk did not reach {}, which is one of the files the version drifted in",
            required
        );
    }

    let mut all: Vec<Finding> = Vec::new();
    for (rel, src) in &files {
        all.extend(findings_in(rel, src));
    }

    // Every row must lie under a directory a suppression may be granted in at
    // all. Checked before the rows are used, so a suppression cannot be smuggled
    // into src/cli.rs — the file the whole defect shipped in — by adding a line
    // to a table that is otherwise about the package manager.
    for (path, token, _) in ADJUDICATED {
        assert!(
            EXEMPT.iter().any(|(prefix, _)| path.starts_with(prefix)),
            "the adjudicated row (\"{}\", \"{}\") is outside every directory a \
             suppression may be granted in ({}). A version literal there is the \
             defect this file exists for.",
            path,
            token,
            EXEMPT
                .iter()
                .map(|(p, _)| *p)
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    let (unadjudicated, stale) = adjudicate(&all);

    assert!(
        unadjudicated.is_empty(),
        "{} version literal(s) nobody has adjudicated:\n  {}\n\n\
         If one of these is legitimate — a fixture describing SOME OTHER \
         package — add its (path, token, count) to ADJUDICATED with the reason \
         in the commit. Do not widen the directory list: a directory grant \
         suppresses the NEXT literal too, including one that states this \
         compiler's own version.",
        unadjudicated.len(),
        unadjudicated.join("\n  ")
    );

    assert!(
        stale.is_empty(),
        "{} adjudicated row(s) no longer match the tree:\n  {}",
        stale.len(),
        stale.join("\n  ")
    );

    // And a directory that no longer holds a single adjudicated finding has no
    // business bounding anything.
    for (prefix, why) in EXEMPT {
        assert!(
            ADJUDICATED
                .iter()
                .any(|(path, _, _)| path.starts_with(prefix)),
            "the directory `{}` bounds no adjudicated finding on this tree. \
             Delete it. It was granted because: {}",
            prefix,
            why
        );
    }
}

/// AND THE SCANNER IS FAULT-INJECTED, against the same `findings_in` the tree
/// walk calls — not a second copy of it. Every row is a shape that a
/// substring-grep version of this check got wrong, or a legitimate line it must
/// stay silent on. A boundary a reader has to infer from the code is one the
/// next round re-litigates, so the cost of the narrowing is rows, not prose.
#[test]
fn the_source_scan_flags_literals_and_nothing_else() {
    let rows: &[(&str, usize, &str)] = &[
        (
            "#[command(version = \"0.1.0-alpha\")]\n",
            1,
            "the literal that actually shipped, twice",
        ),
        (
            "    println!(\"pdc v0.1-alpha\");\n",
            1,
            "the banner's third spelling: two components, v-prefixed",
        ),
        (
            "    println!(\"pdc v0.4.0\");\n",
            1,
            "a FUTURE literal — invisible to a forbidden-list of past ones",
        ),
        (
            "#[command(version = env!(\"CARGO_PKG_VERSION\"))]\n",
            0,
            "the derivation this branch used",
        ),
        (
            "#[command(version)] // = CARGO_PKG_VERSION\n",
            0,
            "the derivation main used — the same fact, another spelling",
        ),
        (
            "// the literal here said \"0.1.0-alpha\" while the package was at 0.3.0\n",
            0,
            "prose about the defect is not the defect; a line comment is stepped over",
        ),
        (
            "/* an older banner read \"v0.1-alpha\" */\n",
            0,
            "and so is a block comment",
        ),
        (
            "let m = r#\"{\"jsonrpc\":\"2.0\",\"id\":1}\"#;\n",
            0,
            "JSON-RPC's protocol version: two components, no `v`, 18 of them in src/lsp",
        ),
        (
            "let s = r#\"pdc v0.4.0\"#;\n",
            1,
            "but a raw string is still a string — cli.rs and main.rs both hold text that way",
        ),
        (
            "println!(\"   Total time: {:.2}ms\", t.as_secs_f64() * 1000.0);\n",
            0,
            "1000.0 is arithmetic, outside any literal",
        ),
        (
            "// Phase 3.6: Effect analysis, per language-spec.md §9.2\n",
            0,
            "spec section numbers, 8 of them in src/",
        ),
        (
            "let q = '\"'; let v = \"0.2.0\";\n",
            1,
            "a QUOTE CHAR LITERAL must not open a string: three files under src/ \
             contain `'\"'`, and swallowing from one hides every literal after it",
        ),
        (
            "let a = \"x\\\"y\"; let v = \"0.2.0\";\n",
            1,
            "an escaped quote does not end a string either — the same erasure one \
             character down",
        ),
        (
            "fn f<'a>(s: &'a str) -> &'a str { s }\nconst V: &str = \"9.9.9\";\n",
            1,
            "a lifetime is not a char literal; treating `'a` as one runs to the next \
             quote and eats the code between",
        ),
        (
            "const S: &str = concat!(\"pdc v\", env!(\"CARGO_PKG_VERSION\"), \"-alpha\");\n",
            1,
            "src/lib.rs's deleted VERSION_STRING: derived, then a tail glued on, \
             rendering a version the package does not have. No digits in the literal \
             half — only the tail rule sees it.",
        ),
        (
            "cmd.arg(\"-o\").arg(\"-O2\").arg(\"-include\");\n",
            0,
            "the driver's gcc flags start with `-` too, and must stay silent",
        ),
        (
            "let v = Version::parse(\"2.0.0-alpha\").unwrap();\n",
            1,
            "a full version is caught by SHAPE, tail or no tail — this row is why \
             src/package/ needs an adjudicated exemption rather than a looser rule",
        ),
    ];

    let mut bad = Vec::new();
    for (src, want, why) in rows {
        let got = findings_in("row.rs", src);
        if got.len() != *want {
            bad.push(format!(
                "{:?} -> {} finding(s), wanted {} ({})\n      {}",
                src,
                got.len(),
                want,
                why,
                got.iter()
                    .map(|f| f.describe())
                    .collect::<Vec<_>>()
                    .join("\n      ")
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "the scanner does not do what its rows say:\n  {}",
        bad.join("\n  ")
    );
}

/// AND THE EXEMPTION IS FAULT-INJECTED, which the previous one was not.
///
/// The directory-prefix version was asserted to be "red if it ever suppresses
/// nothing". That sentence is true and operationally vacuous: a NEW literal
/// under the exempt directory made the counter go UP, so the assertion got more
/// satisfied by exactly the thing it was supposed to catch. It could only fire
/// if `src/package/` stopped holding version literals, which for a package
/// manager will not happen.
///
/// The rows below are the four moves that distinguish the two designs, run
/// against `adjudicate` — the function the tree walk calls — on a SYNTHETIC
/// finding set rather than on the tree, so they measure the mechanism and do not
/// go red when someone edits the package manager.
#[test]
fn the_exemption_is_default_deny() {
    // The finding set a tree exactly matching the table would produce.
    let exactly_the_table = |extra: Vec<Finding>, drop_first: bool| -> Vec<Finding> {
        let mut v = Vec::new();
        for (i, (path, token, count)) in ADJUDICATED.iter().enumerate() {
            let n = if drop_first && i == 0 {
                count - 1
            } else {
                *count
            };
            for line in 0..n {
                v.push(Finding {
                    path: path.to_string(),
                    line: line + 1,
                    token: token.to_string(),
                    message: "synthetic".to_string(),
                });
            }
        }
        v.extend(extra);
        v
    };

    // 0. BASELINE: a tree that matches the table is silent both ways. Without
    //    this the rows below would pass for an `adjudicate` that reports
    //    everything, and the table would be inert.
    let (unadj, stale) = adjudicate(&exactly_the_table(Vec::new(), false));
    assert!(
        unadj.is_empty() && stale.is_empty(),
        "the table does not accept its own contents: {:?} / {:?}",
        unadj,
        stale
    );

    // 1. THE MOVE THE DIRECTORY GRANT COULD NOT SEE. A new compiler-identity
    //    literal, planted in the most-exempt file there is. Under the prefix
    //    design it was suppressed AND incremented the non-vacuity counter; here
    //    it is a finding, and stays one until someone writes a row for it.
    let (path0, _, _) = ADJUDICATED[0];
    let planted = findings_in(path0, "    println!(\"pdc v9.9.9\");\n");
    assert_eq!(
        planted.len(),
        1,
        "the scanner did not see the planted literal at all, so this row measures nothing"
    );
    let (unadj, stale) = adjudicate(&exactly_the_table(planted, false));
    assert_eq!(
        unadj.len(),
        1,
        "a version literal on no adjudicated row was suppressed anyway. That is \
         the directory-grant defect: a hardcoded compiler version under `{}` \
         ships silently.",
        path0
    );
    assert!(
        stale.is_empty(),
        "and it must not disturb the other rows: {:?}",
        stale
    );

    // 2. A row that matches FEWER times than it declares is stale, including the
    //    zero case the old check covered — and only that row.
    let (unadj, stale) = adjudicate(&exactly_the_table(Vec::new(), true));
    assert!(
        unadj.is_empty(),
        "shrinking a row must not invent findings: {:?}",
        unadj
    );
    assert_eq!(
        stale.len(),
        1,
        "a row that no longer matches its declared count was not reported stale — \
         a suppression that has outlived its literals hides whatever lands next"
    );
    assert!(
        stale[0].contains(path0),
        "the stale report must name the row that went dead, got: {}",
        stale[0]
    );

    // 3. And one occurrence MORE than a row allows is unadjudicated, not
    //    absorbed. This is the whole difference between a count and a boolean,
    //    and it is what stops a new literal hiding behind a token that already
    //    appears in the same file.
    let (path, token, count) = ADJUDICATED[0];
    let extra = vec![Finding {
        path: path.to_string(),
        line: 999,
        token: token.to_string(),
        message: "synthetic".to_string(),
    }];
    let (unadj, stale) = adjudicate(&exactly_the_table(extra, false));
    assert_eq!(
        unadj.len(),
        1,
        "occurrence {} of an adjudicated ({}, {}) was absorbed by a row that \
         adjudicated {}",
        count + 1,
        path,
        token,
        count
    );
    assert!(
        stale.is_empty(),
        "a fully-matched row must not also be stale: {:?}",
        stale
    );
}
