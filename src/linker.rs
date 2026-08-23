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
/// THE RULE IS NOT SELF-ENFORCING, so it is enforced. A first version of this
/// list had ONE member, and a review found three more tags that satisfy the rule
/// above verbatim and appeared in neither list — so no decision had been
/// recorded about them at all, while the module documented itself as the place
/// such decisions live. "The gate is born green" was a symptom of that, not a
/// strength. `every_shape_of_ill_typed_c_is_fatal` (tests/linker_diagnostics.rs)
/// now hands the real toolchain one C snippet per shape of "the types
/// disagree", asks what tag it produces, and requires the answer to be in this
/// list. The list is therefore derived from the PROPERTY by a machine, not from
/// what somebody happened to think of.
///
/// `-Wincompatible-pointer-types` — a call or assignment where the pointee
/// types disagree. Palladium's surface has no way to ask for one: every pointer
/// in the emitted C is synthesised by codegen from a type the checker already
/// approved, so a mismatch means codegen lowered a type wrongly. It is also the
/// dangerous direction — gcc passes the value through, the callee dereferences
/// one level too far, and the program segfaults at runtime with no diagnostic
/// anywhere.
///
/// `-Wint-conversion` — an integer used as a pointer or the reverse. THE ONE
/// WITH A NAMED VICTIM IN THIS REPO: `CLAUDE.md` records six `file_*` builtins
/// whose handle representation split in two (legacy = an index, extended =
/// `FileHandle` = `void*`), and states that a gcc error is the only thing
/// currently stopping `file_seek(file_open(p), 0, 0)` from dereferencing the
/// integer `1` as a `FILE*`. That gcc error is this tag — and it is an error by
/// default only on newer clang. On GNU gcc 13 and earlier, which is what
/// `ubuntu-latest` gives this repo, it is a WARNING: exit 0, binary shipped,
/// segfault at runtime. Escalating it here is what makes the two toolchains
/// agree.
///
/// `-Wincompatible-function-pointer-types` — a function pointer assigned from a
/// function of a different signature. Same family, same impossibility: codegen
/// synthesises every function type from a checked signature.
///
/// `-Wimplicit-function-declaration` — a call to a function that was never
/// declared. In generated C this always means codegen emitted a call it never
/// emitted a declaration for; C89 then invents a signature and the call is
/// compiled against a guess. Also an error on newer clang and a warning on
/// older gcc.
pub const FATAL_DIAGNOSTIC_TAGS: &[&str] = &[
    "-Wincompatible-pointer-types",
    "-Wint-conversion",
    "-Wincompatible-function-pointer-types",
    "-Wimplicit-function-declaration",
];

