// Native linking for Palladium
// "Generated C that gcc never optimizes is C thrown away"
//
// Every backend path ends the same way: hand gcc the generated .c, the C
// runtime, and an output name. Six call sites used to spell that out by hand
// and *none* of them passed an optimization level, so `pdc` shipped -O0 code
// while `pdc compile -O` was silently ignored. Measured cost on this machine:
// bubble_sort 1714ms -> 286ms, matrix_multiply 1794ms -> 332ms once gcc is
// allowed to optimize the exact same C. This module is the single place that
// answers "how do we invoke gcc?", so a flag can never go missing from one
// path again.

use crate::errors::Result;
use crate::runtime_paths;
use std::path::Path;
use std::process::Command;

/// How hard gcc should work on the generated C.
///
/// Palladium's C backend emits straightforward, unoptimized C (every local is
/// a real store, every call a real call); the optimizer that makes it
/// competitive with rustc lives in gcc. So the *default* is optimized, and
/// turning it off is the explicit choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OptLevel {
    /// `-O0`: for reading/debugging the emitted C, where generated line
    /// structure must survive into the binary.
    None,
    /// `-O2`: the default for every compile.
    #[default]
    Default,
    /// `-O3`: `pdc compile -O` and release package builds.
    Aggressive,
}

impl OptLevel {
    /// The gcc flag for this level.
    pub fn flag(self) -> &'static str {
        match self {
            OptLevel::None => "-O0",
            OptLevel::Default => "-O2",
            OptLevel::Aggressive => "-O3",
        }
    }

    /// Resolve the two CLI booleans into a level. `--no-opt` wins if both are
    /// somehow set, so "I need to debug this" is never overridden by a
    /// leftover `-O` (clap also rejects the combination up front).
    pub fn from_flags(optimize: bool, no_opt: bool) -> Self {
        match (optimize, no_opt) {
            (_, true) => OptLevel::None,
            (true, false) => OptLevel::Aggressive,
            (false, false) => OptLevel::Default,
        }
    }

    /// Package builds: `--release` means `-O3`, everything else the default.
    pub fn for_release(release: bool) -> Self {
        if release {
            OptLevel::Aggressive
        } else {
            OptLevel::Default
        }
    }
}

/// The gcc invocation that turns generated C into an executable.
///
/// Resolves the runtime itself (`-I<dir>` for `pd_prelude.h`, plus
/// `palladium_runtime.c` as a second translation unit) so no call site has to
/// remember either. Returns the command unexecuted: each caller phrases its
/// own failure message ("Linking failed", "Test compilation failed", ...).
pub fn link_command(c_file: &Path, output: &Path, opt: OptLevel) -> Result<Command> {
    let runtime_dir = runtime_paths::runtime_dir()?;
    let runtime_c = runtime_dir.join(runtime_paths::RUNTIME_C_FILE);

    let mut cmd = Command::new("gcc");
    cmd.arg(opt.flag())
        .arg("-I")
        .arg(&runtime_dir)
        .arg(c_file)
        .arg(&runtime_c)
        .arg("-o")
        .arg(output);
    Ok(cmd)
}

// ---------------------------------------------------------------------------
// What gcc FOUND, as distinct from whether gcc GAVE UP
// ---------------------------------------------------------------------------
//
// A process answers two different questions and pdc used to ask only one.
// `status` says whether gcc gave up; `stderr` says what gcc found. Warnings
// cost gcc nothing and leave the status 0, so reading stderr only on failure
// (`src/main.rs`, `if !gcc_output.status.success()`) discarded every diagnostic
// gcc ever produced about C that this compiler generated.
//
// MEASURED, not hypothetical. This program
//
//     fn inner(s: &String) -> i64 { string_len(s) }
//     fn outer(s: &String) -> i64 { inner(s) }
//     fn main() { let t: String = "abcd"; print_int(outer(&t)); }
//
// compiled clean, linked, printed "Created executable", and exited 139 —
// SIGSEGV — because codegen emitted `inner((*s))` where `inner` takes
// `const char**`. gcc had already said so, at that exact line, and pdc threw
// the sentence away:
//
//     build_output/B3.c:279:18: warning: incompatible pointer types passing
//       'const char *' to parameter of type 'const char **'; take the address
//       with & [-Wincompatible-pointer-types]

