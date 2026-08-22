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
//! list of two files, which is why `src/lib.rs:42` — a fourth spelling,
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
//!   * and all 46 of those live under `src/package/`, the package manager,
//!     where a version literal is a fixture describing SOME OTHER package.
//!
//! So the exemption list below has one entry, adjudicated by directory, and the
//! test fails if it ever suppresses nothing — a dead exemption is deleted rather
//! than inherited. What it does NOT cover is stated where it is granted.

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

/// Directories whose version literals are adjudicated as legitimate, with the
/// reason, because a suppression nobody has to justify is a hole nobody sees.
///
/// GRANTED AT DIRECTORY GRAIN, and that is a real cost: a hardcoded compiler
/// version inside `src/package/` would be suppressed too. What makes it
/// acceptable is that nothing under `src/package/` states this compiler's
/// identity — `--version` is `src/cli.rs`, the banner is `src/main.rs`, and both
/// are scanned — and that per-literal rows would be a 46-entry inventory that
/// has to be edited every time a package-manager test changes.
const EXEMPT: &[(&str, &str)] = &[(
    "src/package/",
    "the package manager. Every version literal under it describes SOME OTHER \
     package: Version::parse(\"1.2.3\"), a lockfile entry, a registry response, \
     the default manifest `pdc new` writes. 46 measured on this tree. Nothing \
     here prints this compiler's own version.",
)];

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

/// Every reason `src` may not be committed as it stands, as `path:line: why`.
fn findings_in(path: &str, src: &str) -> Vec<String> {
    let c: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    for (at, lit) in string_literals(&c) {
        let line = line_of(&c, at);
        for token in version_tokens(&lit) {
            out.push(format!(
                "{}:{}: the string literal \"{}\" states a version by hand ({}). \
                 Derive it — `#[command(version)]` or env!(\"CARGO_PKG_VERSION\") — \
                 or this drifts at the next bump, as 0.1.0-alpha did for two releases.",
                path,
                line,
                short(&lit),
                token
            ));
        }
        if let Some(tail) = bare_prerelease_tail(&lit) {
            out.push(format!(
                "{}:{}: the string literal \"{}\" is a bare pre-release tail ({}). \
                 Glued onto a derived version it manufactures a version the package \
                 does not have — that is what src/lib.rs's VERSION_STRING rendered \
                 as v0.3.0-alpha until it was deleted.",
                path,
                line,
                short(&lit),
                tail
            ));
        }
    }
    out
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

    let mut findings: Vec<String> = Vec::new();
    let mut suppressed = vec![0usize; EXEMPT.len()];
    for (rel, src) in &files {
        for f in findings_in(rel, src) {
            match EXEMPT
                .iter()
                .position(|(prefix, _)| rel.starts_with(prefix))
            {
                Some(i) => suppressed[i] += 1,
                None => findings.push(f),
            }
        }
    }

    assert!(
        findings.is_empty(),
        "{} source file(s) state a version by hand:\n  {}",
        findings.len(),
        findings.join("\n  ")
    );

    // A dead exemption is deleted, not inherited: if the directory stops holding
    // the literals the exemption was granted for, the grant is now covering
    // something nobody adjudicated.
    for (i, (prefix, why)) in EXEMPT.iter().enumerate() {
        assert!(
            suppressed[i] > 0,
            "the exemption for `{}` suppressed nothing on this tree, so it is \
             hiding whatever lands there next. Delete it. It was granted because: {}",
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
                got.join("\n      ")
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "the scanner does not do what its rows say:\n  {}",
        bad.join("\n  ")
    );
}