/// Tags known to this compiler and deliberately NOT fatal, each with the reason.
///
/// THIS LIST IS LOAD-BEARING. It was not, in the first version of this module:
/// `fatal_diagnostics` consulted only the denylist above, so every tag absent
/// from both constants was silently non-fatal and this constant was read by
/// nothing but a test asserting its explanations were long enough. A review
/// called that what it was — decoration — and it was: naming the thing instead
/// of reading it, which is the exact failure the module exists to remove.
/// [`classify_diagnostics`] now consults BOTH lists, and a tag in neither is
/// reported as a POLICY GAP rather than passed over.
///
/// WHY NOT `-Werror`. Every program compiled by this tree carries the first row
/// below, from the emitted prelude. A blanket `-Werror` fails 100% of compiles
/// on the day it lands, and a gate that is born red is switched off within the
/// week.
///
/// THE ROWS ARE MEASURED, AND THE MEASUREMENT HAS A TOOLCHAIN. Every tag here
/// was observed by compiling all 108 generated .c files this repo's corpus
/// produces — on macOS, where `gcc` is Apple clang 21. THESE ARE CLANG
/// SPELLINGS. GNU gcc names several of the same diagnostics differently, most
/// importantly the prelude's, which it calls `-Wdiscarded-qualifiers` rather
/// than `-Wincompatible-pointer-types-discards-qualifiers`; that tag is in
/// neither list, so on GNU it arrives as a policy-gap note on 108/108 compiles.
/// Non-fatal by construction, and noisy, and NOT what "zero policy-gap notes"
/// measured — that receipt is a macOS receipt and is reported as one. Anyone
/// running the corpus under GNU should expect the notes and add the GNU
/// spellings here with their reasons.
///
/// The observed table:
///
/// ```text
/// 324 [-Wparentheses-equality]
/// 108 [-Wincompatible-pointer-types-discards-qualifiers]
///   2 [-Wstring-compare]
///   2 [-Wreturn-type]
///   1 [-Wunused-value]
///   1 [-Wreturn-stack-address]
///   1 [-Wmain-return-type]
/// ```
///
/// Five of those seven were missing from this list while it claimed to be the
/// place where such decisions are recorded. Two of the five look like genuine
/// defects. Neither is escalated HERE — escalation changes which programs
/// compile and needs its own measured blast radius — but both are now written
/// down with the file that produces them, which is the difference between a
/// deferred decision and a lost one.
pub const NON_FATAL_DIAGNOSTIC_TAGS: &[(&str, &str)] = &[
    (
        "-Wincompatible-pointer-types-discards-qualifiers",
        "fires in EVERY compile (108/108) from the emitted prelude's \
         `__pd_read_file_to_string`, which is declared `char*` and returns the \
         `const char*` from `__pd_empty_owned()`. The qualifier is dropped on a \
         pointer that is never written through, so it does not miscompile; it is \
         a real defect in the prelude and it is owned by the C backend \
         (src/codegen/mod.rs emits that text, runtime/pd_prelude.h holds the \
         same body). Escalating it before it is fixed would make this gate red \
         on arrival for every program.",
    ),
    (
        "-Wparentheses-equality",
        "324 occurrences, the most common diagnostic this compiler produces. \
         codegen parenthesises every binary operand, so an `==` comparison is \
         emitted as `if ((a == b))` and gcc suspects a mistyped assignment. \
         Purely a spelling of the generated C: the value computed is the one \
         intended, and no program behaves differently. It is noise from an \
         emitter that does not track precedence, not a defect in what it means.",
    ),
    (
        "-Wreturn-type",
        "a generated function that falls off its end (2 occurrences, both in \
         stdlib_tail_match.c). Already owned, with its own sequencing: codegen \
         lowers `match` to an if/else-if chain with no final `else`, so gcc \
         cannot prove every path returns for any tail `match`. The obligation is \
         held open by \
         `the_linker_will_ask_gcc_to_reject_a_function_that_falls_off_its_end` \
         in tests/rust-debt-manifest.txt, which asks for `-Werror=return-type` \
         on the gcc command line rather than a stderr scan here.",
    ),
    (
        "-Wreturn-stack-address",
        "1 occurrence, in lifetimes_uninferred.c: `address of stack memory \
         associated with local variable 'x' returned`. A REAL MISCOMPILE, and \
         NOT AN UNOWNED ONE: the defect is unchecked lifetimes and it is already \
         held by tests/regression/lifetimes_uninferred.pd, which declares a `run` \
         row in the conformance manifest, carries a promotion protocol, and has a \
         paired reject fixture. Escalating this tag would convert that \
         deliberately declared fixture into COMPILE_FAIL and break the gate that \
         owns the defect — which is a stronger reason to leave it than any \
         blast-radius argument. (An earlier version of this row sent the reader \
         to `&T` forwarding. That was wrong: the two share a symptom, not a \
         cause.)",
    ),
    (
        "-Wstring-compare",
        "2 occurrences, both in tiny_v3.c, which is under bootstrap/v3_incremental \
         and outside conformance scope. It points at a real defect — Palladium \
         `==` on String lowers to a POINTER comparison — but THIS TAG CANNOT \
         CARRY THAT DEFECT, measured: a program comparing two runtime-built \
         strings with identical contents prints `NOT equal` and produces ZERO \
         string-compare diagnostics, because gcc only remarks when one operand is \
         a literal. The shape that warns (literal vs literal) is the shape that \
         happens to give the RIGHT answer, since C pools identical literals. \
         Escalating would catch the harmless case, miss every wrong answer, and \
         make the defect look handled — the appearance of coverage, which is \
         worse than none. The repair is routing the operator to `string_eq` \
         (src/builtins.rs, and the prelude already emits `strcmp(s1,s2) == 0`), \
         which is a typeck/codegen change and belongs in a requirement row, not \
         in a warning policy.",
    ),
    (
        "-Wunused-value",
        "1 occurrence, in stdlib_tail_return.c: an expression statement whose \
         value is discarded. Legal C and legal Palladium — a call used as a \
         statement — and gcc is remarking on the shape, not on a defect.",
    ),
    (
        "-Wmain-return-type",
        "1 occurrence, in test_simple.c. codegen emitted a `main` whose return \
         type is not `int`. A conformance wart in the emitted C rather than a \
         miscompile of the user's program: the C runtime start-up handles it, \
         and the value the program computes is unaffected. Owned by the C \
         backend.",
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
/// * under `-Werror` the bracket holds a list, and THE TWO COMPILERS SPELL IT
///   DIFFERENTLY: clang writes `[-Werror,-Wname]`, GNU gcc writes
///   `[-Werror=name]`. Splitting on `,` alone reads the GNU form as one tag
///   called `-Werror=name`, which is in no list, so a promoted diagnostic would
///   arrive as a policy gap rather than as the tag it is. This matters on a
///   schedule: the open `-Werror=return-type` obligation in
///   tests/rust-debt-manifest.txt uses exactly the GNU spelling. Both forms are
///   split, and `-Werror=` is normalised back to `-W`.
///
/// Lines without a `warning:`/`error:` header are not diagnostics — `note:`
/// continuations and the echoed source line are skipped, so a caret pointing at
/// text that happens to end in a bracket cannot be read as a tag.
///
/// TWO ASSUMPTIONS THIS PARSER MAKES, both of which [`link`] arranges rather
/// than hopes for:
///
/// * **English.** `": warning: "` and `": error: "` are literal. A localized
///   gcc says `": Warnung: "` and every line here matches nothing — the tag
///   would still be in the text and would be read as absent, i.e. silently
///   non-fatal. `link` runs gcc under `LC_ALL=C`, so the toolchain is speaking
///   the language this parser reads.
/// * **No colour.** A colourised gcc wraps the tag in SGR escapes, so the line
///   does not end in `]`. `link` captures with `Command::output()`, i.e. a
///   pipe, and both gcc and clang default to `auto` colour and therefore to
///   NO colour on a pipe. If a caller ever forces `-fdiagnostics-color=always`,
///   this parser stops seeing tags; that is why the colour decision is not
///   left to the environment either.
fn diagnostic_tags(line: &str) -> Vec<String> {
    // Same anchor as `diagnostic_path`, and for the same reason: an echoed
    // source line is indented, and reading one as a header is how fixture text
    // reaches a decision it must never reach.
    if line.starts_with(|c: char| c.is_whitespace()) {
        return Vec::new();
    }
    if !line.contains(": warning: ") && !line.contains(": error: ") {
        return Vec::new();
    }
    let Some(rest) = line.trim_end().strip_suffix(']') else {
        return Vec::new();
    };
    let Some(open) = rest.rfind("[-W") else {
        return Vec::new();
    };
    rest[open + 1..]
        .split(',')
        .map(str::trim)
        // GNU's `-Werror=name` denotes the SAME diagnostic as clang's
        // `-Wname`; the prefix records that it was promoted, not what it is.
        .map(|t| match t.strip_prefix("-Werror=") {
            Some(name) => format!("-W{}", name),
            None => t.to_string(),
        })
        .collect()
}

/// The lines of gcc's stderr that name a tag in [`FATAL_DIAGNOSTIC_TAGS`] AND
/// are attributed to `c_file`.
///
/// Header lines only. gcc follows a diagnostic with `note:` lines and an echo
/// of the source; those are context for a human re-running gcc, and carrying
/// them here would make the count of "what we found" depend on gcc's rendering.
pub fn fatal_diagnostics<'a>(stderr: &'a str, c_file: &Path) -> Vec<&'a str> {
    classify_diagnostics(stderr, c_file).fatal
}