/// gcc diagnostic tags pdc refuses to ship, and the reason each one is in.
///
/// THE MEMBERSHIP RULE: a tag belongs here when there is NO Palladium program
/// for which the C gcc is objecting to is what we meant to emit. That makes the
/// diagnostic a statement about the compiler rather than about the user's
/// program, and the only honest response to it is to stop.
///
/// `-Wincompatible-pointer-types` — a call or assignment where the pointee
/// types disagree. Palladium's surface has no way to ask for one: every pointer
/// in the emitted C is synthesised by codegen from a type the checker already
/// approved, so a mismatch means codegen lowered a type wrongly. It is also the
/// dangerous direction — gcc passes the value through, the callee dereferences
/// one level too far, and the program segfaults at runtime with no diagnostic
/// anywhere.
pub const FATAL_DIAGNOSTIC_TAGS: &[&str] = &["-Wincompatible-pointer-types"];

/// Tags that are deliberately NOT fatal, each with the reason it is out.
///
/// WHY THIS LIST EXISTS AT ALL, i.e. why not `-Werror`. Every program compiled
/// by this tree today carries a warning from the emitted prelude:
///
/// ```text
/// warning: returning 'const char *' from a function with result type
///   'char *' discards qualifiers
///   [-Wincompatible-pointer-types-discards-qualifiers]
/// ```
///
/// `__pd_read_file_to_string` is declared `char*` and returns
/// `__pd_empty_owned()`, which is `const char*`. A blanket `-Werror` therefore
/// fails 100% of compiles on the day it lands, and a gate that is born red is
/// switched off within the week. So the escalation is scoped — and a scoped
/// allowlist that nobody can read is the same failure as the silent discard
/// this change removes, which is why every exclusion is named here with its
/// reason and why `every_excluded_tag_states_why` requires the reason to exist.
///
/// This is a LIST OF DECISIONS, not a list of every tag gcc has. A tag absent
/// from both constants is non-fatal by default: the default is unchanged
/// behaviour, and moving a tag into `FATAL_DIAGNOSTIC_TAGS` is the deliberate
/// act.
pub const NON_FATAL_DIAGNOSTIC_TAGS: &[(&str, &str)] = &[
    (
        "-Wincompatible-pointer-types-discards-qualifiers",
        "fires in EVERY compile from the emitted prelude's \
         `__pd_read_file_to_string`, which is declared `char*` and returns the \
         `const char*` from `__pd_empty_owned()`. The qualifier is dropped on a \
         pointer that is never written through, so it does not miscompile; it is \
         a real defect in the prelude and it is owned by the C backend \
         (src/codegen/mod.rs emits that text, runtime/pd_prelude.h holds the \
         same body). Escalating it before it is fixed would make this gate red \
         on arrival for every program.",
    ),
    (
        "-Wreturn-type",
        "a generated function that falls off its end. Already owned, with its \
         own sequencing: codegen lowers `match` to an if/else-if chain with no \
         final `else`, so gcc cannot prove every path returns for any tail \
         `match`. The obligation is held open by \
         `the_linker_will_ask_gcc_to_reject_a_function_that_falls_off_its_end` \
         in tests/rust-debt-manifest.txt, which asks for `-Werror=return-type` \
         on the gcc command line rather than a stderr scan here.",
    ),
];

/// The `-Wname` tags a gcc/clang diagnostic line carries, if any.
///
/// The tag is the trailing `[-Wname]`. Two details that a substring search gets
/// wrong, and both of them matter here:
///
/// * `-Wincompatible-pointer-types-discards-qualifiers` CONTAINS
///   `-Wincompatible-pointer-types`. The tag that is fatal and the tag that is
///   in every single compile differ by a suffix, so membership is tested on the
///   whole tag and never on a substring of the line.
/// * under `-Werror` the bracket holds a comma-separated list
///   (`[-Werror,-Wname]`), so it is split before comparison.
///
/// Lines without a `warning:`/`error:` header are not diagnostics — `note:`
/// continuations and the echoed source line are skipped, so a caret pointing at
/// text that happens to end in a bracket cannot be read as a tag.
fn diagnostic_tags(line: &str) -> Vec<&str> {
    if !line.contains(": warning: ") && !line.contains(": error: ") {
        return Vec::new();
    }
    let Some(rest) = line.trim_end().strip_suffix(']') else {
        return Vec::new();
    };
    let Some(open) = rest.rfind("[-W") else {
        return Vec::new();
    };
    rest[open + 1..].split(',').map(str::trim).collect()
}

