//! pdc must not discard what gcc told it.
//!
//! THE DEFECT, MEASURED ON `main` BEFORE THE FIX (fb12f6f)
//!
//! ```text
//! fn inner(s: &String) -> i64 { string_len(s) }
//! fn outer(s: &String) -> i64 { inner(s) }
//! fn main() { let t: String = "abcd"; print_int(outer(&t)); }
//! ```
//!
//! ```text
//! $ pdc compile B3.pd -o B3    ->  "Created executable"   (no diagnostic)
//! $ ./build_output/B3; echo $? ->  (no output)  139       <- SIGSEGV
//! ```
//!
//! `compile_file` in `src/main.rs` read `gcc_output.stderr` only inside
//! `if !gcc_output.status.success()`. A warning leaves the status 0, so gcc's
//! diagnosis was read into a buffer and dropped:
//!
//! ```text
//! build_output/B3.c:279:18: warning: incompatible pointer types passing
//!   'const char *' to parameter of type 'const char **'; take the address
//!   with & [-Wincompatible-pointer-types]
//! ```
//!
//! THE SAME `if` LIED THE OTHER WAY TOO
//!
//! `exit != 0` was flattened into one string, `gcc compilation failed`. A gcc
//! that rejected our C, a gcc killed by the OOM killer, and a gcc that is not
//! installed all produced that marker, and a sibling gate reads the marker to
//! conclude "pdc accepted the source and gcc refused the C it emitted — a
//! defect in this compiler". Two of those three are the machine, not the
//! compiler. So the outcome is now structured, and it leaves the process
//! through the EXIT CODE, which no fixture text can forge — unlike a marker in
//! stderr, which gcc will happily echo back out of the generated C.
//!
//! WHAT THIS FILE HAS TO PROVE
//!
//! "The check exists" is not the claim. The claim is that it DISCRIMINATES, and
//! a check that refuses everything, or that fires off the exit status, would
//! satisfy a one-test suite. So:
//!
//! 1. `the_type_confusion_is_now_refused`   — B3 no longer produces a binary.
//! 2. `an_ordinary_program_still_runs`      — the accept side, run to a number.
//! 3. `gcc_giving_up_is_unchanged`          — the nonzero-exit path, verbatim.
//! 4. `the_refusal_reads_stderr_not_status` — gcc EXITED 0 on the C in (1).
//! 5. `a_killed_gcc_is_not_a_rejection`     — case 2 is not reportable as case 1.
//! 6. `a_missing_gcc_is_not_a_rejection`    — the other half of case 2.
//! 7. `the_outcomes_are_distinct_codes`     — and a shell can tell them apart.
//! 8. `a_localized_gcc_still_fires`        — the escalation is not an English
//!    feature, and a `LANG` on a developer's box cannot silently turn it off.
//! 9. `every_shape_of_ill_typed_c_is_fatal`— the fatal list is derived from the
//!    PROPERTY by asking the real toolchain, not from what somebody recalled.
//! 10. `every_gcc_invocation_goes_through_link` — all six discard sites, as a
//!     source fact rather than a claim in a commit message.
//! 11. `pdc_run_agrees_with_pdc_compile`   — and does not report success for a
//!     program that died.
//!
//! (4) and (5) are the whole change, one per direction of the old lie. Without
//! (4) the suite cannot tell this fix from the bug: every other assertion here
//! is also satisfied by an implementation that passes
//! `-Werror=incompatible-pointer-types` and keeps reading only the status.
//! Without (5) the structure is decoration: a gate would still be free to
//! certify a killed gcc as a codegen defect.
//!
//! NOT IN SCOPE: the `&T` forwarding defect that makes codegen emit that C. B3
//! failing to compile is the intended outcome of this change, not a regression.