/// Every diagnostic gcc emitted, sorted into the policy this compiler has.
///
/// FOUR OUTCOMES, and only the first is fatal.
///
/// `fatal` / `unclassified` are about OUR translation unit. The third state —
/// a tag NOBODY HAS DECIDED ABOUT — is a fact about this compiler's policy, not
/// about the program, and it is what a new gcc release looks like on the day it
/// starts diagnosing a real miscompile this repo has been shipping. The first
/// version of this module had no such bucket: every unknown tag was silently
/// non-fatal and [`NON_FATAL_DIAGNOSTIC_TAGS`] was read by nothing.
///
/// Unclassified is deliberately NOT fatal. Making it fatal would mean a
/// toolchain upgrade breaks every build in the tree on a Tuesday, which is how
/// a gate gets turned off. It is instead reported, loudly, by [`link`]'s caller.
///
/// `foreign` IS THE ONE A REVIEW HAD TO FIND. `link_command` puts TWO
/// translation units on the command line — the generated C and
/// `runtime/palladium_runtime.c` — and the first version of this scan read all
/// of stderr with no path filter while `ill_typed_c_error` hard-coded the
/// user's file into a message saying gcc "diagnosed a pointer-type confusion in
/// it". The day anyone introduces a pointer mismatch in the runtime, EVERY
/// compile of EVERY program would have died with an internal compiler error
/// naming a file that is innocent, quoting a diagnostic that names a different
/// one. That is precisely the "born red for every program" outcome the
/// `-Werror` discussion above exists to avoid, arriving through a door that
/// discussion did not check.
#[derive(Debug, Default)]
pub struct Classified<'a> {
    /// Fatal-tagged diagnostics attributed to the translation unit under
    /// compilation.
    ///
    /// `fatal` and `foreign` BOTH stop the build. They differ in OWNERSHIP, not
    /// in effect: this one says the defect is in the C generated from the user's
    /// program, the other says it is in C this compiler ships. (This comment
    /// said "these, and only these, stop the build" for one round after
    /// `foreign` became a refusal, which is the same shape of stale claim the
    /// module keeps finding elsewhere.)
    pub fatal: Vec<&'a str>,
    /// Diagnostics on our translation unit whose tag is in NEITHER list: a gap
    /// in this compiler's policy.
    pub unclassified: Vec<&'a str>,
    /// Fatal-tagged diagnostics attributed to some OTHER file — the C runtime,
    /// a system header, or a unit this parser could not attribute at all.
    ///
    /// ALSO STOPS THE BUILD. Not this program's defect, and refused anyway: the
    /// runtime is linked into every executable this compiler produces, so
    /// shipping it ill-typed puts the same miscompile in every program built
    /// here. Unattributable fatal diagnostics land here too, which is the
    /// fail-closed direction — refuse without naming a culprit.
    pub foreign: Vec<&'a str>,
}

/// What a diagnostic header says about severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Severity {
    Error,
    Warning,
    Note,
}

/// Parse this line as a diagnostic header ABOUT `tu`, and say how severe.
///
/// ONE PARSE, ONE ANSWER — and this shape is the fix for the same defect
/// relocating four times. Every previous version SEARCHED the raw line for the
/// thing it wanted (a substring of the path, then a base name, then `": error: "`
/// anywhere) and each search could be satisfied by text that is not the header
/// field it was standing in for. Two independent scans of one line also meant
/// the exit-0 path and the nonzero path could disagree about whose file it was.
///
/// So attribution is not a search at all. `link_command` puts the translation
/// units on the command line itself, and gcc echoes a path verbatim, so the
/// question "is this line a header for THAT unit" is answered by matching the
/// unit's exact path as a PREFIX at column 0 and then requiring gcc's header
/// grammar — `:<line>[:<col>]: <severity>: ` — immediately after it.
///
/// WHAT THAT CLOSES, in the order the review found them:
///
/// * **Basename collision.** `palladium_runtime.pd` compiles to
///   `build_output/palladium_runtime.c`, and `runtime_paths::RUNTIME_C_FILE` is
///   `palladium_runtime.c`. Identical base names, and the generated one is
///   chosen by whoever names the `.pd` file — so NO comparison ending at the
///   base name can separate the compiler's runtime from the user's program. The
///   full path can: the two differ in their directories, which is exactly what
///   we put on the command line.
/// * **A file name that contains a severity marker.** `foo: error: forged.pd`
///   is a legal name, so the ordinary prelude WARNING arrives as
///   `build_output/foo: error: forged.c:263:12: warning: …`. A scan for
///   `": error: "` finds it in the PATH, at column 0, past the echo anchor.
///   Here the marker is only ever read at the one offset the grammar puts it,
///   after the whole known path — so a name cannot supply it. Note the residual
///   is in the safe direction: a name could contain `:1:1: error: ` and cause
///   this parser to see NO header at all for that unit, which loses a
///   diagnostic rather than inventing an accusation.
///
/// The column-0 anchor is still here and still necessary — it is what excludes
/// gcc's indented source echo and caret lines — but it was never sufficient,
/// because a file name is at column 0 too and the user picks it.
fn header_severity(line: &str, tu: &Path) -> Option<Severity> {
    let tu = tu.to_str()?;
    // Column 0, exact path, nothing before it. Anything indented is an echo, a
    // caret, or an `In file included from` continuation.
    let rest = line.strip_prefix(tu)?.strip_prefix(':')?;

    // `<line>` then optionally `:<col>`, each a non-empty run of digits.
    let (digits, rest) = split_digits(rest)?;
    if digits.is_empty() {
        return None;
    }
    let rest = match rest.strip_prefix(':') {
        Some(after) => match split_digits(after) {
            Some((col, r)) if !col.is_empty() => r,
            // `path:12:` with no column — gcc does this for whole-file
            // diagnostics.
            _ => rest,
        },
        None => rest,
    };

    let rest = rest.strip_prefix(": ")?;
    for (word, sev) in [
        ("error: ", Severity::Error),
        ("warning: ", Severity::Warning),
        ("note: ", Severity::Note),
        // GNU emits these for -Werror promotions and ICEs; both are refusals.
        ("fatal error: ", Severity::Error),
        ("internal compiler error: ", Severity::Error),
    ] {
        if rest.starts_with(word) {
            return Some(sev);
        }
    }
    None
}

