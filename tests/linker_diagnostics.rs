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
//! "The check exists" is not the claim. The claim is that it DISCRIMINATES. A
//! check that refuses everything, or that fires off the exit status, or that
//! lets the program under test choose the verdict, would satisfy a suite that
//! only asserted existence. So each control below is paired with the mistake it
//! would let through, and the ones that matter most have been fault-injected:
//! reverting the defence turns that control RED and leaves the others green.
//!
//! REFUSAL AND ACCEPTANCE
//!   `forwarding_a_shared_reference_compiles_links_and_runs`
//!                                           the REAL toolchain, on real emitted
//!                                           C, run to a value.
//!   `a_fatal_backend_diagnostic_refuses_and_blames_the_compiler`
//!                                           the classifier, on a SHIM.
//!   `an_ordinary_program_still_runs`        the accept side, run to a number.
//!   `ill_typed_c_in_the_runtime_is_refused_and_says_whose_it_is`
//!                                           ownership changes the sentence,
//!                                           never the verdict.
//!
//! WHAT THE SHIM TESTS DO AND DO NOT SAY. Every control tagged SHIM below feeds
//! `pdc` a canned stderr from a fake `gcc`. It therefore pins the READER — the
//! classification of a diagnostic, the exit code, the attribution, the wording —
//! and it says NOTHING about whether this compiler still emits C that would
//! provoke one. It cannot: the shim prints its line whatever the C says. The only
//! claims here about real generated C are the two run-to-a-value controls above,
//! which use the real toolchain.
//!
//! WHAT gcc DID, AS OPPOSED TO THAT IT FAILED
//!   `gcc_giving_up_is_unchanged`            the nonzero path, verbatim.
//!   `the_refusal_reads_stderr_not_status`   SHIM: a gcc that EXITS 0 while
//!                                           printing a fatal diagnostic.
//!   `a_killed_gcc_is_not_a_rejection`       case 2 is not case 1.
//!   `a_missing_gcc_is_not_a_rejection`      the other half of case 2.
//!   `a_gcc_that_says_no_is_still_a_rejection`  the negative control for both.
//!   `a_nonzero_gcc_that_never_named_our_file_is_not_a_rejection`
//!                                           nonzero is not an accusation.
//!
//! ATTRIBUTION — WHOSE FILE, AND WHO GOT TO SAY SO
//!   `a_runtime_diagnostic_is_not_charged_to_a_program_named_runtime`
//!   `a_runtime_error_does_not_make_a_program_named_runtime_a_backend_reject`
//!                                           `ends_with` is not `file_name()`.
//!   `an_echoed_source_line_cannot_forge_a_backend_reject`
//!                                           fixture text may not choose the
//!                                           exit code — the module's own
//!                                           central claim, turned on itself.
//!   `a_real_header_at_column_zero_still_attributes`
//!                                           and the anchor did not over-correct.
//!
//! POLICY
//!   `every_shape_of_ill_typed_c_is_fatal`   the fatal list is derived from the
//!                                           property by the real toolchain.
//!   `a_known_tag_does_not_hide_an_unknown_one_beside_it`
//!   `both_toolchains_spellings_of_a_promoted_tag_resolve`
//!   `each_fatal_class_explains_itself`      no borrowed causal stories.
//!   `a_localized_gcc_still_fires`           not an English-language feature.
//!
//! STRUCTURE AND ITS CONSUMERS
//!   `the_outcomes_are_distinct_codes`
//!   `every_gcc_invocation_goes_through_link`  all six sites, as a source fact.
//!   `pdc_run_agrees_with_pdc_compile`
//!   `pdc_run_propagates_a_failing_child`
//!
//! THE DEFECT THAT FIRST EXPOSED ALL THIS IS GONE. `&T` forwarding emitted a value
//! where a pointer was declared, and B3 could not compile; that is fixed, and
//! `forwarding_a_shared_reference_compiles_links_and_runs` now runs it to a value.
//! The mechanism controls outlived their subject, which is why they moved to shims:
//! a pin whose only witness is a live bug dies the day the bug is fixed.

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