use palladium::linker::{
    self, LinkError, OptLevel, EXIT_BACKEND_ILL_TYPED, EXIT_BACKEND_REJECT, EXIT_GCC_UNEXPLAINED,
    EXIT_TOOLCHAIN,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

/// The three-line program from the module comment.
const TYPE_CONFUSING_SOURCE: &str = r#"fn inner(s: &String) -> i64 { string_len(s) }
fn outer(s: &String) -> i64 { inner(s) }
fn main() { let t: String = "abcd"; print_int(outer(&t)); }
"#;

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// What `pdc compile <file> -o <name>` did.
struct PdcRun {
    ok: bool,
    /// pdc's exit status. `None` only if pdc itself was signalled, which is a
    /// harness failure and is asserted against rather than interpreted.
    code: Option<i32>,
    log: String,
    /// `build_output/<name>`, whether or not it exists.
    binary: PathBuf,
    /// The generated C, whether or not it exists.
    c_file: PathBuf,
}

/// Drive the real `pdc` binary, because the discard being fixed lives in
/// `src/main.rs` and no library call reaches it. `CARGO_BIN_EXE_pdc` is the
/// binary this test run built, so the wiring is what is under test and not a
/// stale install.
fn pdc_compile(source: &str, stem: &str) -> PdcRun {
    pdc_compile_with_path(source, stem, None)
}

/// As `pdc_compile`, but with `PATH` replaced so that `gcc` resolves to
/// something this test wrote.
///
/// The environment is set on the CHILD, never on this process: the test binary
/// runs its tests on threads, and a `set_var("PATH", ...)` here would be a race
/// with every other test that spawns anything.
fn pdc_compile_with_path(source: &str, stem: &str, path: Option<&Path>) -> PdcRun {
    match path {
        Some(p) => pdc_compile_with_env(source, stem, &[("PATH", &p.to_string_lossy())]),
        None => pdc_compile_with_env(source, stem, &[]),
    }
}

/// As `pdc_compile`, with arbitrary environment on the child.
///
/// Needed for the locale control: proving pdc does not INHERIT a hostile
/// `LC_ALL` means setting one, and setting it on this process would race every
/// other test in the binary.
fn pdc_compile_with_env(source: &str, stem: &str, env: &[(&str, &str)]) -> PdcRun {
    let dir = TempDir::new().expect("tempdir");
    let src = dir.path().join(format!("{}.pd", stem));
    fs::write(&src, source).expect("write source");

    let binary = repo_root().join("build_output").join(stem);
    let _ = fs::remove_file(&binary);

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_pdc"));
    cmd.current_dir(repo_root())
        .arg("compile")
        .arg(&src)
        .arg("-o")
        .arg(stem);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd.output().expect("run pdc");

    PdcRun {
        ok: out.status.success(),
        code: out.status.code(),
        log: format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
        binary,
        c_file: repo_root().join("build_output").join(format!("{}.c", stem)),
    }
}

// ---------------------------------------------------------------------------
// 1. the refusal
// ---------------------------------------------------------------------------

/// The C that segfaults is refused, and the refusal names the compiler.
///
/// Asserts on the BINARY as well as the exit code: reporting an error while
/// leaving a runnable miscompile on disk is the same defect one step later —
/// the next `./build_output/x` would run exactly what pdc just refused to ship.
#[test]
fn the_type_confusion_is_now_refused() {
    let run = pdc_compile(TYPE_CONFUSING_SOURCE, "linkdiag_confusion");

    assert!(
        !run.ok,
        "pdc accepted a program whose emitted C confuses pointer types; the \
         binary it produced segfaults.\n{}",
        run.log
    );
    assert!(
        run.log.contains("internal compiler error"),
        "the refusal does not say the defect is the compiler's — a Palladium \
         programmer cannot act on a raw gcc dump about C they never wrote.\n{}",
        run.log
    );
    assert!(
        run.log.contains("-Wincompatible-pointer-types"),
        "the refusal does not carry the gcc diagnostic it rests on.\n{}",
        run.log
    );
    assert!(
        !run.log.contains("Created executable"),
        "pdc still announced an executable.\n{}",
        run.log
    );
    assert!(
        !run.binary.exists(),
        "pdc reported an error but left {} on disk",
        run.binary.display()
    );
    assert_eq!(
        run.code,
        Some(EXIT_BACKEND_ILL_TYPED),
        "a gate reading $? cannot tell this from a gcc that rejected the C, and \
         they are different accusations.\n{}",
        run.log
    );
}

// ---------------------------------------------------------------------------
// 2. the accept side
// ---------------------------------------------------------------------------

/// An ordinary program compiles, links, RUNS, and prints the right number.
///
/// Run to a value rather than to exit 0: the change being guarded here is a new
/// refusal, and the failure mode of a refusal is refusing valid programs. It
/// also proves the prelude's `-Wincompatible-pointer-types-discards-qualifiers`
/// warning — which is in every compile in this tree — did not become fatal.
#[test]
fn an_ordinary_program_still_runs() {
    // No `&` anywhere, and the reason is measured rather than assumed.
    //
    // AN EARLIER VERSION OF THIS COMMENT WAS FALSE. It said `string_len(&s)` is
    // "the spelling the tutorials use". It is not: `grep -rn 'string_len(&'`
    // over every .pd, doc and example in this tree returns ZERO hits, and
    // docs/user-guide/tutorial.md passes the String by value —
    // `string_len(joined)`, `string_len(s)`, `string_len(text)`. The claim was
    // inferred from a probe I wrote myself and then stated about the corpus, in
    // a file whose whole argument is that claims must be measured.
    //
    // What IS measured: the three probes in the branch report (`string_len(&t)`
    // on a local, and two-hop `&String` forwarding) do emit
    // `-Wincompatible-pointer-types`. So a control written with `&` would be
    // exercising the open `&T` forwarding defect instead of the accept side —
    // the right conclusion, reached from evidence that exists.
    let run = pdc_compile(
        "fn add(a: i64, b: i64) -> i64 { return a + b; }\n\
         fn main() { print_int(add(40, 2)); }\n",
        "linkdiag_ordinary",
    );

    assert!(run.ok, "pdc refused an ordinary program:\n{}", run.log);
    assert!(
        run.binary.exists(),
        "pdc reported success but produced no executable\n{}",
        run.log
    );

    let exec = Command::new(&run.binary).output().expect("run program");
    assert!(
        exec.status.success(),
        "program exited {:?}",
        exec.status.code()
    );
    assert_eq!(String::from_utf8_lossy(&exec.stdout).trim(), "42");

    // The control for the exclusion: gcc really did warn on this compile, and
    // the compile really was accepted. If the prelude bug is ever fixed this
    // assertion stops proving anything, so it says so rather than flipping.
    let stderr = gcc_stderr_for(&run.c_file);
    if stderr.contains("-Wincompatible-pointer-types-discards-qualifiers") {
        assert!(
            linker::fatal_diagnostics(&stderr, &run.c_file).is_empty(),
            "the prelude warning present in every compile was escalated"
        );
    } else {
        eprintln!(
            "note: the prelude no longer emits discards-qualifiers; the \
             exclusion in NON_FATAL_DIAGNOSTIC_TAGS can be revisited"
        );
    }
}

// ---------------------------------------------------------------------------
// 3. gcc giving up: unchanged
// ---------------------------------------------------------------------------

/// A nonzero gcc exit still produces `gcc compilation failed:` and gcc's own
/// text, byte-for-byte as before.
///
/// `scripts/conformance.sh` classifies the failing stage by grepping the
/// compiler log for that exact phrase, so it is a contract with a gate and not
/// only with a reader. Driven through a handcrafted broken C file rather than a
/// Palladium program: which .pd sources currently make gcc give up is a moving
/// property of open codegen defects, and this test is about the linker's
/// behaviour, which is not.
#[test]
fn gcc_giving_up_is_unchanged() {
    let dir = TempDir::new().expect("tempdir");
    let c_file = dir.path().join("broken.c");
    let exe = dir.path().join("broken");
    fs::write(&c_file, "int main(void) { this is not C ) }\n").expect("write c");

    let err = linker::link(&c_file, &exe, OptLevel::Default)
        .expect_err("gcc must reject a file that is not C");

    assert!(
        matches!(err, LinkError::GccRejected(_)),
        "a gcc that GAVE UP was classified as something else: {:?}",
        err
    );
    let msg = err.to_string();
    assert!(
        msg.starts_with("gcc compilation failed:\n"),
        "the phrase scripts/conformance.sh greps for is gone: {}",
        msg
    );
    assert!(
        !msg.contains("internal compiler error"),
        "a user's broken build was reported as a compiler defect: {}",
        msg
    );
    assert!(!exe.exists(), "gcc failed but an executable exists");
    assert_eq!(
        err.exit_code(),
        EXIT_BACKEND_REJECT,
        "the one outcome that supports 'the backend emitted C that will not \
         compile' must be the one a gate can read: {}",
        msg
    );
}

// ---------------------------------------------------------------------------
// 4. the point of the change: stderr, not status
// ---------------------------------------------------------------------------

/// gcc EXITED 0 on the C that pdc refuses.
///
/// This is the control that separates the fix from the bug, and from the other
/// plausible fix. If the refusal came from `-Werror=...` on the command line,
/// or from any reading of the exit status, gcc's status here would be nonzero
/// and this test would fail. It asserts three things about ONE compile:
/// gcc succeeded, gcc still said something, and pdc refused anyway.
#[test]
fn the_refusal_reads_stderr_not_status() {
    let run = pdc_compile(TYPE_CONFUSING_SOURCE, "linkdiag_exit0");
    assert!(
        run.c_file.exists(),
        "codegen produced no C, so there is nothing to ask gcc about\n{}",
        run.log
    );

    let runtime_dir = palladium::runtime_paths::runtime_dir().expect("runtime dir");
    let out = Command::new("gcc")
        .arg("-c")
        .arg(&run.c_file)
        .arg("-o")
        .arg(
            TempDir::new()
                .expect("tempdir")
                .keep()
                .join("linkdiag_exit0.o"),
        )
        .arg("-I")
        .arg(&runtime_dir)
        .output()
        .expect("run gcc");

    assert!(
        out.status.success(),
        "gcc did NOT exit 0 on this C, so this control proves nothing about \
         reading stderr. Either the toolchain now errors on the tag (gcc 14+ \
         does) or a -Werror flag was added; in both cases the stderr path needs \
         a different exit-0 witness.\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    let fatal = linker::fatal_diagnostics(&stderr, &run.c_file);
    assert!(
        !fatal.is_empty(),
        "gcc exited 0 and said nothing this change can act on:\n{}",
        stderr
    );
    assert!(
        !run.ok,
        "gcc exited 0 with a fatal diagnostic in stderr and pdc accepted the \
         program anyway — the status is still the only thing being read.\n{}",
        run.log
    );
}

// ---------------------------------------------------------------------------
// 5-7. the other direction of the same lie: WHAT gcc did, not only that it failed
// ---------------------------------------------------------------------------

/// A tiny program that reaches the link stage. Any valid program does; this one
/// is chosen for having nothing else that could fail.
const TRIVIAL_SOURCE: &str = "fn main() { print_int(1); }\n";

/// A program that compiles, links, runs, produces output, and THEN fails.
///
/// `panic` lowers to `__pd_panic`, which prints and calls `abort()`
/// (runtime/pd_prelude.h), so the child dies on a signal rather than exiting —
/// the case the old `status.code().unwrap_or(-1)` printed as `-1` and then
/// returned `Ok(())` for.
const CHILD_FAILS_SOURCE: &str = "fn main() { print_int(7); panic(\"deliberate\"); }\n";

/// Shell that sets `$c` to the first `.c` on gcc's command line.
///
/// The shims must name the REAL translation unit: since diagnostics are
/// filtered to the file under compilation (so a defect in the C runtime cannot
/// condemn the user's program), a shim that invents a path like `x.c` is
/// correctly classified as somebody else's problem and proves nothing.
const SHIM_FIND_TU: &str = "#!/bin/sh\nc=\"\"\nfor a in \"$@\"; do case \"$a\" in *.c) if [ -z \"$c\" ]; then c=\"$a\"; fi;; esac; done\n";

/// Put an executable `gcc` of our own first on `PATH`, and hand back that PATH.
///
/// The only way to observe "gcc was killed" without killing a real compilation:
/// the shim IS gcc as far as pdc is concerned, and it can die however the test
/// needs. The real gcc stays reachable behind it so nothing else breaks.
fn shim_path(dir: &Path, script: &str) -> String {
    let gcc = dir.join("gcc");
    fs::write(&gcc, script).expect("write shim");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&gcc, fs::Permissions::from_mode(0o755)).expect("chmod shim");
    }
    format!(
        "{}:{}",
        dir.display(),
        std::env::var("PATH").unwrap_or_default()
    )
}