/// Split a leading run of ASCII digits from `s`.
fn split_digits(s: &str) -> Option<(&str, &str)> {
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    Some((&s[..end], &s[end..]))
}

/// Is this diagnostic a header about the translation unit we asked gcc to
/// compile?
fn attributed_to(line: &str, c_file: &Path) -> bool {
    header_severity(line, c_file).is_some()
}

/// Tags that say something about the CODE, as opposed to how the compiler was
/// configured.
///
/// `-Werror` is the only one of the second kind that gcc puts inside the
/// bracket, and it appears alongside the real tag (`[-Werror,-Wname]`). Without
/// this filter it would count as "a known tag on the line" and mask an unknown
/// one sitting beside it.
fn is_metadata_tag(tag: &str) -> bool {
    // `-Werror=name` is normalised to `-Wname` by `diagnostic_tags`, so only the
    // bare form reaches here.
    tag == "-Werror"
}

/// Is this tag escalated?
fn is_fatal_tag(tag: &str) -> bool {
    FATAL_DIAGNOSTIC_TAGS.contains(&tag)
}

/// Sort gcc's stderr into [`Classified`], relative to the file we asked about.
pub fn classify_diagnostics<'a>(stderr: &'a str, c_file: &Path) -> Classified<'a> {
    let mut out = Classified::default();
    for line in stderr.lines() {
        let tags = diagnostic_tags(line);
        if tags.is_empty() {
            continue;
        }
        let is_ours = attributed_to(line, c_file);
        let fatal_tag = tags.iter().any(|t| is_fatal_tag(t));
        if fatal_tag && is_ours {
            out.fatal.push(line);
        } else if fatal_tag {
            out.foreign.push(line);
        } else if is_ours
            // PER TAG, not per line. Asking whether ANY tag on the line is known
            // meant a line carrying one known-benign tag and one nobody has
            // decided about reported no gap — the unknown one hidden by the
            // company it keeps. Metadata tags are skipped rather than counted as
            // knowledge.
            && tags.iter().any(|t| {
                !is_metadata_tag(t)
                    && !NON_FATAL_DIAGNOSTIC_TAGS
                        .iter()
                        .any(|(known, _)| *known == t.as_str())
            })
        {
            out.unclassified.push(line);
        }
    }
    out
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
//
// THAT ARGUMENT BINDS THIS MODULE TOO, and it was violated here before it was
// noticed: `attributes_an_error_to` searched every line of stderr for
// `": error: "`, and gcc's echo of the generated source is one of those lines.
// A Palladium string literal reading `x.c:1:1: error: …` would have been parsed
// as a header and attributed, letting a fixture upgrade an honest 6 into an
// accusing 3. Eliminating a forgery surface at the output while opening one at
// the input is not a fix. Both parsers now anchor at column 0, which is where
// gcc puts a header and where an echo never starts.

// A NOTE ON WHAT AN EXIT CODE MAY PROMISE.
//
// The first version of this taxonomy mapped every nonzero gcc status to a code
// documented as "gcc rejected the translation unit this compiler emitted". The
// test behind it observed only that gcc returned nonzero. A full disk, an
// unwritable output path, a missing assembler, a gcc that crashed internally —
// all nonzero, none of them a rejection of our C.
//
// That is worse than the ambiguous sentence it replaced. An ambiguous sentence
// is read by a human who can doubt it; a structured code is read by a gate that
// cannot, and the sibling conformance gate is being built to turn this exact
// code into "that is a defect in this compiler". STRUCTURING A SIGNAL DOES NOT
// MAKE IT TRUER — it makes a false claim easier to consume, which is the defect
// this whole branch exists to close, reproduced one level in.
//
// So each code below promises exactly what was observed, and no more. Where
// evidence is a text heuristic it says so, and the heuristic is arranged to
// fail in the direction of NOT accusing.

/// gcc exited nonzero AND at least one `error:` diagnostic in its output is
/// attributed to the translation unit pdc handed it.
///
/// The one case in which "the backend emitted C that will not compile" is a
/// supportable accusation — and the support is a TEXT HEURISTIC over gcc's
/// stderr ([`attributes_an_error_to`]), not a fact the operating system told us.
/// It is sufficient, not necessary: a link-stage failure (an undefined symbol,
/// say) is a real backend defect that carries no `file:line: error:` for our .c
/// and therefore lands on [`EXIT_GCC_UNEXPLAINED`] instead. Under-claiming is
/// the deliberate direction. A gate may act on this code; it may not conclude
/// from its ABSENCE that the backend is innocent.
pub const EXIT_BACKEND_REJECT: i32 = 3;

/// gcc ran, exited 0, and diagnosed C that pdc had no business emitting.
///
/// Distinct from [`EXIT_BACKEND_REJECT`] because gcc did NOT refuse: a binary
/// was produced (and has been deleted). A gate that treats the two as one
/// cannot tell "we were stopped" from "we were about to ship a segfault".
pub const EXIT_BACKEND_ILL_TYPED: i32 = 4;

/// gcc never reached a verdict: it could not be spawned, the runtime could not
/// be located, or it terminated abnormally.
///
/// A statement about the machine, never about the compiler or the program.
pub const EXIT_TOOLCHAIN: i32 = 5;