/// THE PROGRAM THIS FILE WAS BUILT AROUND NOW WORKS, and that is the assertion.
///
/// It used to be the subject of a refusal: `outer` forwarding its `&String`
/// parameter to `inner` emitted `inner((*s))` — a dereference into a pointer
/// parameter — and gcc caught it, so pdc refused rather than ship C it had
/// mis-generated. su2 fixed the cause. The call site decided whether to take an
/// address from `param.mutable` alone, while the DECLARATION side had always used
/// `param.mutable || matches!(ty, Type::Reference { .. })`; the two now ask one
/// question, and a reference parameter is forwarded as the pointer it already is.
///
/// The strongest form of the transform: the exact program that was the refusal's
/// subject is now the fix's witness. It compiles, it links, it runs, and it prints
/// the length of "abcd". Anything less — asserting only that pdc exits 0 — would
/// pass on a compiler that emitted nothing at all.
#[test]
fn forwarding_a_shared_reference_compiles_links_and_runs() {
    let run = pdc_compile(TYPE_CONFUSING_SOURCE, "linkdiag_confusion");

    assert!(
        run.ok,
        "forwarding a `&T` parameter is refused again; su2's call-site predicate \
         has regressed.\n{}",
        run.log
    );
    assert!(
        !run.log.contains("internal compiler error"),
        "pdc still reports its own codegen as ill-typed for this program.\n{}",
        run.log
    );
    assert!(
        run.binary.exists(),
        "pdc reported success but left no executable at {}",
        run.binary.display()
    );

    let out = Command::new(&run.binary).output().expect("run the binary");
    assert!(
        out.status.success(),
        "the binary pdc produced did not run cleanly\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "4",
        "the forwarded reference did not survive to the callee: `string_len` \
         answered about the wrong thing"
    );
}