/// CASE 2 IS NOT CASE 1: a gcc that was killed is not a gcc that said no.
///
/// This is the control the structured outcome exists for. Before it, a
/// signalled gcc arrived at the gate as `gcc compilation failed` with exit 1 —
/// indistinguishable from a genuine rejection — and a conformance verdict built
/// on that marker would certify "the backend emitted C that will not compile"
/// about a compilation that never happened.
///
/// The mechanism under test is `ExitStatus::signal()`, not a reading of stderr:
/// the shim below prints NOTHING before dying, so any implementation that
/// classified by looking at the text would have nothing to look at.
#[test]
#[cfg(unix)]
fn a_killed_gcc_is_not_a_rejection() {
    let dir = TempDir::new().expect("tempdir");
    let path = shim_path(dir.path(), "#!/bin/sh\nkill -9 $$\n");
    let run = pdc_compile_with_path(TRIVIAL_SOURCE, "linkdiag_killed", Some(Path::new(&path)));

    assert!(!run.ok, "a killed gcc was reported as success\n{}", run.log);
    assert_eq!(
        run.code,
        Some(EXIT_TOOLCHAIN),
        "a killed gcc is reported with the code that means 'the backend emitted \
         C that will not compile'. It means nothing of the kind — gcc never \
         reached a verdict.\n{}",
        run.log
    );
    assert_ne!(
        run.code,
        Some(EXIT_BACKEND_REJECT),
        "case 2 is reportable as case 1, which is the whole defect\n{}",
        run.log
    );
    assert!(
        !run.log.contains("gcc compilation failed"),
        "the marker a gate reads as 'gcc refused our C' was printed for a gcc \
         that was killed\n{}",
        run.log
    );
    assert!(
        run.log.contains("killed by signal 9"),
        "the report does not say what actually happened\n{}",
        run.log
    );
}