/// The lines of gcc's stderr that name a tag in [`FATAL_DIAGNOSTIC_TAGS`].
///
/// Header lines only. gcc follows a diagnostic with `note:` lines and an echo
/// of the source; those are context for a human re-running gcc, and carrying
/// them here would make the count of "what we found" depend on gcc's rendering.
pub fn fatal_diagnostics(stderr: &str) -> Vec<&str> {
    stderr
        .lines()
        .filter(|line| {
            diagnostic_tags(line)
                .iter()
                .any(|tag| FATAL_DIAGNOSTIC_TAGS.contains(tag))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// THE SAME `if` LIED IN BOTH DIRECTIONS
// ---------------------------------------------------------------------------
//
// Reading stderr only on failure discarded what gcc FOUND (above). The other
// half of the same statement flattened what gcc DID: every nonzero exit became
// the one string `gcc compilation failed`. gcc rejecting our C, gcc killed by
// the OOM killer, gcc not being installed, gcc dying on SIGSEGV — four
// different facts, one marker.
//
// That marker is now load-bearing. `scripts/conformance.sh` on the sibling
// branch reads it and concludes "pdc accepted the source and gcc refused the C
// it emitted — a defect in this compiler". A gcc that was KILLED would be
// certified as a codegen defect by that reading, and the harness cannot tell,
// because the only evidence it was handed is a sentence.
//
// So the outcome is structured, and the structure leaves the process through a
// channel no fixture can write: THE EXIT CODE. Marker lines in stderr are
// forgeable — gcc echoes the generated C, the generated C carries the fixture's
// identifiers, and this repo has already had a fixture containing the word
// `Linking` classified as a link failure by a grep. An exit code has no such
// path from fixture text.

/// gcc ran, gave a verdict, and the verdict was no.
///
/// The one case in which "the backend emitted C that will not compile" is a
/// supportable accusation.
pub const EXIT_BACKEND_REJECT: i32 = 3;

/// gcc ran, exited 0, and diagnosed C that pdc had no business emitting.
///
/// Distinct from [`EXIT_BACKEND_REJECT`] because gcc did NOT refuse: a binary
/// was produced (and has been deleted). A gate that treats the two as one
/// cannot tell "we were stopped" from "we were about to ship a segfault".
pub const EXIT_BACKEND_ILL_TYPED: i32 = 4;

/// gcc never gave a verdict: it could not be spawned, the runtime could not be
/// located, or it died on a signal.
///
/// This is a statement about the machine, never about the compiler or the
/// program, and it is the case the flattened string used to swallow.
pub const EXIT_TOOLCHAIN: i32 = 5;

// Why these numbers. `1` is pdc's existing "something went wrong" and every
// front-end refusal already uses it — reusing it would put the new distinction
// back inside the old one. `2` is spoken for twice over: clap exits 2 on a
// usage error, and `make` collapses every nonzero recipe status to 2, so a
// gate reading `make`'s status could not tell 2 from 2. 3/4/5 are unused here.

/// Why a link did not produce a usable executable.
///
/// FOUR causes, kept apart because they support four different accusations.
/// Collapsing any two of them lets a gate blame the compiler for the machine,
/// which is the specific failure this enum exists to prevent.
#[derive(Debug)]
pub enum LinkError {
    /// gcc could not be spawned, or the runtime could not be located. gcc never
    /// looked at the C.
    Toolchain(String),
    /// gcc started and was killed by a signal. It never reached a verdict
    /// either, so it belongs with `Toolchain` and NOT with `GccFailed` — a
    /// signalled process also has a nonzero status, which is exactly how it
    /// used to be reported as a rejection.
    GccDied(String),
    /// gcc ran to completion and exited nonzero: it REJECTED the translation
    /// unit. The message is byte-identical to the one pdc printed before this
    /// change, because `scripts/conformance.sh` matches on it.
    GccFailed(String),
    /// gcc exited 0 and diagnosed C that pdc generated. An internal compiler
    /// error: no Palladium program asks for ill-typed C.
    IllTypedC(String),
}

impl LinkError {
    /// The process exit code pdc reports for this outcome.
    ///
    /// The machine-readable half of the diagnostic. A shell gate reads `$?`;
    /// nothing a fixture can contain reaches this value.
    pub fn exit_code(&self) -> i32 {
        match self {
            // One code for both, deliberately: "gcc never gave a verdict" is
            // one fact about the machine, and a gate's decision (do not blame
            // the compiler) is the same for a missing gcc and a killed one.
            // They stay separate VARIANTS because the sentence a human needs is
            // different.
            LinkError::Toolchain(_) | LinkError::GccDied(_) => EXIT_TOOLCHAIN,
            LinkError::GccFailed(_) => EXIT_BACKEND_REJECT,
            LinkError::IllTypedC(_) => EXIT_BACKEND_ILL_TYPED,
        }
    }
}

impl std::fmt::Display for LinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinkError::Toolchain(m)
            | LinkError::GccDied(m)
            | LinkError::GccFailed(m)
            | LinkError::IllTypedC(m) => {
                write!(f, "{}", m)
            }
        }
    }
}