/// gcc ran to completion, exited nonzero, and pdc could not establish why.
///
/// THE HONEST BUCKET. gcc gave a verdict and pdc cannot show that the verdict
/// was about our C: no `error:` line names the translation unit. Disk full,
/// output path unwritable, an assembler or linker component missing, an ICE in
/// gcc itself, or a rejection whose diagnostics this parser could not read.
///
/// It is still a FAILURE — nothing was built and no caller should proceed — but
/// it is not an accusation, and a gate must not turn it into one.
///
/// NOT EXOTIC, and planning for it as a rarity would be wrong. An undefined
/// symbol from the LINK stage is an ordinary codegen defect — codegen emitted a
/// call to something it never defined — and it carries no `file:line` naming our
/// translation unit, so it is unattributable and lands here rather than on 3.
/// This code is the resting place of a whole defect class, not a corner.
///
/// THE TENSION IT CREATES, NAMED BECAUSE IT IS A TRADEOFF AND NOT AN OVERSIGHT.
/// [`LinkError::GccUnexplained`] deliberately OMITS the `gcc compilation failed`
/// marker, because a gate grepping for that marker reads it as "gcc refused our
/// C" — a claim nobody supported here. That omission is right, and it is
/// precisely what blinds `scripts/conformance.sh`'s contradiction check, which
/// greps for the same marker to catch "the log says gcc ran but no translation
/// unit exists". With the .c missing or mis-derived, a 6 would fall through that
/// check into the front-end arm, where `compile` is a stage a manifest row may
/// declare — the conjunction fail-open the sibling branch closed, reopened by a
/// code its consumers had never heard of.
///
/// The resolution is the structured code, which is why 6 is enrolled in BOTH
/// consumers rather than left to be inferred: `scripts/conformance.sh`'s
/// `backend_code` case and its own verdict arm, and `scripts/gate_probe.py`'s
/// `BACKEND_CODES` and `PDC_REJECT_CODES`. A code the consumer has never heard
/// of is worse than no code at all.
pub const EXIT_GCC_UNEXPLAINED: i32 = 6;

// Why these numbers. `1` is pdc's existing "something went wrong" and every
// front-end refusal already uses it — reusing it would put the new distinction
// back inside the old one. `2` is spoken for twice over: clap exits 2 on a
// usage error, and `make` collapses every nonzero recipe status to 2, so a
// gate reading `make`'s status could not tell 2 from 2. 3-6 are unused here.

/// Why a link did not produce a usable executable.
///
/// FIVE causes, kept apart because they support five different accusations.
/// Collapsing any two of them lets a gate blame the compiler for the machine —
/// or blame it for something nobody established — which is the specific failure
/// this enum exists to prevent.
#[derive(Debug)]
pub enum LinkError {
    /// gcc could not be spawned, or the runtime could not be located. gcc never
    /// looked at the C.
    Toolchain(String),
    /// gcc started and terminated abnormally (a signal on Unix; any absent exit
    /// code elsewhere). It never reached a verdict either, so it belongs with
    /// `Toolchain` and NOT with a rejection — an abnormally terminated process
    /// also has a nonzero status, which is exactly how it used to be reported
    /// as one.
    GccAbnormal(String),
    /// gcc exited nonzero and attributed an `error:` to the C we emitted. The
    /// message is byte-identical to the one pdc printed before this change,
    /// because `scripts/conformance.sh` matches on it.
    GccRejected(String),
    /// gcc exited nonzero and nothing establishes that our C was the reason.
    /// Deliberately does NOT carry the `gcc compilation failed` marker: a gate
    /// grepping for it would be reading a claim nobody supported.
    GccUnexplained(String),
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
            LinkError::Toolchain(_) | LinkError::GccAbnormal(_) => EXIT_TOOLCHAIN,
            LinkError::GccRejected(_) => EXIT_BACKEND_REJECT,
            LinkError::GccUnexplained(_) => EXIT_GCC_UNEXPLAINED,
            LinkError::IllTypedC(_) => EXIT_BACKEND_ILL_TYPED,
        }
    }
}

impl std::fmt::Display for LinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinkError::Toolchain(m)
            | LinkError::GccAbnormal(m)
            | LinkError::GccRejected(m)
            | LinkError::GccUnexplained(m)
            | LinkError::IllTypedC(m) => {
                write!(f, "{}", m)
            }
        }
    }
}

impl std::error::Error for LinkError {}

/// Does gcc's output attribute an `error:` to the file we handed it?
///
/// THIS IS A TEXT HEURISTIC AND IT IS LABELLED AS ONE. It looks for a
/// diagnostic header of gcc's usual `<path>:<line>:<col>: error:` shape whose
/// path is the translation unit under compilation. Attribution is
/// [`attributed_to`] — the SAME function the exit-0 path uses, parsing the
/// header once and comparing whole path components. It used to have its own
/// copy that split on the first `:` and asked `ends_with`, which is how a
/// diagnostic about `runtime/palladium_runtime.c` could be attributed to a
/// translation unit called `runtime.c`.
///
/// FAILS TOWARDS SILENCE, ON PURPOSE. Every way this can be wrong — a localized
/// gcc, a colourised gcc, a linker-stage error with no `file:line`, a
/// diagnostic format a future release changes — makes it answer `false`, which
/// downgrades a rejection to [`LinkError::GccUnexplained`]. A missed accusation
/// costs a sharper message; a fabricated one costs a gate certifying a defect
/// that was never shown.
pub fn stderr_rejects(stderr: &str, c_file: &Path) -> bool {
    // The SEVERITY COMES FROM THE PARSE, not from a second scan of the line.
    // `line.contains(": error: ")` was the whole bug: a file named
    // `foo: error: forged.pd` puts that substring in the PATH of an ordinary
    // warning, at column 0, and the accusation became a property of the file
    // name. One parser answers both "whose file" and "how severe", so the two
    // can no longer be sourced from different text.
    stderr
        .lines()
        .any(|line| header_severity(line, c_file) == Some(Severity::Error))
}