/// The other half of case 2: gcc is not there at all.
///
/// Same verdict, different sentence. A `PATH` with no gcc on it makes the spawn
/// fail before any compilation exists to have a verdict about.
#[test]
#[cfg(unix)]
fn a_missing_gcc_is_not_a_rejection() {
    let empty = TempDir::new().expect("tempdir");
    let run = pdc_compile_with_path(TRIVIAL_SOURCE, "linkdiag_nogcc", Some(empty.path()));

    assert_eq!(
        run.code,
        Some(EXIT_TOOLCHAIN),
        "a machine with no gcc reports a defect in the compiler\n{}",
        run.log
    );
    assert!(
        !run.log.contains("gcc compilation failed"),
        "gcc never ran, and the log says it refused something\n{}",
        run.log
    );
}

/// A gcc that really did reject the C is still case 1 — through the shim, so
/// the discrimination is measured against the SAME mechanism as cases 2 and 3
/// rather than against a different code path.
///
/// Also the negative control for `a_killed_gcc_is_not_a_rejection`: without it,
/// an implementation that answered `EXIT_TOOLCHAIN` for every failure would
/// pass every assertion in that test.
#[test]
#[cfg(unix)]
fn a_gcc_that_says_no_is_still_a_rejection() {
    let dir = TempDir::new().expect("tempdir");
    let path = shim_path(
        dir.path(),
        &format!(
            "{}echo \"$c:1:1: error: gcc says no\" >&2\nexit 1\n",
            SHIM_FIND_TU
        ),
    );
    let run = pdc_compile_with_path(TRIVIAL_SOURCE, "linkdiag_reject", Some(Path::new(&path)));

    assert_eq!(
        run.code,
        Some(EXIT_BACKEND_REJECT),
        "a genuine gcc rejection is no longer reported as one\n{}",
        run.log
    );
    assert!(
        run.log.contains("gcc compilation failed"),
        "the marker scripts/conformance.sh matches on is gone\n{}",
        run.log
    );
    assert!(
        run.log.contains("gcc says no"),
        "gcc's own text was dropped\n{}",
        run.log
    );
}