impl std::error::Error for LinkError {}

/// The signal that killed a process, if one did.
///
/// `ExitStatus::success()` is false for a signalled child exactly as it is for
/// a child that exited 1, which is why the old code could not tell them apart.
/// This is the real mechanism rather than a heuristic over stderr: a killed gcc
/// may have printed nothing at all, and "no diagnostics" is not evidence of
/// anything.
#[cfg(unix)]
fn death_signal(status: &std::process::ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.signal()
}

/// Non-Unix: there is no signal to report, so a nonzero status is a verdict.
#[cfg(not(unix))]
fn death_signal(_status: &std::process::ExitStatus) -> Option<i32> {
    None
}

/// The pdc-level diagnostic for C the backend should never have emitted.
///
/// Phrased as an internal compiler error and not as a gcc dump: the reader of
/// this message wrote Palladium, the text gcc is objecting to is C they never
/// saw, and telling them "incompatible pointer types" without saying whose
/// mistake it is invites them to go looking for it in their own source.
pub fn ill_typed_c_error(c_file: &Path, diagnostics: &[&str], binary_note: &str) -> String {
    let mut msg = String::new();
    msg.push_str("internal compiler error: the C backend emitted ill-typed C\n");
    msg.push_str(&format!(
        "\n  gcc accepted {} (it exited 0) but diagnosed a pointer-type\n  \
         confusion in it. That C is never what pdc meant to emit — no Palladium\n  \
         program can ask for it — so this is a defect in the compiler, not in\n  \
         your program. Left alone it miscompiles silently: the callee\n  \
         dereferences one level too far and the program crashes at runtime.\n\n",
        c_file.display()
    ));
    for d in diagnostics {
        msg.push_str(&format!("  {}\n", d.trim()));
    }
    msg.push_str(binary_note);
    msg.push_str(&format!(
        "\n  Please report this, with your Palladium source and the line of\n  \
         generated C named above. Full context: gcc -c {} -I <pdc --print-runtime>\n",
        c_file.display()
    ));
    msg
}