/// SHIM. THE CLASSIFIER, NOT THE EMITTER.
///
/// The test above used to carry these assertions, resting on a real codegen bug
/// as its subject. That is a bad place for a mechanism pin: the day the bug is
/// fixed — it now is — the pin goes with it, and the machinery that turns a fatal
/// backend diagnostic into a Palladium-level refusal loses its only end-to-end
/// witness. So the subject is a SHIM that exits 0 and prints the diagnostic shape
/// the real gcc printed.
///
/// WHAT THIS DOES NOT SAY, and what its old name `ill_typed_c_is_refused_and_
/// names_the_compiler` did say: nothing here shows that this compiler emits
/// ill-typed C. The shim prints its line regardless of what was compiled, so the
/// property pinned is the READING — refuse, blame the compiler rather than the
/// programmer, quote the diagnostic, leave no binary, and exit a code a gate can
/// tell apart from a gcc that rejected the C.
#[test]
#[cfg(unix)]
fn a_fatal_backend_diagnostic_refuses_and_blames_the_compiler() {
    let dir = TempDir::new().expect("tempdir");
    let path = shim_path(
        dir.path(),
        &format!(
            "{}echo \"$c:1:1: warning: passing argument 1 of 'f' from incompatible \
             pointer type [-Wincompatible-pointer-types]\" >&2\nexit 0\n",
            SHIM_FIND_TU
        ),
    );
    let run = pdc_compile_with_path(TRIVIAL_SOURCE, "linkdiag_illtyped", Some(Path::new(&path)));

    assert!(
        !run.ok,
        "the backend reported a fatal diagnostic and pdc shipped the program \
         anyway.\n{}",
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
        "the refusal does not carry the backend diagnostic it rests on.\n{}",
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
/// plausible fix. If the refusal came from `-Werror=...` on the command line, or
/// from any reading of the exit status, a gcc that exits 0 would be accepted and
/// this test would fail. It asserts two things about ONE compile: gcc SUCCEEDED,
/// and pdc refused anyway.
///
/// THE SUBJECT IS A SHIM, and used to be a real program whose C provoked an
/// exit-0 warning. That subject was the `&T` forwarding defect, which su2 fixed —
/// so the control had to be re-based or lost. A shim is the better base anyway:
/// the property being pinned is "the reader looks at stderr, not at `$?`", and a
/// gcc that exits 0 while printing a fatal diagnostic states that property
/// directly instead of depending on some current program still provoking one.
#[test]
#[cfg(unix)]
fn the_refusal_reads_stderr_not_status() {
    let dir = TempDir::new().expect("tempdir");
    let stderr_line = "warning: passing argument 1 of 'f' from incompatible pointer type \
                       [-Wincompatible-pointer-types]"
        .to_string();
    let path = shim_path(
        dir.path(),
        &format!("{}echo \"$c:1:1: {}\" >&2\nexit 0\n", SHIM_FIND_TU, &stderr_line),
    );
    let run = pdc_compile_with_path(TRIVIAL_SOURCE, "linkdiag_exit0", Some(Path::new(&path)));

    // The shim exits 0 BY CONSTRUCTION, which is the whole point: there is no
    // nonzero status anywhere for an implementation to have keyed on.
    assert!(
        !run.ok,
        "the shim exited 0 with a fatal diagnostic in stderr and pdc accepted the \
         program anyway — the status is still the only thing being read.\n{}",
        run.log
    );
    assert_eq!(
        run.code,
        Some(EXIT_BACKEND_ILL_TYPED),
        "a diagnostic read out of a successful gcc must still be classified as \
         ill-typed C, not as a rejection.\n{}",
        run.log
    );

    // And the classifier itself agrees, on the same text, in isolation.
    let captured = format!("{}:1:1: {}\n", run.c_file.display(), stderr_line);
    let fatal = linker::fatal_diagnostics(&captured, &run.c_file);
    assert!(
        !fatal.is_empty(),
        "`fatal_diagnostics` no longer treats an exit-0 incompatible-pointer \
         warning as fatal, so the end-to-end refusal above rests on nothing"
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
            // Exemptions are RELATIVE PATHS, not base names. Matching on the
            // base name would silently exempt a future `src/backend/linker.rs`
            // — a new file with its own copy of the policy, invisible to the
            // one check that exists to find exactly that.
            let rel = path.strip_prefix(repo_root()).unwrap_or(&path);
            let rel = rel.to_string_lossy().replace('\\', "/");
            // src/linker.rs defines it and its own unit tests assert on the
            // command it builds; the llvm backend's uses are inside
            // `#[cfg(test)]`.
            if rel == "src/linker.rs" || rel == "src/codegen/llvm_text_backend.rs" {
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
/// `Program exited with code: -1`, and exited 0. Two adjacent commands, opposite
/// verdicts, and the one that actually executed the miscompiled binary was the
/// one reporting success.
///
/// RE-BASED ONTO A SHIM for the reason its neighbours were: the source that used
/// to disagree was the `&T` forwarding defect, and su2 fixed it, so a program
/// that is still refused is needed to state the invariant at all. The invariant
/// is about the two COMMANDS agreeing, not about any particular defect, and a
/// shimmed gcc makes that independent of what the compiler currently gets wrong.
#[test]
#[cfg(unix)]
fn pdc_run_agrees_with_pdc_compile() {
    let dir = TempDir::new().expect("tempdir");
    let path = shim_path(
        dir.path(),
        &format!(
            "{}echo \"$c:1:1: warning: passing argument 1 of 'f' from incompatible \
             pointer type [-Wincompatible-pointer-types]\" >&2\nexit 0\n",
            SHIM_FIND_TU
        ),
    );

    let compiled = pdc_compile_with_path(TRIVIAL_SOURCE, "linkdiag_runagree", Some(Path::new(&path)));

    let src = dir.path().join("linkdiag_runagree.pd");
    fs::write(&src, TRIVIAL_SOURCE).expect("write source");
    let out = Command::new(env!("CARGO_BIN_EXE_pdc"))
        .current_dir(repo_root())
        .env("PATH", &path)
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
        compiled.code,
        "`pdc run` and `pdc compile` disagree about the same source: run={:?} \
         compile={:?}\n{}",
        out.status.code(),
        compiled.code,
        log
    );
    assert_eq!(
        out.status.code(),
        Some(EXIT_BACKEND_ILL_TYPED),
        "the agreed verdict is not the ill-typed-C one\n{}",
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
// 12-16. what the third round found: attribution, forgery, and ownership
// ---------------------------------------------------------------------------

/// gcc's second translation unit, as it appears on the command line.
const RUNTIME_TU: &str = "runtime/palladium_runtime.c";

/// THE BASENAME COLLISION, WHICH THE OLD GUARD TEST COULD NOT REACH.
///
/// `link_command` compiles TWO units: the generated C and
/// `runtime/palladium_runtime.c`. Attribution used to be
/// `path.ends_with(file_name)` on the raw string, and for a user program called
/// `runtime.pd` the generated unit is `runtime.c` — at which point
///
///     "runtime/palladium_runtime.c".ends_with("runtime.c")  ->  true
///
/// so a genuine defect in the C runtime was charged to the user's file. At gcc
/// exit 0 that is an exit-4 internal compiler error naming `runtime.c` while
/// quoting a diagnostic that names `palladium_runtime.c`, with the binary
/// deleted; at nonzero it is an exit-3 "this compiler emitted C that will not
/// compile". Both are the defect `Classified::foreign` was added to prevent,
/// surviving because a suffix test is not a `file_name()` comparison.
///
/// The previous guard pinned `build_output/B3.c`, whose name shares no suffix
/// with the runtime's, so it could not have failed.
#[test]
fn a_runtime_diagnostic_is_not_charged_to_a_program_named_runtime() {
    let ours = Path::new("build_output/runtime.c");
    let line = format!(
        "{}:12:5: warning: incompatible pointer types assigning to 'char **' \
         from 'char *' [-Wincompatible-pointer-types]",
        RUNTIME_TU
    );

    let c = linker::classify_diagnostics(&line, ours);
    assert!(
        c.fatal.is_empty(),
        "a defect in {} was charged to a user program that merely ends with the \
         same characters: {:?}",
        RUNTIME_TU,
        c.fatal
    );
    assert_eq!(c.foreign.len(), 1, "{:?}", c);

    // The other direction, so the fix is not "attribute nothing": the user's
    // own file is still attributed when it really is named.
    let mine = "build_output/runtime.c:9:9: warning: incompatible pointer types \
                [-Wincompatible-pointer-types]";
    let c2 = linker::classify_diagnostics(mine, ours);
    assert_eq!(c2.fatal.len(), 1, "{:?}", c2);
    assert!(c2.foreign.is_empty(), "{:?}", c2);
}

/// The same collision on the NONZERO path, where it produces an accusation.
///
/// `attributes_an_error_to` had its own copy of the comparison — splitting on
/// the first `:` and asking `ends_with` — so fixing `classify_diagnostics`
/// alone would have left this half wrong. Both now go through one function.
#[test]
fn a_runtime_error_does_not_make_a_program_named_runtime_a_backend_reject() {
    let dir = TempDir::new().expect("tempdir");
    // gcc rejects the RUNTIME and says so, naming only the runtime.
    let path = shim_path(
        dir.path(),
        &format!(
            "#!/bin/sh\necho \"{}:12:5: error: something wrong in the runtime\" >&2\nexit 1\n",
            RUNTIME_TU
        ),
    );
    // The stem makes the generated unit `runtime.c` — the colliding name.
    let run = pdc_compile_with_path(TRIVIAL_SOURCE, "runtime", Some(Path::new(&path)));

    assert_eq!(
        run.code,
        Some(EXIT_GCC_UNEXPLAINED),
        "an error naming only {} was attributed to the user's runtime.c, which \
         is exit 3: 'this compiler emitted C that will not compile'\n{}",
        RUNTIME_TU,
        run.log
    );
    assert_ne!(run.code, Some(EXIT_BACKEND_REJECT), "{}", run.log);
}

/// FIXTURE TEXT MAY NOT CHOOSE THE EXIT CODE.
///
/// The module argues that stderr markers are forgeable because gcc echoes the
/// generated C, and that an exit code is not. That argument condemns any parse
/// of the echo — and `attributes_an_error_to` was such a parse: it searched
/// EVERY line for `": error: "` and attributed on a path match. A Palladium
/// string literal spelling a header for our own translation unit is echoed
/// under the caret display, so a program could make gcc print a line that the
/// scan reads as "gcc rejected your C". Combine it with any unrelated nonzero
/// exit — a full disk, a missing assembler — and an honest 6 becomes an
/// accusing 3, chosen by the fixture.
///
/// WHY THIS TEST DRIVES THE NONZERO PATH AND NOT THE TAG PATH. The tag scan is
/// incidentally safe: `diagnostic_tags` requires the line to END in `]`, and an
/// echoed source line ends in whatever the source ends in. `attributes_an_error_to`
/// has no such requirement — no tag, no trailing bracket — so it is the surface
/// that was actually reachable, and it is the one measured here.
///
/// Both parsers now anchor at column 0. gcc puts a header there; an echo never
/// starts there.
#[test]
#[cfg(unix)]
fn an_echoed_source_line_cannot_forge_a_backend_reject() {
    let dir = TempDir::new().expect("tempdir");
    // gcc fails for a reason that has nothing to do with our C, and — as it
    // always does — echoes the offending source line, indented. The echo
    // contains a forged header naming the translation unit under compilation.
    let path = shim_path(
        dir.path(),
        &format!(
            "{}             echo \"gcc: fatal error: cannot write output: No space left on device\" >&2\n             echo \"  279 |     let s = \\\"$c:1:1: error: forged\\\";\" >&2\n             echo \"      |             ^~~~~~~~~~~~~~~~~~\" >&2\n             exit 1\n",
            SHIM_FIND_TU
        ),
    );
    let run = pdc_compile_with_path(TRIVIAL_SOURCE, "linkdiag_forge", Some(Path::new(&path)));

    assert_eq!(
        run.code,
        Some(EXIT_GCC_UNEXPLAINED),
        "a line of ECHOED SOURCE was parsed as a gcc diagnostic header, so the \
         program under compilation chose its own exit code — and chose the one \
         that accuses this compiler of emitting C that will not compile. That is \
         the forgery surface this module says an exit code does not have.\n{}",
        run.log
    );
    assert_ne!(run.code, Some(EXIT_BACKEND_REJECT), "{}", run.log);
}

/// The anchor must not simply switch attribution off: a REAL header, at column
/// 0, still produces a rejection.
///
/// Negative control for the test above. Without it, `diagnostic_path` returning
/// `None` unconditionally would pass every forgery assertion in this file.
#[test]
#[cfg(unix)]
fn a_real_header_at_column_zero_still_attributes() {
    let dir = TempDir::new().expect("tempdir");
    let path = shim_path(
        dir.path(),
        &format!(
            "{}echo \"$c:1:1: error: a real diagnostic about the real file\" >&2\nexit 1\n",
            SHIM_FIND_TU
        ),
    );
    let run = pdc_compile_with_path(TRIVIAL_SOURCE, "linkdiag_realhdr", Some(Path::new(&path)));
    assert_eq!(
        run.code,
        Some(EXIT_BACKEND_REJECT),
        "a genuine gcc header naming our translation unit stopped being \
         attributed — the anchor over-corrected\n{}",
        run.log
    );
}

/// ILL-TYPED C IN THE RUNTIME IS REFUSED, NOT NOTED.
///
/// `Classified::foreign` was added so a runtime defect would not be blamed on
/// the user. It then returned that diagnostic as a NOTE and let the caller ship
/// the executable — "do not blame the user's file" implemented as "do not tell
/// anyone", which is the original bug (a real diagnostic reaching a branch that
/// discards it) for the third time on this branch.
///
/// Ownership changes the SENTENCE, never the VERDICT: the runtime is linked
/// into every executable this compiler produces.
#[test]
#[cfg(unix)]
fn ill_typed_c_in_the_runtime_is_refused_and_says_whose_it_is() {
    let dir = TempDir::new().expect("tempdir");
    let path = shim_path(
        dir.path(),
        &format!(
            "#!/bin/sh\necho \"{}:12:5: warning: incompatible pointer types \
             [-Wincompatible-pointer-types]\" >&2\nexit 0\n",
            RUNTIME_TU
        ),
    );
    let run = pdc_compile_with_path(TRIVIAL_SOURCE, "linkdiag_rtice", Some(Path::new(&path)));

    assert!(
        !run.ok,
        "ill-typed C in the compiler's own runtime was reported as a note and \
         the executable was shipped\n{}",
        run.log
    );
    assert_eq!(run.code, Some(EXIT_BACKEND_ILL_TYPED), "{}", run.log);
    assert!(
        run.log.contains("its own runtime"),
        "the refusal does not say whose defect it is\n{}",
        run.log
    );
    assert!(
        run.log.contains("Your\n  source is not the defect")
            || run.log.contains("source is not the defect"),
        "the refusal does not clear the user's program\n{}",
        run.log
    );
    assert!(
        !run.binary.exists(),
        "the executable built from ill-typed runtime C is still on disk"
    );
}

/// Every fatal class gets its OWN causal sentence.
///
/// The first version told all four "the callee dereferences one level too far",
/// which is the pointer-depth story and false for the other three. A diagnostic
/// whose job is to report what was observed may not assert a mechanism that was
/// not.
#[test]
fn each_fatal_class_explains_itself() {
    let cases: &[(&str, &str)] = &[
        ("-Wincompatible-pointer-types", "indirection level"),
        ("-Wint-conversion", "an address it never was"),
        ("-Wincompatible-function-pointer-types", "wrong ABI"),
        ("-Wimplicit-function-declaration", "invents a signature"),
    ];
    for (tag, expected) in cases {
        let line = format!("build_output/x.c:1:1: warning: something [{}]", tag);
        let msg = linker::ill_typed_c_error(
            Path::new("build_output/x.c"),
            &[line.as_str()],
            linker::IllTypedOwner::GeneratedC,
            "",
        );
        assert!(
            msg.contains(expected),
            "`{}` is explained with somebody else's causal story:\n{}",
            tag,
            msg
        );
        assert!(msg.contains(tag), "{}", msg);
    }

    // The pointer story must NOT be attached to the others.
    let line = "build_output/x.c:1:1: warning: x [-Wimplicit-function-declaration]";
    let msg = linker::ill_typed_c_error(
        Path::new("build_output/x.c"),
        &[line],
        linker::IllTypedOwner::GeneratedC,
        "",
    );
    assert!(
        !msg.contains("dereferences one level too far"),
        "an undeclared call was explained as a pointer-depth error:\n{}",
        msg
    );
}

/// GNU spells a promoted tag `[-Werror=name]`; clang spells it `[-Werror,-Wname]`.
///
/// Both must resolve to the same tag. This is not hypothetical maintenance: the
/// open `-Werror=return-type` obligation in tests/rust-debt-manifest.txt uses
/// the GNU spelling, so paying it would otherwise turn every promoted line into
/// a policy gap.
#[test]
fn both_toolchains_spellings_of_a_promoted_tag_resolve() {
    let ours = Path::new("build_output/x.c");
    let clang = "build_output/x.c:1:1: error: x [-Werror,-Wincompatible-pointer-types]";
    let gnu = "build_output/x.c:1:1: error: x [-Werror=incompatible-pointer-types]";
    assert_eq!(linker::classify_diagnostics(clang, ours).fatal.len(), 1);
    assert_eq!(
        linker::classify_diagnostics(gnu, ours).fatal.len(),
        1,
        "the GNU spelling of a promoted fatal tag was not recognised"
    );

    // And a promoted KNOWN-BENIGN tag is not a policy gap under either spelling.
    let gnu_benign = "build_output/x.c:1:1: error: x [-Werror=return-type]";
    assert!(
        linker::classify_diagnostics(gnu_benign, ours)
            .unclassified
            .is_empty(),
        "a promoted known tag was reported as undecided"
    );
}

/// A line carrying one KNOWN tag and one nobody has decided about still reports
/// the gap.
///
/// The check used to ask whether ANY tag on the line was known, so an unknown
/// tag was hidden by the company it kept.
#[test]
fn a_known_tag_does_not_hide_an_unknown_one_beside_it() {
    let ours = Path::new("build_output/x.c");
    let line = "build_output/x.c:1:1: warning: x [-Wreturn-type,-Wnobody-decided]";
    let c = linker::classify_diagnostics(line, ours);
    assert_eq!(
        c.unclassified.len(),
        1,
        "an undecided tag was masked by a decided one on the same line: {:?}",
        c
    );
}

// ---------------------------------------------------------------------------
// 23-26. the fourth relocation of the same defect: one line, scanned twice
// ---------------------------------------------------------------------------
//
// Both controls below failed when they were written. They are the same root
// cause: `attributed_to` and `attributes_an_error_to` each scanned the raw line
// for what they needed instead of parsing the header once and using the parse,
// so each could be satisfied by text that is not a header field at all.

/// EXACT BASENAMES COLLIDE, AND THE COLLIDING NAME IS USER-CHOSEN.
///
///     palladium_runtime.pd  ->  build_output/palladium_runtime.c
///     src/runtime_paths.rs  ->  RUNTIME_C_FILE = "palladium_runtime.c"
///
/// Round two replaced `ends_with` with a `file_name()` comparison and called it
/// component comparison. The hole narrowed and did not close: the GENERATED
/// basename is chosen by whoever names the .pd file, so no test that ends at the
/// basename can separate the compiler's runtime from the user's program. A fatal
/// diagnostic in the bundled runtime was then attributed to the user's C — an
/// ICE naming the wrong file on the exit-0 path, and code 3 instead of 6 on the
/// nonzero one.
///
/// The previous collision controls used `runtime.c` vs `palladium_runtime.c`,
/// which is the SUFFIX case; they cannot reach this one.
#[test]
fn the_runtime_and_a_program_sharing_its_exact_name_are_still_told_apart() {
    // What the driver hands gcc for a program called `palladium_runtime.pd`,
    // and what `link_command` hands it for the runtime, on the same line.
    let ours = Path::new("build_output/palladium_runtime.c");
    let runtime_tu = palladium::runtime_paths::runtime_c().expect("runtime resolves");

    let line = format!(
        "{}:12:5: warning: incompatible pointer types assigning to 'char **' \
         from 'char *' [-Wincompatible-pointer-types]",
        runtime_tu.display()
    );
    let c = linker::classify_diagnostics(&line, ours);
    assert!(
        c.fatal.is_empty(),
        "a defect in the compiler's own runtime was charged to a user program \
         that merely shares its file name. The ICE would name {} and quote a \
         diagnostic about {}.\n{:?}",
        ours.display(),
        runtime_tu.display(),
        c.fatal
    );
    assert_eq!(c.foreign.len(), 1, "{:?}", c);

    // And the user's own file is still attributed when it really is the one
    // named, so the fix is not "attribute nothing".
    let mine = "build_output/palladium_runtime.c:9:9: warning: incompatible \
                pointer types [-Wincompatible-pointer-types]";
    let c2 = linker::classify_diagnostics(mine, ours);
    assert_eq!(c2.fatal.len(), 1, "{:?}", c2);
    assert!(c2.foreign.is_empty(), "{:?}", c2);
}

/// A FILENAME IS AT COLUMN 0 TOO, AND THE USER PICKS IT.
///
/// `attributes_an_error_to` asked whether `": error: "` occurred ANYWHERE on the
/// line. `foo: error: forged.pd` is a legal file name, so the ordinary prelude
/// warning arrives as
///
///     build_output/foo: error: forged.c:263:12: warning: … [-W…]
///
/// and the FILENAME supplies the substring, past the column-0 anchor that was
/// added to stop gcc's source echo. Link then fails for an unrelated reason and
/// an honest 6 becomes an accusing 3 — chosen by the name of the source file.
#[test]
fn a_filename_containing_a_severity_marker_cannot_forge_a_rejection() {
    let ours = Path::new("build_output/foo: error: forged.c");
    // The prelude warning every compile produces, under that file name. No
    // `error:` is being reported here at all: the substring is part of the path.
    let stderr = "build_output/foo: error: forged.c:263:12: warning: returning \
                  'const char *' from a function with result type 'char *' \
                  discards qualifiers [-Wincompatible-pointer-types-discards-qualifiers]\n\
                  ld: symbol(s) not found for architecture arm64\n";
    assert!(
        !linker::stderr_rejects(stderr, ours),
        "a file NAME supplied the `: error: ` substring, so gcc's ordinary \
         warning was read as gcc rejecting our translation unit"
    );

    // Negative control: a real error on the same oddly-named file IS a
    // rejection, so the fix is not "never attribute".
    let real = "build_output/foo: error: forged.c:1:1: error: use of undeclared \
                identifier 'x'\n";
    assert!(
        linker::stderr_rejects(real, ours),
        "a genuine error naming our translation unit stopped being attributed"
    );
}

/// The basename collision, end to end through the real binary.
///
/// The unit control above proves the classifier; this proves the wiring, with a
/// program actually named `palladium_runtime.pd` and a shim that diagnoses only
/// the bundled runtime. Before the fix this was exit 3 — "pdc accepted this
/// source and then gcc refused the C it emitted" — about a file gcc never
/// mentioned.
#[test]
#[cfg(unix)]
fn a_program_named_after_the_runtime_is_not_blamed_for_it() {
    let runtime_tu = palladium::runtime_paths::runtime_c().expect("runtime resolves");
    let dir = TempDir::new().expect("tempdir");
    let path = shim_path(
        dir.path(),
        &format!(
            "#!/bin/sh\necho \"{}:12:5: error: something wrong in the runtime\" >&2\nexit 1\n",
            runtime_tu.display()
        ),
    );
    // Stem chosen so codegen emits `build_output/palladium_runtime.c`, whose
    // base name is byte-identical to `runtime_paths::RUNTIME_C_FILE`.
    let run = pdc_compile_with_path(TRIVIAL_SOURCE, "palladium_runtime", Some(Path::new(&path)));

    assert_eq!(
        run.code,
        Some(EXIT_GCC_UNEXPLAINED),
        "an error naming ONLY the bundled runtime was attributed to a user \
         program that shares its file name, which is exit 3: 'this compiler \
         emitted C that will not compile'\n{}",
        run.log
    );
    assert_ne!(run.code, Some(EXIT_BACKEND_REJECT), "{}", run.log);
}

/// The file-name forgery, end to end.
///
/// A source file whose NAME contains `: error: `, compiled with a shim that
/// fails for an unrelated reason and emits only a warning. The substring is in
/// the path; nothing here is an error. Before the fix the file name chose the
/// exit code.
#[test]
#[cfg(unix)]
fn a_source_file_named_with_a_severity_marker_cannot_choose_the_exit_code() {
    let dir = TempDir::new().expect("tempdir");
    let path = shim_path(
        dir.path(),
        &format!(
            "{}\
             echo \"$c:263:12: warning: returning 'const char *' discards qualifiers \
             [-Wincompatible-pointer-types-discards-qualifiers]\" >&2\n\
             echo \"ld: symbol(s) not found for architecture arm64\" >&2\n\
             exit 1\n",
            SHIM_FIND_TU
        ),
    );
    let run = pdc_compile_with_path(TRIVIAL_SOURCE, "foo: error: forged", Some(Path::new(&path)));

    assert_eq!(
        run.code,
        Some(EXIT_GCC_UNEXPLAINED),
        "the NAME of the source file supplied a `: error: ` substring, so an \
         undefined-symbol link failure was reported as gcc rejecting the C this \
         compiler emitted. The accusation was chosen by the file name.\n{}",
        run.log
    );
    assert!(!run.log.contains("gcc compilation failed"), "{}", run.log);
}