/// Three outcomes, three codes, and none of them collides with a code that
/// already means something else.
///
/// The numbers are the interface: a shell gate reads `$?` and has nothing else.
/// Pinning them here is what makes renumbering a deliberate act with a failing
/// test attached, rather than a silent reclassification of every fixture.
#[test]
fn the_outcomes_are_distinct_codes() {
    let codes = [
        EXIT_BACKEND_REJECT,
        EXIT_BACKEND_ILL_TYPED,
        EXIT_TOOLCHAIN,
        EXIT_GCC_UNEXPLAINED,
    ];
    for (i, a) in codes.iter().enumerate() {
        for b in &codes[i + 1..] {
            assert_ne!(a, b, "two outcomes share an exit code");
        }
        // 0 is success; 1 is pdc's existing "something went wrong", which every
        // front-end refusal already uses; 2 is clap's usage error AND the status
        // `make` reports for any failing recipe.
        assert!(*a > 2, "exit code {} is already spoken for", a);
    }

    // The variants agree with the constants. Written out rather than derived, so
    // that moving a variant into the wrong class is a failing test.
    assert_eq!(
        LinkError::Toolchain(String::new()).exit_code(),
        EXIT_TOOLCHAIN
    );
    assert_eq!(
        LinkError::GccAbnormal(String::new()).exit_code(),
        EXIT_TOOLCHAIN
    );
    assert_eq!(
        LinkError::GccRejected(String::new()).exit_code(),
        EXIT_BACKEND_REJECT
    );
    assert_eq!(
        LinkError::IllTypedC(String::new()).exit_code(),
        EXIT_BACKEND_ILL_TYPED
    );
    assert_eq!(
        LinkError::GccUnexplained(String::new()).exit_code(),
        EXIT_GCC_UNEXPLAINED
    );
}

// ---------------------------------------------------------------------------
// 8-12. what the second round of review found
// ---------------------------------------------------------------------------