/// How a process ended, when it did not end by exiting.
///
/// `ExitStatus::success()` is false for an abnormally terminated child exactly
/// as it is for a child that exited 1, which is why the old code could not tell
/// them apart. This is the real mechanism rather than a heuristic over stderr:
/// a killed gcc may have printed nothing at all, and "no diagnostics" is not
/// evidence of anything.
///
/// PORTABLE, unlike the first version of this function. That one returned the
/// Unix signal number and answered `None` everywhere else, so on a non-Unix
/// platform a killed gcc was necessarily classified as a rejection — the same
/// false accusation the taxonomy exists to prevent, quietly reintroduced by a
/// `cfg` fallback. The portable fact is `ExitStatus::code() == None`, i.e. the
/// process did not exit; the SIGNAL NUMBER is the Unix-only refinement, and it
/// only ever makes the sentence more specific.
fn abnormal_end(status: &std::process::ExitStatus) -> Option<String> {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = status.signal() {
            return Some(format!("killed by signal {}", sig));
        }
    }
    if status.code().is_none() {
        // No exit code and (on Unix) no signal either: the platform is telling
        // us the process did not exit normally without saying how. Still not a
        // verdict, which is the only thing the caller needs.
        return Some("terminated abnormally, without an exit code".to_string());
    }
    None
}

/// The pdc-level diagnostic for C the backend should never have emitted.
///
/// Phrased as an internal compiler error and not as a gcc dump: the reader of
/// this message wrote Palladium, the text gcc is objecting to is C they never
/// saw, and telling them "incompatible pointer types" without saying whose
/// mistake it is invites them to go looking for it in their own source.
/// What each fatal tag actually means for the program, in one clause.
///
/// TAG-SPECIFIC BECAUSE THE STORY IS. The first version told every reader that
/// "the callee dereferences one level too far and the program crashes at
/// runtime" — true for the pointer-depth witness this branch started from, and
/// FALSE for the other three: an undeclared call is compiled against an invented
/// signature, an int/pointer conversion truncates or fabricates an address, a
/// mismatched function pointer calls through the wrong ABI. Handing all four the
/// same causal sentence is asserting a mechanism that was not observed, in the
/// diagnostic whose job is to report what was.
fn consequence_of(tag: &str) -> &'static str {
    match tag {
        "-Wincompatible-pointer-types" => {
            "a pointer is passed or assigned at the wrong indirection level, so \
             the receiving code dereferences one level too far"
        }
        "-Wint-conversion" => {
            "an integer is used where a pointer is required (or the reverse), so \
             the value is treated as an address it never was"
        }
        "-Wincompatible-function-pointer-types" => {
            "a function is called through a pointer of a different signature, so \
             the arguments and return value are read under the wrong ABI"
        }
        "-Wimplicit-function-declaration" => {
            "a function is called that was never declared, so C invents a \
             signature for it and the call is compiled against that guess"
        }
        _ => "the emitted C is ill-typed",
    }
}

/// The fatal tags present on a set of diagnostic lines, in order, deduplicated.
fn fatal_tags_in(diagnostics: &[&str]) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for line in diagnostics {
        for tag in diagnostic_tags(line) {
            if let Some(known) = FATAL_DIAGNOSTIC_TAGS.iter().find(|k| **k == tag.as_str()) {
                if !out.contains(known) {
                    out.push(known);
                }
            }
        }
    }
    out
}

/// Who owns the ill-typed C: the file compiled from the user's program, or a
/// translation unit this compiler ships.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IllTypedOwner {
    /// The `.c` codegen wrote for this program.
    GeneratedC,
    /// `runtime/palladium_runtime.c` or a header it pulls in — shipped with the
    /// compiler, identical for every program, and nothing the user wrote.
    Runtime,
}

/// The pdc-level diagnostic for ill-typed C that this compiler is responsible
/// for, whichever of its own translation units it came from.
///
/// Phrased as an internal compiler error and not as a gcc dump: the reader of
/// this message wrote Palladium, the text gcc is objecting to is C they never
/// saw, and telling them "incompatible pointer types" without saying whose
/// mistake it is invites them to go looking for it in their own source.
pub fn ill_typed_c_error(
    c_file: &Path,
    diagnostics: &[&str],
    owner: IllTypedOwner,
    binary_note: &str,
) -> String {
    let mut msg = String::new();
    msg.push_str("internal compiler error: ");
    match owner {
        IllTypedOwner::GeneratedC => {
            msg.push_str("the C backend emitted ill-typed C\n");
            msg.push_str(&format!(
                "\n  gcc accepted {} (it exited 0) but diagnosed ill-typed C in\n  \
                 it. That C is never what pdc meant to emit — no Palladium program\n  \
                 can ask for it — so this is a defect in the compiler, not in your\n  \
                 program.\n\n",
                c_file.display()
            ));
        }
        IllTypedOwner::Runtime => {
            msg.push_str("this compiler ships ill-typed C in its own runtime\n");
            msg.push_str(
                "\n  gcc exited 0 while compiling a translation unit that pdc SHIPS —\n  \
                 the C runtime, not the file generated from your program. Your\n  \
                 source is not the defect and nothing about it is being questioned.\n  \
                 The build is refused anyway: the runtime is linked into every\n  \
                 executable this compiler produces, so shipping it would put the\n  \
                 same miscompile in every program built here, including this one.\n\n",
            );
        }
    }

    for tag in fatal_tags_in(diagnostics) {
        msg.push_str(&format!("  {}: {}.\n", tag, consequence_of(tag)));
    }
    msg.push('\n');
    for d in diagnostics {
        msg.push_str(&format!("  {}\n", d.trim()));
    }
    msg.push_str(binary_note);
    match owner {
        IllTypedOwner::GeneratedC => msg.push_str(&format!(
            "\n  Please report this, with your Palladium source and the line of\n  \
             generated C named above. Full context: gcc -c {} -I <pdc --print-runtime>\n",
            c_file.display()
        )),
        IllTypedOwner::Runtime => msg.push_str(
            "\n  Please report this against the compiler. Nothing in your program\n  \
             needs to change. Full context: gcc -c <pdc --print-runtime>/palladium_runtime.c\n",
        ),
    }
    msg
}