/// Run the gcc invocation, and ask it every question it can answer.
///
/// `link_command` builds the command; this runs it and decides what the run
/// means. Every caller that wants an executable should use this rather than
/// `.output()` on the command: `.output()` hands back a status and a buffer,
/// and turning those two into a verdict is the part that was being got wrong.
///
/// The three questions, in the only order that works:
///   1. did gcc reach a verdict at all?   (spawn failure, signal)
///   2. was the verdict no?               (normal nonzero exit)
///   3. what did gcc find on the way?     (stderr, even at exit 0)
pub fn link(c_file: &Path, output: &Path, opt: OptLevel) -> std::result::Result<(), LinkError> {
    let gcc_output = link_command(c_file, output, opt)
        .map_err(|e| LinkError::Toolchain(e.to_string()))?
        .output()
        .map_err(|e| LinkError::Toolchain(format!("Failed to run gcc: {}", e)))?;

    let stderr = String::from_utf8_lossy(&gcc_output.stderr);

    // Question one: did gcc reach a verdict? A signalled gcc is nonzero too, so
    // this MUST precede the status check or the answer to question two is a
    // guess. Nothing here is inferred from stderr: a killed process may have
    // printed nothing, and silence is not evidence.
    if let Some(sig) = death_signal(&gcc_output.status) {
        return Err(LinkError::GccDied(format!(
            "gcc did not finish: it was killed by signal {} while compiling {}.\n  \
             gcc never reached a verdict, so nothing here is known about that C —\n  \
             this is a fact about the machine (out of memory, a killed process\n  \
             group), not about the compiler or the program. Anything gcc managed\n  \
             to print before it died follows.\n{}",
            sig,
            c_file.display(),
            stderr
        )));
    }

    // Question two: was the verdict no? Message unchanged, byte for byte.
    if !gcc_output.status.success() {
        return Err(LinkError::GccFailed(format!(
            "gcc compilation failed:\n{}",
            stderr
        )));
    }

    // Question three: what did gcc find? This is the half that was discarded.
    let fatal = fatal_diagnostics(&stderr);
    if fatal.is_empty() {
        return Ok(());
    }

    // gcc exited 0, so the executable EXISTS and it is the one that segfaults.
    // Reporting an error while leaving it on disk would let the next `./prog`
    // run the miscompile we just refused to ship.
    let binary_note = match std::fs::remove_file(output) {
        Ok(()) => format!(
            "\n  The executable gcc produced ({}) has been removed: it was built\n  \
             from this C and does not run correctly.\n",
            output.display()
        ),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => format!(
            "\n  WARNING: the executable gcc produced ({}) could NOT be removed\n  \
             ({}). Do not run it — it was built from this C.\n",
            output.display(),
            e
        ),
    };

    Err(LinkError::IllTypedC(ill_typed_c_error(
        c_file,
        &fatal,
        &binary_note,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_map_to_gcc_flags() {
        assert_eq!(OptLevel::None.flag(), "-O0");
        assert_eq!(OptLevel::Default.flag(), "-O2");
        assert_eq!(OptLevel::Aggressive.flag(), "-O3");
    }

    /// The regression this module exists for: no flags at all must still be
    /// an optimized build.
    #[test]
    fn default_is_optimized() {
        assert_eq!(OptLevel::default(), OptLevel::Default);
        assert_eq!(OptLevel::from_flags(false, false).flag(), "-O2");
    }

    #[test]
    fn optimize_flag_selects_o3_and_no_opt_wins() {
        assert_eq!(OptLevel::from_flags(true, false), OptLevel::Aggressive);
        assert_eq!(OptLevel::from_flags(false, true), OptLevel::None);
        assert_eq!(OptLevel::from_flags(true, true), OptLevel::None);
    }

    #[test]
    fn release_packages_use_o3() {
        assert_eq!(OptLevel::for_release(true), OptLevel::Aggressive);
        assert_eq!(OptLevel::for_release(false), OptLevel::Default);
    }

    /// The level must actually reach the command line, ahead of the sources.
    #[test]
    fn link_command_carries_the_level_and_the_runtime() {
        let cmd = link_command(
            Path::new("/tmp/x.c"),
            Path::new("/tmp/x"),
            OptLevel::Aggressive,
        )
        .expect("runtime should resolve in a dev checkout");

        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();

        assert_eq!(cmd.get_program(), "gcc");
        assert_eq!(args[0], "-O3");
        assert!(args.iter().any(|a| a == "-I"), "{:?}", args);
        assert!(
            args.iter()
                .any(|a| a.ends_with(runtime_paths::RUNTIME_C_FILE)),
            "{:?}",
            args
        );
        assert_eq!(args[args.len() - 2], "-o");
        assert_eq!(args[args.len() - 1], "/tmp/x");
    }

    // -----------------------------------------------------------------------
    // Reading stderr: the classifier, over gcc's real output text
    // -----------------------------------------------------------------------

    /// gcc's actual stderr for the segfaulting three-line program in the module
    /// comment, captured from `gcc -c build_output/B3.c` on this tree. Pasted
    /// rather than paraphrased: the classifier's whole job is to read what gcc
    /// really prints, and a hand-written approximation would let it pass while
    /// disagreeing with the toolchain.
    ///
    /// Note that it contains BOTH tags — the fatal one and the prelude one that
    /// must not be fatal — which is the reason it is one constant and not two.
    const REAL_STDERR: &str = "\
build_output/B3.c:263:12: warning: returning 'const char *' from a function with result type 'char *' discards qualifiers [-Wincompatible-pointer-types-discards-qualifiers]
  263 |     return __pd_empty_owned();
      |            ^~~~~~~~~~~~~~~~~~
build_output/B3.c:279:18: warning: incompatible pointer types passing 'const char *' to parameter of type 'const char **'; take the address with & [-Wincompatible-pointer-types]
  279 |     return inner((*s));
      |                  ^~~~
      |                  &
build_output/B3.c:274:30: note: passing argument to parameter 's' here
  274 | long long inner(const char** s) {
      |                              ^
2 warnings generated.
";

    #[test]
    fn the_type_confusion_is_found_in_real_gcc_output() {
        let found = fatal_diagnostics(REAL_STDERR);
        assert_eq!(found.len(), 1, "{:?}", found);
        assert!(found[0].contains("B3.c:279:18"), "{:?}", found);
        assert!(
            found[0].ends_with("[-Wincompatible-pointer-types]"),
            "{:?}",
            found
        );
    }

    /// THE SUFFIX TRAP, and the reason `diagnostic_tags` compares whole tags.
    ///
    /// `-Wincompatible-pointer-types-discards-qualifiers` contains
    /// `-Wincompatible-pointer-types` as a prefix. A `stderr.contains(tag)`
    /// implementation is green on the test above and refuses EVERY program in
    /// the repo, because that prelude warning is in every compile. This is the
    /// test that tells the two implementations apart.
    #[test]
    fn the_prelude_qualifier_warning_is_not_fatal() {
        let prelude_only = REAL_STDERR
            .lines()
            .filter(|l| !l.contains("incompatible pointer types passing"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            prelude_only.contains("-Wincompatible-pointer-types-discards-qualifiers"),
            "the fixture no longer carries the prelude warning, so this proves nothing"
        );
        assert!(
            fatal_diagnostics(&prelude_only).is_empty(),
            "the prelude's discards-qualifiers warning was escalated; it is in \
             every compile, so this refuses every program: {:?}",
            fatal_diagnostics(&prelude_only)
        );
    }

    /// Clean gcc output is clean. Guards the direction where a bug in the
    /// scanner refuses programs gcc never said anything about.
    #[test]
    fn silence_and_untagged_lines_are_not_diagnostics() {
        assert!(fatal_diagnostics("").is_empty());
        assert!(fatal_diagnostics("2 warnings generated.\n").is_empty());
        // A `note:` continuation and an echoed source line that happens to end
        // in a bracket. Neither is a diagnostic header.
        let noise = "x.c:1:1: note: passing argument to parameter 's' here\n\
                     x.c:2:2: warning: something else [-Wunused-variable]\n\
                     int a[] = { [-Wincompatible-pointer-types]\n";
        assert!(fatal_diagnostics(noise).is_empty());
    }

    /// gcc under `-Werror` spells the tag `[-Werror,-Wname]`. pdc does not pass
    /// `-Werror` today (see `NON_FATAL_DIAGNOSTIC_TAGS`), but the day a caller
    /// does, the tag must still be recognised rather than silently stop
    /// matching.
    #[test]
    fn a_comma_separated_werror_tag_still_matches() {
        let line =
            "x.c:1:1: error: incompatible pointer types [-Werror,-Wincompatible-pointer-types]";
        assert_eq!(fatal_diagnostics(line).len(), 1);
    }

    /// The escalation is scoped, and the scope is legible. An exclusion with no
    /// stated reason is an allowlist nobody can audit — the same failure as the
    /// silent discard this module removes.
    #[test]
    fn every_excluded_tag_states_why() {
        assert!(!NON_FATAL_DIAGNOSTIC_TAGS.is_empty());
        for (tag, why) in NON_FATAL_DIAGNOSTIC_TAGS {
            assert!(tag.starts_with("-W"), "not a gcc tag: {}", tag);
            assert!(
                why.len() > 80,
                "`{}` is excluded without a reason a reviewer can weigh: {:?}",
                tag,
                why
            );
            assert!(
                !FATAL_DIAGNOSTIC_TAGS.contains(tag),
                "`{}` is in both lists",
                tag
            );
        }
    }

    /// The message is a pdc diagnostic, not a gcc dump handed to someone who
    /// never wrote C. It has to say whose defect it is and it has to carry the
    /// gcc line it is based on.
    #[test]
    fn the_message_blames_the_compiler_and_cites_gcc() {
        let found = fatal_diagnostics(REAL_STDERR);
        let msg = ill_typed_c_error(Path::new("build_output/B3.c"), &found, "\n  removed\n");
        assert!(msg.contains("internal compiler error"), "{}", msg);
        assert!(msg.contains("not in\n  your program"), "{}", msg);
        assert!(msg.contains("build_output/B3.c:279:18"), "{}", msg);
        assert!(msg.contains("removed"), "{}", msg);
        // `scripts/conformance.sh` greps the compiler log for `error` to build
        // the one-line detail it reports. A message it cannot see is a blank
        // column in the gate output.
        assert!(msg.contains("error"), "{}", msg);
    }
}