/// A NONZERO gcc THAT NEVER MENTIONED OUR FILE IS NOT A REJECTION OF IT.
///
/// The overclaim this closes: the first version mapped every nonzero exit onto
/// a code documented as "gcc rejected the translation unit this compiler
/// emitted", while the only thing observed was the status. A full disk, an
/// unwritable output path, a missing assembler, an internal error in gcc —
/// nonzero, none of them about our C. Structuring that signal did not make it
/// truer; it made a false accusation machine-readable for a gate being built to
/// consume it.
#[test]
#[cfg(unix)]
fn a_nonzero_gcc_that_never_named_our_file_is_not_a_rejection() {
    let dir = TempDir::new().expect("tempdir");
    // The shape of a disk or permission failure: gcc says something real, exits
    // nonzero, and never diagnoses the translation unit.
    let path = shim_path(
        dir.path(),
        "#!/bin/sh\necho 'gcc: fatal error: cannot write output: No space left on device' >&2\nexit 1\n",
    );
    let run = pdc_compile_with_path(
        TRIVIAL_SOURCE,
        "linkdiag_unexplained",
        Some(Path::new(&path)),
    );

    assert_eq!(
        run.code,
        Some(EXIT_GCC_UNEXPLAINED),
        "a failure nobody attributed to our C was reported with the code that \
         means 'this compiler emitted C that will not compile'\n{}",
        run.log
    );
    assert!(
        !run.log.contains("gcc compilation failed"),
        "the marker a gate reads as 'gcc refused our C' was printed for a \
         failure that never named it\n{}",
        run.log
    );
    assert!(
        run.log.contains("No space left on device"),
        "gcc's own text was dropped\n{}",
        run.log
    );
}

/// THE ESCALATION IS NOT AN ENGLISH LANGUAGE FEATURE.
///
/// `diagnostic_tags` matches the literal `": warning: "`. GNU gcc localizes its
/// diagnostic prose and does NOT localize the `[-Wname]` tag, so on a box with
/// `LANG=ja_JP.UTF-8` the parser would match nothing, `link` would return `Ok`,
/// and the segfaulting binary would ship with pdc printing `Created
/// executable` — byte-identical to the original bug, re-enabled by an
/// environment variable, with no signal that a check had been skipped. A
/// fail-OPEN in the one place this branch exists to make fail closed.
///
/// The fix is `LC_ALL=C` on the child. The shim answers in German unless it is
/// told otherwise, so the test does not need a localized toolchain installed,
/// and pdc is run under a hostile ambient locale so that inheriting it would
/// fail here.
#[test]
#[cfg(unix)]
fn a_localized_gcc_still_fires() {
    let dir = TempDir::new().expect("tempdir");
    let path = shim_path(
        dir.path(),
        &format!(
            "{}if [ \"$LC_ALL\" = C ]; then\n\
             echo \"$c:1:1: warning: incompatible pointer types [-Wincompatible-pointer-types]\" >&2\n\
             else\n\
             echo \"$c:1:1: Warnung: inkompatible Zeigertypen [-Wincompatible-pointer-types]\" >&2\n\
             fi\nexit 0\n",
            SHIM_FIND_TU
        ),
    );
    let run = pdc_compile_with_env(
        TYPE_CONFUSING_SOURCE,
        "linkdiag_locale",
        &[
            ("PATH", path.as_str()),
            ("LC_ALL", "de_DE.UTF-8"),
            ("LANG", "de_DE.UTF-8"),
        ],
    );

    assert_eq!(
        run.code,
        Some(EXIT_BACKEND_ILL_TYPED),
        "under an ambient LC_ALL=de_DE the escalation did not fire. gcc's tag \
         is not localized but its prose is, so the check silently switched \
         itself off — the original defect, re-enabled by an environment \
         variable.\n{}",
        run.log
    );
}

