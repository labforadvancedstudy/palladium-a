//! `pdc --version` must be the version in `Cargo.toml`.
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
//! GOING RED AGAIN
//! Both call sites now read `env!("CARGO_PKG_VERSION")`, which cargo derives
//! from `Cargo.toml` at compile time. Re-introduce a literal — write
//! `#[command(version = "0.1.0-alpha")]` back into `src/cli.rs` — and
//! `version_flag_matches_cargo_package_version` fails, because the constant
//! this test compares against comes from the same manifest and cannot be
//! edited into agreement with a stale literal.

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

/// The version must not be a literal ANYWHERE in the CLI or the banner.
///
/// The assertion above compares one output string, so it would still pass if
/// someone hardcoded the *current* version — and that copy would drift at the
/// next bump exactly as the last one did. This test reads the two source files
/// and requires the value to be derived.
#[test]
fn no_source_file_hardcodes_the_version() {
    for (path, src) in [
        ("src/cli.rs", include_str!("../src/cli.rs")),
        ("src/main.rs", include_str!("../src/main.rs")),
    ] {
        // Split the needle so this test file does not match its own scan if it
        // is ever folded into the sources it reads.
        let derived = concat!("env!(\"CARGO", "_PKG_VERSION\")");
        assert!(
            src.contains(derived),
            "{} no longer derives the version from Cargo.toml",
            path
        );

        // The literal that was actually there, plus the current one: a bump
        // that re-hardcodes the value must fail too, not just a stale copy.
        for literal in ["0.1.0-alpha", "v0.1-alpha", EXPECTED] {
            let quoted = format!("\"{}\"", literal);
            assert!(
                !src.contains(&quoted),
                "{} hardcodes the version as {} — use env!(\"CARGO_PKG_VERSION\") so it cannot drift",
                path,
                quoted
            );
        }
    }
}