/// Print the notes [`link`] hands back, if any.
///
/// One function so that every call site reports a policy gap the same way, and
/// so that "the registry is consulted" is one grep rather than five. Silent on
/// an empty list, which is the measured case for the whole corpus today.
pub fn report_notes(notes: &[String]) {
    for n in notes {
        // `note:` and not `error:`/`warning:` — this is not a defect in the
        // program being compiled, and `scripts/conformance.sh` builds its
        // one-line detail by grepping the log for `error`.
        eprintln!("\x1b[1;33mnote:\x1b[0m {}", n);
    }
}

/// Run the gcc invocation, and ask it every question it can answer.
///
/// `link_command` builds the command; this runs it and decides what the run
/// means. Every caller that wants an executable should use this rather than
/// `.output()` on the command: `.output()` hands back a status and a buffer,
/// and turning those two into a verdict is the part that was being got wrong.
///
/// The four questions, in the only order that works:
///   1. did gcc reach a verdict at all?   (spawn failure, abnormal termination)
///   2. if it said no, was it about OUR C? (attribution, and it is a heuristic)
///   3. what did gcc find on the way?      (stderr, even at exit 0)
///   4. did it find anything nobody has a policy for? (returned, not swallowed)
///
/// Returns the unclassified diagnostics on success. They are not an error — a
/// tag this compiler has never decided about must not break a build — but they
/// are not nothing either, and handing them back is what stops the registry in
/// [`NON_FATAL_DIAGNOSTIC_TAGS`] from being decoration. Callers report them.
pub fn link(
    c_file: &Path,
    output: &Path,
    opt: OptLevel,
) -> std::result::Result<Vec<String>, LinkError> {
    let gcc_output = link_command(c_file, output, opt)
        .map_err(|e| LinkError::Toolchain(e.to_string()))?
        // LC_ALL=C so gcc speaks the language `diagnostic_tags` reads. A
        // localized toolchain would carry the fatal tag in a line this parser
        // matches nothing in, and "matched nothing" is indistinguishable from
        // "gcc was happy" — a silent non-fatal, which is the original defect
        // wearing a different hat. Colour needs no flag: `output()` captures
        // through a pipe and both gcc and clang default to colour=auto.
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output()
        .map_err(|e| LinkError::Toolchain(format!("Failed to run gcc: {}", e)))?;

    let stderr = String::from_utf8_lossy(&gcc_output.stderr);
    let stderr = stderr.as_ref();

    // Question one: did gcc reach a verdict? An abnormally terminated gcc is
    // nonzero too, so this MUST precede the status check or the answer to
    // question two is a guess. Nothing here is inferred from stderr: a killed
    // process may have printed nothing, and silence is not evidence.
    if let Some(how) = abnormal_end(&gcc_output.status) {
        return Err(LinkError::GccAbnormal(format!(
            "gcc did not finish: {} while compiling {}.\n  \
             gcc never reached a verdict, so nothing here is known about that C —\n  \
             this is a fact about the machine (out of memory, a killed process\n  \
             group), not about the compiler or the program. Anything gcc managed\n  \
             to print before it died follows.\n{}",
            how,
            c_file.display(),
            stderr
        )));
    }

    // Question two: gcc said no — but about what? Only an `error:` attributed
    // to the translation unit we handed it supports "the backend emitted C that
    // will not compile". Everything else exits nonzero too and means something
    // else entirely, so it gets a code that accuses nobody.
    if !gcc_output.status.success() {
        if stderr_rejects(stderr, c_file) {
            // Byte-identical to the message pdc printed before this change.
            return Err(LinkError::GccRejected(format!(
                "gcc compilation failed:\n{}",
                stderr
            )));
        }
        return Err(LinkError::GccUnexplained(format!(
            "gcc exited {} without diagnosing {}.\n  \
             gcc gave a verdict, and pdc cannot show the verdict was about the C\n  \
             it emitted: no error in gcc's output names that file. A full disk, an\n  \
             unwritable output path, a missing assembler or linker component, or an\n  \
             internal error in gcc itself all look like this. Nothing was built.\n  \
             This is NOT evidence that the generated C is bad, and it is not\n  \
             evidence that it is good. gcc's output follows.\n{}",
            gcc_output
                .status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "nonzero".to_string()),
            c_file.display(),
            stderr
        )));
    }

    // Question three: what did gcc find? This is the half that was discarded.
    let Classified {
        fatal,
        unclassified,
        foreign,
    } = classify_diagnostics(stderr, c_file);
    // ILL-TYPED C IS REFUSED WHICHEVER OF OUR TRANSLATION UNITS IT IS IN.
    //
    // `foreign` was introduced to stop a defect in the C runtime being reported
    // as a defect in the user's program. It then RETURNED THAT DIAGNOSTIC AS A
    // NOTE and let the caller ship the executable — so a pointer-depth defect in
    // `palladium_runtime.c` stayed runnable purely because it was not the user's
    // fault. The review that found it named the shape exactly: the fix asked for
    // was "do not blame the user's file", and what got built was "do not tell
    // anyone", which is the original bug — a real diagnostic reaching a branch
    // that discards it — for the third time on this branch.
    //
    // Ownership changes the SENTENCE, never the VERDICT. The runtime is linked
    // into every executable this compiler produces; shipping it ill-typed puts
    // the same miscompile in every program built here.
    if !fatal.is_empty() || !foreign.is_empty() {
        let (lines, owner) = if !fatal.is_empty() {
            (fatal, IllTypedOwner::GeneratedC)
        } else {
            (foreign, IllTypedOwner::Runtime)
        };

        // gcc exited 0, so the executable EXISTS and it is the one that
        // segfaults. Reporting an error while leaving it on disk would let the
        // next `./prog` run the miscompile we just refused to ship.
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

        return Err(LinkError::IllTypedC(ill_typed_c_error(
            c_file,
            &lines,
            owner,
            &binary_note,
        )));
    }

    // Question four: anything nobody has decided about? Handed back, not
    // swallowed. See `NON_FATAL_DIAGNOSTIC_TAGS`.
    let notes: Vec<String> = unclassified
        .into_iter()
        .map(|line| {
            format!(
                "gcc emitted a diagnostic pdc has no policy for. It was NOT \
                 treated as fatal.\n    {}\n    Classify its tag in \
                 FATAL_DIAGNOSTIC_TAGS or NON_FATAL_DIAGNOSTIC_TAGS \
                 (src/linker.rs).",
                line.trim()
            )
        })
        .collect();
    Ok(notes)
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

    /// The translation unit `REAL_STDERR` is about.
    fn ours() -> &'static Path {
        Path::new("build_output/B3.c")
    }

    #[test]
    fn the_type_confusion_is_found_in_real_gcc_output() {
        let found = fatal_diagnostics(REAL_STDERR, ours());
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
            fatal_diagnostics(&prelude_only, ours()).is_empty(),
            "the prelude's discards-qualifiers warning was escalated; it is in \
             every compile, so this refuses every program: {:?}",
            fatal_diagnostics(&prelude_only, ours())
        );
    }

    /// Clean gcc output is clean. Guards the direction where a bug in the
    /// scanner refuses programs gcc never said anything about.
    #[test]
    fn silence_and_untagged_lines_are_not_diagnostics() {
        assert!(fatal_diagnostics("", ours()).is_empty());
        assert!(fatal_diagnostics("2 warnings generated.\n", ours()).is_empty());
        // A `note:` continuation and an echoed source line that happens to end
        // in a bracket. Neither is a diagnostic header.
        let noise = "x.c:1:1: note: passing argument to parameter 's' here\n\
                     x.c:2:2: warning: something else [-Wunused-variable]\n\
                     int a[] = { [-Wincompatible-pointer-types]\n";
        assert!(fatal_diagnostics(noise, Path::new("x.c")).is_empty());
    }

    /// gcc under `-Werror` spells the tag `[-Werror,-Wname]`. pdc does not pass
    /// `-Werror` today (see `NON_FATAL_DIAGNOSTIC_TAGS`), but the day a caller
    /// does, the tag must still be recognised rather than silently stop
    /// matching.
    #[test]
    fn a_comma_separated_werror_tag_still_matches() {
        let line =
            "x.c:1:1: error: incompatible pointer types [-Werror,-Wincompatible-pointer-types]";
        assert_eq!(fatal_diagnostics(line, Path::new("x.c")).len(), 1);
    }

    /// THE RUNTIME IS NOT THE USER'S FILE.
    ///
    /// `link_command` puts two translation units on the command line. Before
    /// the path filter, a pointer mismatch introduced in
    /// `runtime/palladium_runtime.c` would have made EVERY compile of EVERY
    /// program die with an internal compiler error that named the user's `.c`
    /// while quoting a diagnostic that named the runtime's — the exact
    /// "born red for every program" outcome the `-Werror` reasoning above
    /// exists to avoid, arriving through a door that reasoning did not check.
    #[test]
    fn a_fatal_diagnostic_in_the_runtime_does_not_condemn_the_users_program() {
        let stderr = "\
runtime/palladium_runtime.c:12:5: warning: incompatible pointer types assigning to 'char **' from 'char *' [-Wincompatible-pointer-types]
build_output/B3.c:263:12: warning: returning 'const char *' from a function with result type 'char *' discards qualifiers [-Wincompatible-pointer-types-discards-qualifiers]
";
        let c = classify_diagnostics(stderr, ours());
        assert!(
            c.fatal.is_empty(),
            "a defect in the C runtime was charged to the user's program: {:?}",
            c.fatal
        );
        assert_eq!(c.foreign.len(), 1, "{:?}", c);
        assert!(c.foreign[0].contains("palladium_runtime.c"), "{:?}", c);
        // And the user's own fatal diagnostic still lands, so the filter did
        // not simply switch the check off.
        let both = format!(
            "{}build_output/B3.c:279:18: warning: incompatible pointer types passing 'const char *' to parameter of type 'const char **' [-Wincompatible-pointer-types]\n",
            stderr
        );
        let c2 = classify_diagnostics(&both, ours());
        assert_eq!(c2.fatal.len(), 1, "{:?}", c2);
        assert_eq!(c2.foreign.len(), 1, "{:?}", c2);
    }

    /// A tag in neither list is a POLICY GAP, and it is reported rather than
    /// passed over.
    ///
    /// The first version of this module had no such bucket: `fatal_diagnostics`
    /// consulted only the denylist, so every unknown tag was silently
    /// non-fatal and `NON_FATAL_DIAGNOSTIC_TAGS` was read by nothing but a test
    /// asserting its comments were long enough. This is what makes membership
    /// load-bearing.
    #[test]
    fn a_tag_in_neither_list_is_reported_as_a_gap() {
        let line = "build_output/B3.c:9:9: warning: something nobody classified [-Wnobody-decided]";
        let c = classify_diagnostics(line, ours());
        assert!(c.fatal.is_empty());
        assert_eq!(c.unclassified.len(), 1, "{:?}", c);

        // A tag that IS classified is not a gap — otherwise every compile in
        // the tree reports one, and the report becomes noise nobody reads.
        let known =
            "build_output/B3.c:263:12: warning: discards qualifiers [-Wincompatible-pointer-types-discards-qualifiers]";
        assert!(classify_diagnostics(known, ours()).unclassified.is_empty());
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
        let found = fatal_diagnostics(REAL_STDERR, ours());
        let msg = ill_typed_c_error(
            Path::new("build_output/B3.c"),
            &found,
            IllTypedOwner::GeneratedC,
            "\n  removed\n",
        );
        assert!(msg.contains("internal compiler error"), "{}", msg);
        assert!(msg.contains("not in your\n  program"), "{}", msg);
        // The causal clause must be the one belonging to the tag that was
        // actually found, not a story borrowed from another class.
        assert!(msg.contains("indirection level"), "{}", msg);
        assert!(msg.contains("build_output/B3.c:279:18"), "{}", msg);
        assert!(msg.contains("removed"), "{}", msg);
        // `scripts/conformance.sh` greps the compiler log for `error` to build
        // the one-line detail it reports. A message it cannot see is a blank
        // column in the gate output.
        assert!(msg.contains("error"), "{}", msg);
    }
}