/// THE FATAL LIST IS DERIVED FROM THE PROPERTY, BY THE TOOLCHAIN.
///
/// `FATAL_DIAGNOSTIC_TAGS` states a rule: a tag belongs there when no Palladium
/// program can ask for the C gcc is objecting to. A hand-written list does not
/// enforce its own rule — the first version had ONE member and a review found
/// three more tags satisfying it verbatim, recorded nowhere. `-Wint-conversion`
/// is the expensive one: CLAUDE.md says a gcc error is the only thing stopping
/// six `file_*` builtins from dereferencing an integer as a `FILE*`, and that
/// diagnostic is only an ERROR on newer clang — on GNU gcc 13 and earlier,
/// which is what this repo's Linux CI runs, it is a warning that exits 0.
///
/// So the list is checked against the real compiler, one snippet per shape.
#[test]
fn every_shape_of_ill_typed_c_is_fatal() {
    let shapes: &[(&str, &str)] = &[
        (
            "a pointer of the wrong depth",
            "void f(char **p); void g(char *p){ f(p); }",
        ),
        (
            "an integer used as a pointer",
            "void f(char *p); void g(long n){ f(n); }",
        ),
        (
            "a pointer used as an integer",
            "void f(long n); void g(char *p){ f(p); }",
        ),
        (
            "a function pointer of the wrong signature",
            "void f(int (*p)(char)); int h(long); void g(void){ f(h); }",
        ),
        (
            "a call to a function that was never declared",
            "long g(void){ return nosuchfn(); }",
        ),
    ];

    let dir = TempDir::new().expect("tempdir");
    for (what, src) in shapes {
        let c = dir.path().join("shape.c");
        fs::write(&c, src).expect("write c");
        let out = Command::new("gcc")
            .env("LC_ALL", "C")
            .arg("-c")
            .arg(&c)
            .arg("-o")
            .arg(dir.path().join("shape.o"))
            .output()
            .expect("run gcc");
        let stderr = String::from_utf8_lossy(&out.stderr);

        let tags: Vec<&str> = stderr
            .lines()
            .filter(|l| l.contains(": warning: ") || l.contains(": error: "))
            .filter_map(|l| l.trim_end().strip_suffix(']'))
            .filter_map(|l| l.rfind("[-W").map(|i| &l[i + 1..]))
            .flat_map(|t| t.split(','))
            .collect();

        assert!(
            !tags.is_empty(),
            "this toolchain says nothing about `{}` ({}), so the shape cannot \
             be checked here:\n{}",
            what,
            src,
            stderr
        );
        assert!(
            tags.iter()
                .any(|t| linker::FATAL_DIAGNOSTIC_TAGS.contains(t)),
            "`{}` is C that no Palladium program can ask for, and the tag this \
             toolchain gives it ({:?}) is in neither FATAL_DIAGNOSTIC_TAGS nor \
             any recorded decision at all. That is the membership rule going \
             unenforced.\nsource: {}\n{}",
            what,
            tags,
            src,
            stderr
        );
    }
}

/// ALL SIX DISCARD SITES, AS A SOURCE FACT.
///
/// The defect was one policy copied into six places, so "they were all
/// migrated" has to be checked rather than asserted in a commit message. It is
/// also what makes `LC_ALL=C` a property of the compiler rather than of one
/// call site: `link_command` cannot carry the env itself, because its body is
/// content-pinned in docs/citation-pins.tsv (a file this branch may not edit),
/// so the guarantee is "every production invocation goes through `link`".
#[test]
fn every_gcc_invocation_goes_through_link() {
    let mut offenders = Vec::new();
    let mut stack = vec![repo_root().join("src")];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("read src") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            // linker.rs defines it and its own unit tests assert on the command
            // it builds; the llvm backend's uses are inside `#[cfg(test)]`.
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "linker.rs" || name == "llvm_text_backend.rs" {
                continue;
            }
            let text = fs::read_to_string(&path).expect("read rs");
            for (i, line) in text.lines().enumerate() {
                if line.contains("link_command(") && !line.trim_start().starts_with("//") {
                    offenders.push(format!("{}:{}: {}", path.display(), i + 1, line.trim()));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "these call sites build the gcc command themselves instead of going \
         through `linker::link`, so each has its own copy of the status/stderr \
         policy — the defect this branch exists to remove, and how `pdc run` \
         came to disagree with `pdc compile`:\n{}",
        offenders.join("\n")
    );
}

/// `pdc run` AND `pdc compile` MUST NOT DISAGREE ABOUT THE SAME SOURCE.
///
/// Measured on this branch before the migration: `pdc compile B3.pd` refused
/// (exit 4, no binary) while `pdc run B3.pd` built it, ran it, printed
/// `Program exited with code: -1`, and exited 0. Two adjacent commands,
/// opposite verdicts, and the one that actually executed the miscompiled
/// binary was the one reporting success.
#[test]
fn pdc_run_agrees_with_pdc_compile() {
    let dir = TempDir::new().expect("tempdir");
    let src = dir.path().join("linkdiag_runagree.pd");
    fs::write(&src, TYPE_CONFUSING_SOURCE).expect("write source");

    let out = Command::new(env!("CARGO_BIN_EXE_pdc"))
        .current_dir(repo_root())
        .arg("run")
        .arg(&src)
        .output()
        .expect("run pdc");
    let log = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    assert!(
        !out.status.success(),
        "`pdc run` accepted and EXECUTED a program `pdc compile` refuses\n{}",
        log
    );
    assert_eq!(
        out.status.code(),
        Some(EXIT_BACKEND_ILL_TYPED),
        "`pdc run` and `pdc compile` disagree about the same source\n{}",
        log
    );
    assert!(!log.contains("Program completed successfully"), "{}", log);
}

/// A launcher may not report success for a program that died.
///
/// The other half of the same site: `compile_and_run` used to `println!` the
/// child's exit code and then return `Ok(())`.
#[test]
fn pdc_run_propagates_a_failing_child() {
    let dir = TempDir::new().expect("tempdir");
    let src = dir.path().join("linkdiag_deadchild.pd");
    // A program that runs correctly and then fails. `RunOutcome::exit_code`
    // documents the boundary this exercises: once the program starts, the
    // status is the program's.
    fs::write(&src, CHILD_FAILS_SOURCE).expect("write source");

    let out = Command::new(env!("CARGO_BIN_EXE_pdc"))
        .current_dir(repo_root())
        .arg("run")
        .arg(&src)
        .output()
        .expect("run pdc");
    let log = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    assert!(
        log.contains('7'),
        "the program did not run, so this proves nothing about its status\n{}",
        log
    );
    assert!(
        !out.status.success(),
        "`pdc run` exited 0 for a program that failed\n{}",
        log
    );
}

/// gcc's stderr for one generated C file, compiled but not linked.
fn gcc_stderr_for(c_file: &Path) -> String {
    let runtime_dir = palladium::runtime_paths::runtime_dir().expect("runtime dir");
    let out = Command::new("gcc")
        .arg("-fsyntax-only")
        .arg(c_file)
        .arg("-I")
        .arg(&runtime_dir)
        .output()
        .expect("run gcc");
    String::from_utf8_lossy(&out.stderr).into_owned()
}

// ---------------------------------------------------------------------------
// The consumer this change has NOT been reconciled with
// ---------------------------------------------------------------------------

/// `scripts/gate_probe.py` treats any pdc exit outside `(0, 1)` as a MALFUNCTION.
///
/// WHY THIS IS A DEBT AND NOT A BUG IN THIS BRANCH. `classify(r, reject_codes=(1,))`
/// is reached from `scripts/stdlib-gate.sh` at six call sites and from
/// `scripts/check_doc_evidence.py`, so `make stdlib-gate` and `make check-docs`
/// are both exposed. A `Malfunction` WITHHOLDS the producer's output, so the
/// first fixture to exit 3 or 4 would give an operator strictly less than the
/// same fixture gave before this change.
///
/// THE RIGHT SHAPE, and it is not mine to apply: `reject_codes=(1, 3, 4)` at the
/// two pdc call sites. Exits 3 and 4 ARE conclusions — gcc reached a verdict, or
/// pdc reached one about gcc's output — which is exactly the `Concluded` half of
/// the distinction `gate_probe` already draws. Exit 5 (and 6) must STAY a
/// malfunction: an absent, killed, or unexplained toolchain establishes nothing,
/// which is the same argument, in the same words, as this module's own reason
/// for not calling those a backend rejection.
///
/// WHY IT IS UNREACHED TODAY AND THAT IS NOT A DEFENCE: no .pd in the corpus
/// reaches a gcc-stage failure (measured: 191 files, 0 hits for `gcc
/// compilation failed`), so no gate exits 3-6 today. "Unreached today" is a
/// measurement with a shelf life, and writing it in a commit message is how it
/// stops being anybody's. `scripts/gate_probe.py` belongs to another agent
/// right now, so this row holds the obligation instead.
#[test]
#[ignore = "XFAIL: scripts/gate_probe.py classify() pins reject_codes=(1,), so a pdc exit of 3 (backend reject) or 4 (backend emitted ill-typed C) is reported as MALFUNCTION — and a Malfunction withholds the output the operator needs. Fix is reject_codes=(1, 3, 4) at the two pdc call sites in stdlib-gate.sh and check_doc_evidence.py; exits 5 and 6 must remain malfunctions. Not applied here: scripts/gate_probe.py is reserved for another agent on a concurrent branch. (owned by unscheduled: it is a cross-branch reconciliation, and no milestone owns the gate harness)"]
fn the_gate_probe_accepts_the_codes_a_backend_failure_now_reports() {
    let probe = repo_root().join("scripts/gate_probe.py");
    let text = fs::read_to_string(&probe)
        .expect("scripts/gate_probe.py is the consumer this row is about");
    assert!(
        text.contains("reject_codes=(1, 3, 4)"),
        "gate_probe.classify still pins reject_codes=(1,), so pdc exit 3 and 4 \
         are reported as MALFUNCTION with the output withheld"
    );
}
