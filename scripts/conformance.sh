#!/usr/bin/env bash
# Palladium conformance runner.
#
# Compiles + links + runs every .pd fixture and checks the result against a
# CLOSED INVENTORY: tests/conformance-manifest.txt declares every fixture and
# what it is expected to do. The gate answers "is the corpus green", not
# "did nothing I happened to look at fail" — those differ whenever the runner
# can be made to look at less, and every way of looking at less is an error here:
#
#   * a discovered fixture that is not declared        -> UNDECLARED   (fail)
#   * a declared fixture that was not discovered       -> MISSING      (fail)
#   * a scope that cannot be enumerated, or is empty   -> fatal        (exit 2)
#   * a manifest that does not exist                   -> fatal        (exit 2)
#
# Classes (column 2):
#   run      must compile, link, run with exit 0, AND its stdout must match the
#            sibling <fixture>.expected transcript byte for byte. Column 4 is
#            always `expected`; there is no exit-code-only spelling, because that
#            was a documented bypass of the protection below.
#   untranscribed
#            ran and exited 0, but carries NO transcript. This is the reviewed
#            allowance for a fixture that genuinely cannot have one (nondetermin-
#            istic output, a timestamp, a machine-dependent value). It is shaped
#            exactly like an xfail row — owner plus a `why:` reason — and is
#            reported as a debt on every run, so "no transcript" is a written
#            decision rather than a default.
#
#            Be precise about how much that is worth: `owner` is an editable
#            label, so reassigning a row to another milestone or to `unscheduled`
#            slips a specific CONFORMANCE_FORBID_OWNER check. The authorisation
#            boundary is REVIEW OF THIS MANIFEST, not the runner. Printing a debt
#            creates visibility; it does not create mechanical pressure, and this
#            file cannot tell an honest reclassification from an evasive one.
#
#            Why the distinction is load-bearing: a missing C `return` is
#            undefined behaviour, so a tail-return miscompile (defect D3) prints
#            garbage and still exits 0. Measured here, `long long f(a,b){(a+b);}`
#            returns 8261746944 with exit 0 at BOTH -O0 and -O2. An exit-code
#            verdict cannot tell that from a correct program. That is how D3
#            miscompiled stdlib/ for a year underneath a green gate.
#   vacuous  runs, but only prints that its feature is unimplemented. Counted
#            apart from `pass` so a green run is never mistaken for coverage.
#            The classification is DECLARED here, not inferred from the file.
#            Its note must begin `claims:` and name the feature the file appears
#            to cover but does not — "vacuous" alone does not tell a future
#            reader that async conformance is at zero.
#   xfail    known to fail, with an expected STAGE and a DIAGNOSTIC FINGERPRINT.
#            A failure at a different stage, or with a different diagnostic, is
#            NOT the declared failure and fails the gate — otherwise a fresh bug
#            could hide behind an old excuse. A listed fixture that PASSES is
#            XPASS and fails the gate, so an expectation cannot go stale.
#            An xfail is a DEBT: it carries the milestone that owes it.
#   reject   must fail, and that is CORRECT behaviour — a negative test. Same
#            stage+fingerprint machinery as xfail, opposite meaning: it is real
#            coverage, owed to nobody, and counted as such. This is how you test
#            "the compiler refuses `.await` with a span-carrying diagnostic"
#            instead of shipping a program that prints prose about it. If the
#            compiler ACCEPTS it, that is REJECT_ACCEPTED and fails the gate.
#   skip     not a standalone program (no `fn main`): a library module or a
#            package manifest. Declared, so it cannot be used to smuggle a
#            program out of the gate.
#
# ONE OUTCOME IS NOT A CLASS AT ALL — the backend rejecting its own output.
# There is no valid Palladium program for which `pdc` accepts the source and gcc
# then refuses the C that codegen emitted. If the front end said yes, C that does
# not compile is a defect in this compiler, always: never a property of the
# input, and never something a fixture may declare as its expected behaviour. So
# the runner does not CLASSIFY that outcome, it REFUSES it — it fails
# unconditionally whatever the manifest says (see the verdict section), and stage
# `link` is a manifest error so the excuse cannot be reintroduced as a column
# value. Same shape as NO_BINARY below, same reason.
#
# WHAT IT IS CALLED depends on how much the producer established, and since
# fix/gcc-diagnostics-discarded landed it establishes a lot. `BACKEND_REJECT`
# fires on exit 3 (gcc refused our C) and on exit 4 (gcc exited 0 and diagnosed
# ill-typed C we emitted). `HARNESS_ERROR` fires on 5 (gcc never reached a
# verdict) and on 6 (it reached one nobody could attribute to our C). All four
# are never-expectable and all four fail the gate — the invariant is ENFORCED
# either way; only the accusation is withheld. See the verdict section.
#
# Be exact about the size of that claim. This makes the outcome INADMISSIBLE FOR
# EVERY FIXTURE THE CORPUS RUNS. It does not find backend-reject defects: a
# corpus-driven gate can only ever confirm what someone already thought to write
# down, so a program nobody added is still unprotected. Measured: the two
# reproductions that motivated this check (a struct field of enum type; a nested
# array local) are BOTH absent from this corpus, so this gate would not have
# caught either one. Corpus coverage is a separate, still-open debt.
#
# AND THIS FILE IS NOT THE ONLY PLACE THE OUTCOME IS DECLARABLE. Four surfaces
# carried it; this branch closes the first two and leaves the other two standing,
# so the residue is named rather than described as one stray classifier:
#
#   1 scripts/gate_probe.py `pdc-reject`/`pdc-verdict`   CLOSED with this change
#     — classified the same outcome by the same forgeable log grep.
#   2 scripts/gate_probe.py `--expect-stage`             CLOSED with this change
#     — `link` was an offered CLI choice, so a caller could declare it and be
#     told `rejected-as-expected`.
#   3 stdlib/MANIFEST.tsv:22 `LINK_FAIL`                 CLOSED with this change
#     — "accepted by the compiler, rejected by gcc", a verdict in the vocabulary
#     scripts/stdlib-gate.sh enforces. Zero rows used it (all 21 are
#     COMPILE_FAIL). Nothing emits it now, AND a row naming it is refused by name
#     rather than merely failing to match.
#   4 tests/stdlib/BUILTINS.tsv:38 `UNUSABLE` + its stage column  PARTLY OPEN
#     — the stage is passed straight from the manifest column into gate_probe
#     (scripts/stdlib-gate.sh:472-473), and `--expect-stage link` is now refused
#     at parse time, so such a row fails. But the PROSE in BUILTINS.tsv still
#     documents the class as "rejected by the C compiler", and that file is
#     outside this branch's file scope, so the spelling survives there. Zero rows
#     use it today.
#
# 4 is listed rather than fixed so the branch does not claim a closure it did not
# make; the residue is one comment line in a file this branch may not touch.
#
# THE HISTORY IS NOT HYPOTHETICAL, and it already agrees with this invariant.
# tests/stdlib/BUILTINS.tsv:40 records "THE UNUSABLE CLASS IS NOW EMPTY, AND IT
# HELD SIX", and :50-54 that on 2026-08-22 all six went red with "expected
# rejection at link, got compile" (:53) — because the repair was to make the type
# checker refuse, so the program never reached gcc at all. That is this rule,
# applied by hand, six times, while the declarable spelling was left standing.
#
# Manifest format: 6 TAB-separated columns, every column non-empty, `-` = N/A.
#   1 path         repo-root-relative. Tabs are the delimiter, so spaces are safe.
#   2 class        run | untranscribed | vacuous | xfail | reject | skip
#   3 stage        compile | run                 (xfail/reject/skip only, else `-`)
#                  `link` is REFUSED — see the paragraph above.
#   4 observable   what must be observed, per class:
#                    run           `expected` (diff sibling <fixture>.expected)
#                    xfail/reject  a substring of the diagnostic, or exit=<N>
#                                  when stage=run
#                    others        `-`
#   5 owner        M1..M9 | unscheduled
#                                  (untranscribed/vacuous/xfail only, else `-`)
#   6 note         free text, required except for class=run;
#                  must begin `claims:` for class=vacuous
#                  must begin `why:`    for class=untranscribed
#
# ON TRANSCRIPTS: a .expected file is a CONTRACT, not a recording. `diff`ing it
# proves the compiler still does what it did, which is only worth something if
# what it did was right. A change to a golden is therefore a change to the
# expected behaviour of the language and must be reviewed SEPARATELY from, and
# more carefully than, the compiler change that motivated it — a diff that
# "just updates the goldens" is a silent respecification. Prefer transcripts
# whose correctness can be checked by reading the fixture (a known factorial, a
# known Fibonacci) over ones that merely record whatever was printed.
#
# Env:
#   CONFORMANCE_MANIFEST       override the manifest path (used by the runner's
#                              own regression tests)
#   CONFORMANCE_FORBID_OWNER   fail if any evaluated untranscribed/vacuous/xfail
#                              entry is owned by this milestone. This is what
#                              turns a milestone exit criterion into a command:
#                                CONFORMANCE_FORBID_OWNER=M1 make conformance
#   CONFORMANCE_BLESS=1        rewrite every transcript from THIS build's output.
#                              Never exits 0, and refuses to run under CI, so it
#                              cannot be mistaken for or automated into a pass.
#
# Usage: scripts/conformance.sh [scope ...]     (default: tests examples)

set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

PDC=./target/release/pdc
OUT_DIR=build_output
MANIFEST=${CONFORMANCE_MANIFEST:-tests/conformance-manifest.txt}
FORBID_OWNER=${CONFORMANCE_FORBID_OWNER:-}

if [ ! -x "$PDC" ]; then
  echo "error: $PDC not built. Run: cargo build --release" >&2
  exit 2
fi
if [ ! -f "$MANIFEST" ]; then
  echo "error: manifest '$MANIFEST' not found. The conformance corpus is a closed" >&2
  echo "       inventory; without it the gate cannot know what is missing." >&2
  exit 2
fi

# Blessing rewrites the definition of "correct" from whatever the compiler just
# did. That is a human judgement, so it may not happen on a machine whose whole
# job is to answer yes/no without one.
if [ "${CONFORMANCE_BLESS:-0}" = "1" ] && [ -n "${CI:-}${GITHUB_ACTIONS:-}" ]; then
  echo "error: CONFORMANCE_BLESS is not permitted under CI." >&2
  echo "       A transcript records what the language is supposed to do; it must" >&2
  echo "       be changed deliberately and reviewed, never regenerated by a bot." >&2
  exit 2
fi

TMPROOT=$(mktemp -d) || exit 2
trap 'rm -rf "$TMPROOT"' EXIT

# Tidy a path emitted by `find` (leading ./, doubled slashes) into a manifest key.
# This is only lexical, and is only ever applied to output of a scope that has
# already been resolved physically below.
canon() {
  printf '%s' "$1" | sed -e 's|^\./||' -e 's|//*|/|g' -e 's|/\./|/|g' -e 's|/*$||'
}

strip_ansi() { sed $'s/\033\\[[0-9;]*m//g'; }

# The shared diagnostic-header parser (GI-12). SOURCED, never copied: this file
# and scripts/check-diagnostic-codes.sh must not be able to disagree about what a
# coded header is. Today it is OBSERVATIONAL here — the reject verdict below is
# still the whole-log fixed-string match, and the code pins arrive at the
# cutover — so what it buys now is that the plumbing is exercised on the live
# corpus every run instead of only in the gate's own fixtures.
. scripts/lib/diag-parse.sh
diag_coded=0; diag_uncoded=0; diag_malformed=0; diag_unreadable=0

# grep answers three questions, not two: 0 matched, 1 did not match, 2 COULD NOT
# LOOK. Collapsing 2 into "did not match" is how an unreadable fixture became a
# harmless `skip` and a dangling symlink passed as a non-program. Every consumer
# of grep's status in this file goes through here and must handle 2 explicitly.
#   rc 0 = matched, 1 = no match, 2 = error (unreadable, missing, bad pattern)
grep_status() {
  local mode=$1 pat=$2 file=$3
  case "$mode" in
    E) grep -qE -- "$pat" "$file" 2>/dev/null ;;
    F) grep -qF -- "$pat" "$file" 2>/dev/null ;;
  esac
  return $?
}

# Literal prefix test. `grep -q "^$d/"` interpreted the scope name as a regular
# expression, so an EMPTY scope named `fooba.` matched a populated `foobar/…` and
# evaded the fatal empty-scope check. A quoted expansion in a `case` pattern is
# literal, which is why this form is used throughout instead of grep.
has_prefix() {   # has_prefix <prefix> <string>
  case "$2" in "$1"*) return 0 ;; esac
  return 1
}

# Resolve a FILE to its physical path, following a chain of symlinks. Portable:
# no readlink -f (absent on older macOS), bounded so a cycle cannot hang.
resolve_file() {
  local p=$1 d b i=0
  while [ -L "$p" ] && [ "$i" -lt 32 ]; do
    d=$(dirname "$p"); b=$(readlink "$p")
    case "$b" in /*) p=$b ;; *) p="$d/$b" ;; esac
    i=$((i+1))
  done
  d=$(cd "$(dirname "$p")" 2>/dev/null && pwd -P) || return 1
  printf '%s/%s' "$d" "$(basename "$p")"
}

# --------------------------------------------------------------------------
# Scopes
# --------------------------------------------------------------------------
# Scope arguments are resolved PHYSICALLY, not by string cleanup: `cd` + `pwd -P`
# collapses `..`, absolutises, follows symlinked directories, and on a
# case-insensitive filesystem reports the true on-disk case. So `tests`,
# `./tests`, `tests/../tests`, `$PWD/tests`, a symlink to tests, and `TESTS` all
# produce the same manifest key. Lexical cleanup did not: it left `..` alone, and
# every such spelling matched no manifest entry.
#
# Note this resolves the scope DIRECTORY only. Fixture paths underneath keep the
# spelling `find` reports, which is what the manifest declares — important
# because tests/integration/test.pd is itself a symlink and must stay a distinct,
# declared fixture rather than being folded into its target.
REPO_ROOT=$(pwd -P)
resolve_scope() {
  local raw=$1 phys
  phys=$(cd "$raw" 2>/dev/null && pwd -P) || return 1
  case "$phys" in
    "$REPO_ROOT")   printf '.' ;;
    "$REPO_ROOT"/*) printf '%s' "${phys#"$REPO_ROOT"/}" ;;
    *)              return 2 ;;
  esac
}

# Does scope $1 contain path $2 (or equal it)? `.` is the repository root and so
# contains every repo-relative path — the special case that made `conformance.sh .`
# report declared_in_scope=0 and let a deleted fixture escape MISSING entirely.
scope_contains() {
  [ "$1" = "$2" ] && return 0
  [ "$1" = "." ] && return 0
  case "$2" in "$1"/*) return 0 ;; esac
  return 1
}

if [ "$#" -gt 0 ]; then SCOPES=("$@"); else SCOPES=(tests examples); fi
i=0
while [ "$i" -lt "${#SCOPES[@]}" ]; do
  raw=${SCOPES[$i]}
  if [ -z "$raw" ]; then echo "error: empty scope argument" >&2; exit 2; fi
  # Enumeration and the manifest are both newline-delimited, so a newline in a
  # path would split into two meaningless entries rather than fail.
  # $'\n' and not "$(printf '\n')": command substitution strips trailing
  # newlines, so the latter is the empty string and matches every path.
  case "$raw" in *$'\n'*)
    echo "error: scope path contains a newline; paths must be newline-free" >&2
    exit 2 ;;
  esac
  if [ ! -d "$raw" ]; then
    echo "error: scope '$raw' is not a directory. Refusing to report a green run" >&2
    echo "       over a scope that does not exist." >&2
    exit 2
  fi
  if [ ! -r "$raw" ] || [ ! -x "$raw" ]; then
    echo "error: scope '$raw' is not readable" >&2
    exit 2
  fi
  s=$(resolve_scope "$raw"); rs=$?
  if [ "$rs" -eq 1 ]; then
    echo "error: scope '$raw' could not be resolved" >&2; exit 2
  elif [ "$rs" -eq 2 ]; then
    echo "error: scope '$raw' resolves outside the repository ($REPO_ROOT)." >&2
    echo "       Fixtures are declared by repo-relative path, so a directory" >&2
    echo "       outside the repo can never match the manifest." >&2
    exit 2
  fi
  SCOPES[i]=$s
  i=$((i+1))
done

# Overlapping scopes are rejected rather than silently deduplicated. `find tests
# ./tests` visits every fixture twice, which inflated fixtures/evaluated/verified
# and still exited 0 — a coverage number doubled by repeating an argument. Silent
# dedup would hide the mistake; naming it is the point.
i=0
while [ "$i" -lt "${#SCOPES[@]}" ]; do
  j=$((i+1))
  while [ "$j" -lt "${#SCOPES[@]}" ]; do
    a=${SCOPES[$i]}; b=${SCOPES[$j]}
    if [ "$a" = "$b" ]; then
      echo "error: scope '$a' given more than once (as '$a' and '$b' after resolution)." >&2
      echo "       Every fixture under it would be compiled and counted twice." >&2
      exit 2
    fi
    if scope_contains "$a" "$b" || scope_contains "$b" "$a"; then
      echo "error: scopes '$a' and '$b' overlap; one contains the other." >&2
      echo "       Every fixture in the nested scope would be counted twice." >&2
      echo "       Pass the outer directory alone." >&2
      exit 2
    fi
    j=$((j+1))
  done
  i=$((i+1))
done

# --------------------------------------------------------------------------
# Manifest
# --------------------------------------------------------------------------
M_PATH=(); M_CLASS=(); M_STAGE=(); M_FP=(); M_OWNER=(); M_NOTE=()
M_LINE=(); M_SEEN=()
manifest_errors=0

merr() { echo "error: $MANIFEST:$1: $2" >&2; manifest_errors=$((manifest_errors+1)); }

# The stage column, for the three classes that carry one. `link` — gcc refusing
# the C that codegen emitted — is not validated here, it is REFUSED. Declaring it
# would mean declaring a compiler defect as a fixture's expected behaviour, and
# the runner fails that outcome unconditionally anyway (BACKEND_REJECT, or
# HARNESS_ERROR while the accusation is unproven). Refusing
# the spelling is what keeps the escape hatch shut: the verdict cannot later be
# excused by writing a column value, because the column value does not parse.
# This check is green today — measured on this tree, of 82 non-comment manifest
# rows the stage column holds 58 `-` and 24 `compile`, and zero `link`. Its job
# is to keep that zero, not to discover anything.
check_stage() {   # check_stage <lineno> <class> <stage>
  case "$3" in
    compile|run) return 0 ;;
    link) merr "$1" "class=$2 declares stage 'link': that gcc is expected to reject the C this compiler emits. That is never a property of the fixture. If pdc accepted the source, C that will not compile is a defect in pdc — fix the backend, do not declare the defect. The runner fails on this outcome whatever this manifest says, so the row would not buy a green run either." ;;
    *) merr "$1" "class=$2 needs stage compile|run, got '$3'" ;;
  esac
}

lineno=0
while IFS= read -r raw || [ -n "$raw" ]; do
  lineno=$((lineno+1))
  raw=${raw%$'\r'}
  case "$raw" in ''|'#'*) continue ;; esac
  case "$raw" in *[![:space:]]*) ;; *) continue ;; esac

  IFS=$'\t' read -r mp mc ms mf mo mn <<<"$raw"
  mp=${mp:-}; mc=${mc:-}; ms=${ms:-}; mf=${mf:-}; mo=${mo:-}; mn=${mn:-}
  if [ -z "$mp" ] || [ -z "$mc" ] || [ -z "$ms" ] || [ -z "$mf" ] || [ -z "$mo" ] || [ -z "$mn" ]; then
    merr "$lineno" "expected 6 tab-separated non-empty columns (use '-' for N/A), got: $raw"
    continue
  fi

  mp=$(canon "$mp")

  # Duplicates must be rejected here, naming both lines. Left to surface later
  # they look like an unrelated MISSING entry and send you to the wrong file.
  dup=-1; j=0
  while [ "$j" -lt "${#M_PATH[@]}" ]; do
    if [ "${M_PATH[$j]}" = "$mp" ]; then dup=$j; break; fi
    j=$((j+1))
  done
  if [ "$dup" -ge 0 ]; then
    merr "$lineno" "duplicate entry for '$mp' (first declared at line ${M_LINE[$dup]})"
    continue
  fi

  case "$mc" in
    run)
      [ "$ms" = "-" ] || merr "$lineno" "class=run must have stage '-', got '$ms'"
      case "$mf" in
        expected)
          # Declared here, but the golden must actually be on disk: a declared
          # transcript that does not exist would otherwise verify nothing.
          if [ ! -f "${mp%.pd}.expected" ]; then
            merr "$lineno" "declares 'expected' but ${mp%.pd}.expected does not exist"
          fi ;;
        # `-` used to mean "check the exit code and nothing else", which is the
        # exact exit-0-wrong-answer hole transcripts exist to close, available as
        # a silent opt-out. It is now a manifest error: a fixture that genuinely
        # cannot carry a transcript must say so as class=untranscribed, with an
        # owner and a reason, and is then reported as a debt on every run.
        *) merr "$lineno" "class=run observable must be 'expected', got '$mf'. A fixture that cannot have a transcript must be declared class=untranscribed with an owner and a reason." ;;
      esac
      [ "$mo" = "-" ] || merr "$lineno" "class=run must have owner '-', got '$mo'"
      ;;
    untranscribed)
      # The reviewed allowance for "this one really cannot be transcribed".
      # Deliberately shaped like an xfail row: owned, reasoned, and printed on
      # every run, so it is a visible debt rather than a quiet exemption.
      [ "$ms" = "-" ] || merr "$lineno" "class=untranscribed must have stage '-', got '$ms'"
      [ "$mf" = "-" ] || merr "$lineno" "class=untranscribed must have observable '-', got '$mf'"
      case "$mo" in M[1-9]|unscheduled) ;; *) merr "$lineno" "class=untranscribed needs an owner M1..M9 or 'unscheduled', got '$mo'" ;; esac
      case "$mn" in why:*) ;; *) merr "$lineno" "class=untranscribed note must begin 'why:<reason this fixture cannot be transcribed>'; got '$mn'" ;; esac
      ;;
    vacuous)
      [ "$ms" = "-" ] || merr "$lineno" "class=vacuous must have stage '-', got '$ms'"
      [ "$mf" = "-" ] || merr "$lineno" "class=vacuous must have fingerprint '-', got '$mf'"
      case "$mo" in M[1-9]|unscheduled) ;; *) merr "$lineno" "class=vacuous needs an owner M1..M9 or 'unscheduled', got '$mo'" ;; esac
      # A placeholder must name what it appears to cover. Without this the count
      # says "3 vacuous" and a reader still cannot tell that async is at zero.
      case "$mn" in claims:*) ;; *) merr "$lineno" "class=vacuous note must begin 'claims:<feature>' naming the feature it appears to cover but does not; got '$mn'" ;; esac
      ;;
    xfail)
      check_stage "$lineno" xfail "$ms"
      [ "$mf" != "-" ] || merr "$lineno" "class=xfail needs a diagnostic fingerprint"
      if [ "$ms" = "run" ]; then
        case "$mf" in exit=[0-9]*) ;; *) merr "$lineno" "stage=run needs fingerprint 'exit=<N>', got '$mf'" ;; esac
      fi
      case "$mo" in M[1-9]|unscheduled) ;; *) merr "$lineno" "class=xfail needs an owner M1..M9 or 'unscheduled', got '$mo'" ;; esac
      [ "$mn" != "-" ] || merr "$lineno" "class=xfail needs a note"
      ;;
    reject)
      check_stage "$lineno" reject "$ms"
      [ "$mf" != "-" ] || merr "$lineno" "class=reject needs a diagnostic fingerprint"
      if [ "$ms" = "run" ]; then
        case "$mf" in exit=[0-9]*) ;; *) merr "$lineno" "stage=run needs fingerprint 'exit=<N>', got '$mf'" ;; esac
      fi
      # A negative test is correct behaviour, so it is owed to no milestone.
      [ "$mo" = "-" ] || merr "$lineno" "class=reject must have owner '-' (it is coverage, not a debt), got '$mo'"
      [ "$mn" != "-" ] || merr "$lineno" "class=reject needs a note"
      ;;
    skip)
      # A non-program must prove it is one, the same way an xfail does: by the
      # compiler refusing it, at a declared stage, with a declared diagnostic.
      # This replaced an `fn main` REGEX, which three compiler-valid spellings
      # evaded — `fn /* c */ main()`, `fn // c<LF> main()` and plain `fn<LF>
      # main()` all compile and run, and all three would have passed as `skip`:
      # never compiled, never gated.
      check_stage "$lineno" skip "$ms"
      [ "$mf" != "-" ] || merr "$lineno" "class=skip needs the diagnostic proving it is not a program (e.g. 'No main function found')"
      [ "$mo" = "-" ] || merr "$lineno" "class=skip must have owner '-', got '$mo'"
      [ "$mn" != "-" ] || merr "$lineno" "class=skip needs a note"
      ;;
    *) merr "$lineno" "unknown class '$mc' (run|untranscribed|vacuous|xfail|reject|skip)" ; continue ;;
  esac

  M_PATH+=("$mp"); M_CLASS+=("$mc"); M_STAGE+=("$ms"); M_FP+=("$mf")
  M_OWNER+=("$mo"); M_NOTE+=("$mn"); M_LINE+=("$lineno"); M_SEEN+=(0)
done < "$MANIFEST"

if [ "$manifest_errors" -gt 0 ]; then
  echo "error: $manifest_errors malformed manifest entr(y/ies); refusing to run" >&2
  exit 2
fi

m_index() {
  local target=$1 k=0
  while [ "$k" -lt "${#M_PATH[@]}" ]; do
    if [ "${M_PATH[$k]}" = "$target" ]; then echo "$k"; return 0; fi
    k=$((k+1))
  done
  echo -1
}

in_scope() {
  local p=$1 d
  for d in "${SCOPES[@]}"; do
    scope_contains "$d" "$p" && return 0
  done
  return 1
}

# --------------------------------------------------------------------------
# Enumeration — status captured, never discarded
# --------------------------------------------------------------------------
FIND_RAW=$TMPROOT/found.raw
FIND_OUT=$TMPROOT/found
FIND_ERR=$TMPROOT/found.err
# No -type f: tests/integration/test.pd is a symlink, and silently dropping a
# fixture is precisely the failure this manifest exists to prevent.
# -print0, not newline. A filename may legally contain a newline, and a
# newline-delimited pipeline turns one such file into two paths: measured, a
# fixture named `a.pd<LF>b.pd` produced `tests/a.pd` twice and `b.pd` once from
# three real files, so the real fixture was never gated and another was
# double-counted — verified=4 over 3 files, exit 0.
find "${SCOPES[@]}" -name '*.pd' -print0 >"$FIND_RAW" 2>"$FIND_ERR"
find_rc=$?
if [ "$find_rc" -ne 0 ] || [ -s "$FIND_ERR" ]; then
  echo "error: enumeration failed (find exit $find_rc):" >&2
  sed 's/^/       /' "$FIND_ERR" >&2
  exit 2
fi

# Read NUL-delimited, then reject any newline-bearing path outright. The manifest
# is a line-based file, so such a fixture could never be declared in it — the only
# honest answer is to refuse, not to silently drop it.
: > "$FIND_OUT"
fixture_count=0
while IFS= read -r -d '' rawline; do
  case "$rawline" in *$'\n'*)
    echo "error: fixture path contains a newline and cannot be declared in a" >&2
    echo "       line-based manifest: $(printf '%q' "$rawline")" >&2
    exit 2 ;;
  esac
  canon "$rawline" >> "$FIND_OUT"
  printf '\n' >> "$FIND_OUT"
  fixture_count=$((fixture_count+1))
done < "$FIND_RAW"
if [ "$fixture_count" -eq 0 ]; then
  echo "error: no .pd fixtures under: ${SCOPES[*]}" >&2
  exit 2
fi
sort "$FIND_OUT" -o "$FIND_OUT"

# Per scope, not just overall: a valid-but-wrong directory contributing nothing
# would otherwise ride along invisibly behind a scope that did find something.
# Literal prefix comparison, never a regex — see has_prefix.
for d in "${SCOPES[@]}"; do
  found_here=0
  if [ "$d" = "." ]; then
    found_here=1
  else
    while IFS= read -r p; do
      if has_prefix "$d/" "$p"; then found_here=1; break; fi
    done < "$FIND_OUT"
  fi
  [ "$found_here" -eq 1 ] && continue
  echo "error: no .pd fixtures under scope '$d'" >&2
  echo "       an empty corpus is a broken invocation, not a pass." >&2
  exit 2
done

verified=0; untranscribed=0; vacuous=0; xfail=0; reject=0; skip=0
hard_fail=0; blessed=0
declare -a FAILED
declare -a XFAIL_NOTES
declare -a VACUOUS_NOTES
declare -a REJECT_NOTES
declare -a UNTRANSCRIBED_NOTES

fail() { FAILED+=("$1"); hard_fail=$((hard_fail+1)); }

printf '%-52s %s\n' "FILE" "VERDICT"
printf '%s\n' "-------------------------------------------------- ----------------"

n=0
while IFS= read -r f; do
  # Already canonicalised during enumeration. If a path does not resolve to a
  # real file here, the newline-delimited transport split it — refuse rather than
  # report a phantom fixture.
  # -e is false for a DANGLING symlink, which is a real enumerated path (the repo
  # has one: bootstrap/v3_incremental/test.pd). Only a path that is neither a file
  # nor a link indicates the newline-delimited transport split it.
  if [ ! -e "$f" ] && [ ! -L "$f" ]; then
    echo "error: enumerated path '$f' does not exist; a filename containing a" >&2
    echo "       newline would split like this. Fixture paths must be newline-free." >&2
    exit 2
  fi
  n=$((n+1))

  idx=$(m_index "$f")
  if [ "$idx" -lt 0 ]; then
    printf '%-52s %s\n' "$f" "UNDECLARED"
    fail "$f [UNDECLARED] not in $MANIFEST — every fixture must declare its expected class"
    continue
  fi
  M_SEEN[idx]=1
  class=${M_CLASS[$idx]}; stage_exp=${M_STAGE[$idx]}; fp=${M_FP[$idx]}
  owner=${M_OWNER[$idx]}; note=${M_NOTE[$idx]}
  # xfail and reject share the fingerprint machinery and differ only in meaning,
  # so they share the mismatch path but must not share its label.
  case "$class" in
    reject) MM=REJECT ;;
    skip)   MM=SKIP ;;
    *)      MM=XFAIL ;;
  esac

  # Readability is still checked here (an unreadable fixture is a harness
  # failure, not a silent pass), but nothing infers "is this a program" from the
  # text any more — the compiler decides that below.
  grep_status E '.' "$f"
  if [ $? -gt 1 ]; then
    printf '%-52s %s\n' "$f" "UNREADABLE"
    fail "$f [UNREADABLE] could not be read (permissions, or a dangling symlink). A fixture the gate cannot read is a harness failure, not a non-program."
    continue
  fi

  # A symlink fixture is followed by grep and by pdc alike, so a link pointing
  # outside the repository would put the gate over mutable, unversioned content
  # and still report green. The corpus legitimately contains one internal
  # symlink; external targets are refused.
  if [ -L "$f" ]; then
    tgt=$(resolve_file "$f") || tgt=""
    if [ -z "$tgt" ]; then
      printf '%-52s %s\n' "$f" "UNREADABLE"
      fail "$f [UNREADABLE] symlink target could not be resolved"
      continue
    fi
    if ! has_prefix "$REPO_ROOT/" "$tgt"; then
      printf '%-52s %s\n' "$f" "ESCAPES_REPO"
      fail "$f [ESCAPES_REPO] symlink resolves to $tgt, outside $REPO_ROOT. The gate would be measuring unversioned content."
      continue
    fi
  fi

  # The vacuous marker is documentation for the reader; the manifest is what the
  # gate believes. Requiring them to agree keeps either from drifting alone.
  # The pipeline below discards grep's status, so probe readability first with a
  # status we actually inspect; unreadable was already caught above, but a file
  # that becomes unreadable between the two calls must not read as "no marker".
  grep_status E '^[[:space:]]*//@[[:space:]]*vacuous:' "$f"
  marker_rc=$?
  if [ "$marker_rc" -gt 1 ]; then
    printf '%-52s %s\n' "$f" "UNREADABLE"
    fail "$f [UNREADABLE] could not be scanned for a vacuous marker"
    continue
  fi
  marker_at=""
  if [ "$marker_rc" -eq 0 ]; then
    marker_at=$(grep -nE -- '^[[:space:]]*//@[[:space:]]*vacuous:' "$f" | head -1 | cut -d: -f1)
  fi
  if [ -n "$marker_at" ] && [ "$marker_at" != "1" ]; then
    printf '%-52s %s\n' "$f" "MARKER_MISPLACED"
    fail "$f [MARKER_MISPLACED] '//@ vacuous:' on line $marker_at; only line 1 is honoured"
    continue
  fi
  if [ "$class" = "vacuous" ] && [ -z "$marker_at" ]; then
    printf '%-52s %s\n' "$f" "MARKER_MISSING"
    fail "$f [MARKER_MISSING] declared vacuous in $MANIFEST but has no line-1 '//@ vacuous:' marker"
    continue
  fi
  if [ "$class" != "vacuous" ] && [ -n "$marker_at" ]; then
    printf '%-52s %s\n' "$f" "MARKER_UNDECLARED"
    fail "$f [MARKER_UNDECLARED] carries a '//@ vacuous:' marker but is declared class=$class"
    continue
  fi

  # ---- compile / link / run ------------------------------------------------
  # Unique output per fixture, removed and verified absent first: a stale binary
  # (or one from a same-basename fixture) could otherwise satisfy the -x test
  # after a compile that produced nothing, turning NO_BINARY into PASS.
  out="cf_${n}_$(printf '%s' "$f" | tr -c 'A-Za-z0-9' '_')"
  rm -f "$OUT_DIR/$out"
  if [ -e "$OUT_DIR/$out" ]; then
    printf '%-52s %s\n' "$f" "HARNESS_ERROR"
    fail "$f [HARNESS_ERROR] could not remove stale output $OUT_DIR/$out"
    continue
  fi

  # The emitted translation unit. Its EXISTENCE is how the verdict below decides
  # WHO refused this program: codegen is the last phase (src/driver/mod.rs:210-245),
  # so this file exists if and only if the front end accepted. That question is
  # asked of the filesystem and not of the log on purpose — a fixture's own text
  # can reach the log. Measured on this tree: `fn main() { print_int(Linking); }`
  # is a pure front-end refusal whose diagnostic reads "Undefined variable or
  # function: 'Linking'", and the previous stage classifier grepped the log for
  # the literal `Linking`, so that front-end refusal was already being reported
  # as a link-stage failure. Under a check that fails unconditionally, the same
  # confusion would accuse the backend of a defect the front end had just caught.
  # Unlike the binary, this name comes from the fixture's BASENAME
  # (src/codegen/mod.rs:3650-3655), so two same-named fixtures share it — remove
  # it first, or the previous fixture's C answers for this one.
  emitted_c="$OUT_DIR/$(basename "${f%.pd}").c"
  rm -f "$emitted_c"
  if [ -e "$emitted_c" ]; then
    printf '%-52s %s\n' "$f" "HARNESS_ERROR"
    fail "$f [HARNESS_ERROR] could not remove stale generated C $emitted_c"
    continue
  fi

  log=$TMPROOT/log
  # R6: STDERR IS CAPTURED SEPARATELY, and the merged log is then rebuilt for
  # every consumer that already existed. The parser must read stderr alone,
  # because a fixture's own stdout can contain a line shaped like a header and a
  # merged stream cannot tell the two producers apart — the same confusion, one
  # stream up, that made a fixture containing `Linking` read as a link failure.
  #
  # THE REBUILD IS CONCATENATION, AND ITS COST IS MEASURED, NOT ASSUMED: over all
  # 221 fixtures, stdout-then-stderr is line-identical to the interleaved capture
  # for 220, and reorders lines for exactly one (`tests/03_const_items.pd`, whose
  # gcc `note:` lines move relative to one stdout line). No consumer of "$log"
  # here is order-sensitive — the fingerprint match and the gcc-contradiction
  # check are both whole-log `grep -qF`.
  "$PDC" compile "$f" -o "$out" >"$TMPROOT/pdc_stdout" 2>"$TMPROOT/pdc_stderr"
  pdc_rc=$?
  cat "$TMPROOT/pdc_stdout" "$TMPROOT/pdc_stderr" >"$log"

  if [ "$class" = "reject" ] || [ "$class" = "skip" ]; then
    if diag_state=$(pd_diag_parse "$TMPROOT/pdc_stderr"); then
      case "$(pd_diag_state "$diag_state")" in
        CODED)     diag_coded=$((diag_coded+1)) ;;
        MALFORMED) diag_malformed=$((diag_malformed+1)) ;;
        *)         diag_uncoded=$((diag_uncoded+1)) ;;
      esac
    else
      diag_unreadable=$((diag_unreadable+1))
    fi
  fi
  diag=$(strip_ansi <"$log" | grep -m1 -E 'error' | head -c 200)

  # TWO INDEPENDENT WITNESSES THAT THE FRONT END ACCEPTED, and either is enough.
  # Requiring BOTH would turn a sufficient condition into a conjunction, and a
  # conjunction fails OPEN on the half that is missing: pdc exits 3 (gcc refused
  # the translation unit) while codegen names or locates its output differently
  # than this gate derives it, or the file is cleaned up mid-run — and the
  # contradiction check below does not fire, because the sibling branch replaced
  # the legacy `gcc compilation failed` prose it looks for. The fixture would
  # fall through to stage `compile`, where `reject|compile` is a row a manifest
  # is allowed to write, and the outcome this branch calls unblessable would be
  # blessed. So:
  #
  #   * a STRUCTURED exit code (3/4/5) is conclusive on its own. It is a
  #     statement by the producer about what happened and needs no corroboration.
  #   * the TRANSLATION UNIT on disk still decides the UNSTRUCTURED case, which
  #     is every failure today's pdc can produce, and still detects the
  #     contradiction in the front-end arm below.
  backend_code=0
  case "$pdc_rc" in 3|4|5|6) backend_code=1 ;; esac

  stage_act=""; detail=""
  if [ "$pdc_rc" -ne 0 ] && { [ "$backend_code" -eq 1 ] || [ -f "$emitted_c" ]; }; then
    # The front end ACCEPTED this program (codegen ran and wrote the C, or pdc
    # said so with its exit code), and the build still failed. Never expectable,
    # exactly like NO_BINARY: if pdc said yes to the source, C it cannot build is
    # a defect in pdc. There is no manifest column that excuses this — the
    # verdict is reached before the declared class is consulted, and stage `link`
    # is refused at parse time.
    #
    # What this does NOT do: find such defects. The corpus only contains programs
    # someone wrote down, so a program nobody added stays unprotected — measured,
    # neither of the two reproductions that motivated this check is in the corpus,
    # so this gate would not have caught either. It makes the OUTCOME inadmissible
    # for every fixture the corpus runs. Coverage is a separate, open debt.
    #
    # WHICH of them happened is a SEPARATE question, and this gate is not
    # allowed to guess it. Before the producer was fixed, `gcc compilation
    # failed` was emitted for EVERY unsuccessful gcc status, so a translation
    # unit gcc refused and a gcc killed by SIGKILL produced the same string.
    # Naming the first accuses codegen on evidence that cannot tell it from the
    # machine running out of memory, and a heuristic over gcc's stderr would be
    # exactly the forgeable classifier this change exists to remove, one level
    # down.
    #
    # SO THE ANSWER COMES FROM THE EXIT CODE, WHICH FIXTURE TEXT HAS NO ROUTE TO.
    # Read src/linker.rs's EXIT_* constants and its LinkError variants directly,
    # not a summary of them — including this one, which is a summary. The codes
    # this gate acts on:
    #
    #   3 EXIT_BACKEND_REJECT     GccFailed  — gcc ran to completion and exited
    #                             nonzero: it REFUSED the translation unit.
    #   4 EXIT_BACKEND_ILL_TYPED  IllTypedC  — gcc exited 0 and diagnosed C that
    #                             pdc generated. An ICE: no Palladium program
    #                             asks for ill-typed C. Also a compiler defect,
    #                             and reported as one, with its own sentence.
    #   5 EXIT_TOOLCHAIN          Toolchain | GccDied — gcc could not be spawned,
    #                             or was killed by a signal. It never reached a
    #                             verdict, so nothing is established about the C.
    #
    #   6 EXIT_GCC_UNEXPLAINED   GccUnexplained — gcc RAN and exited nonzero,
    #                             and pdc could not show the verdict was about
    #                             our C. Not exotic: an undefined symbol from
    #                             the link stage lands here, and so does a full
    #                             disk. Refused, and not attributed to anyone.
    #
    # THE STUBS STILL OWE AN INTEGRATION RECEIPT; IT DID NOT LAND WITH THEM.
    # The controls in scripts/test-conformance-runner.sh manufacture these codes,
    # which is what makes them permanent — but a stub reproduces the numbers
    # WRITTEN DOWN HERE, so it agrees with this comment by construction and
    # cannot detect the producer renumbering them. Only the real binary can:
    # compile a program whose emitted C gcc refuses and assert pdc exits 3.
    #
    # `$diag` is the FIRST line matching `error`, which on this path is always
    # pdc's own wrapper `error: gcc compilation failed:` — a line that tells the
    # reader nothing the verdict has not already said. For a message whose whole
    # content is "go fix the backend", the useful line is the first one AFTER the
    # wrapper: the actual `x.c:NNN:CC: error: ...`. Falls back to $diag when there
    # is no wrapper, so no path loses its diagnostic.
    cdiag=$(strip_ansi <"$log" 2>/dev/null | sed -n '/gcc compilation failed/,$p' \
              | tail -n +2 | grep -m1 -E 'error' | head -c 200)
    [ -n "$cdiag" ] || cdiag=$diag

    # Say where the C is, or say that it is missing. When the exit code is the
    # witness the file need not exist, and naming a path that is not there would
    # be the same over-claim this block exists to avoid, one sentence down.
    if [ -f "$emitted_c" ]; then
      where="($emitted_c)"
    else
      where="(no translation unit at $emitted_c — pdc's exit code is the witness that the front end accepted, and it is sufficient on its own)"
    fi

    case "$pdc_rc" in
      3)
        printf '%-52s %s\n' "$f" "BACKEND_REJECT"
        fail "$f [BACKEND_REJECT] pdc accepted this source and then gcc refused the C it emitted $where. That is a defect in this compiler, not a property of the fixture, and no manifest column may declare it: there is no valid Palladium program whose emitted C is allowed not to compile. Fix the backend. Diagnostic: $cdiag"
        ;;
      4)
        printf '%-52s %s\n' "$f" "BACKEND_REJECT"
        fail "$f [BACKEND_REJECT] pdc accepted this source and then emitted C that the C compiler diagnosed as ill-typed $where. gcc did not refuse it — this compiler generated it, which makes it an internal defect rather than anything the fixture asked for. Fix the backend. Diagnostic: $cdiag"
        ;;
      5)
        printf '%-52s %s\n' "$f" "HARNESS_ERROR"
        fail "$f [HARNESS_ERROR] the front end accepted this program $where, but the C toolchain never reached a verdict (gcc could not be spawned, or was killed by a signal). Nothing is established about the emitted C, so nothing is claimed about it: $cdiag"
        ;;
      6)
        # gcc DID reach a verdict — unlike 5 — and pdc could not attribute it to
        # the C it emitted. An undefined symbol from the link stage looks like
        # this and IS a codegen defect; a full disk looks like this and is not.
        # Refused either way, and not attributed, because the producer could not
        # attribute it either.
        printf '%-52s %s\n' "$f" "HARNESS_ERROR"
        fail "$f [HARNESS_ERROR] the front end accepted this program $where, and gcc then exited nonzero WITHOUT diagnosing that translation unit. gcc reached a verdict, but nothing attributes it to the emitted C: an undefined symbol from the link stage (a real backend defect) and a full disk (not one) are indistinguishable here, so no accusation is made. Not a fixture property either way, and no manifest column excuses it: $cdiag"
        ;;
      *)
        # Only reachable with the translation unit on disk: an unstructured code
        # is not a witness, so the file is the only thing that got us here.
        printf '%-52s %s\n' "$f" "HARNESS_ERROR"
        fail "$f [HARNESS_ERROR] the front end accepted this program and emitted $emitted_c, and the build then failed — but pdc exited $pdc_rc, which is not one of the structured codes (3 backend reject, 4 backend emitted ill-typed C, 5 toolchain never reached a verdict, 6 gcc gave a verdict pdc could not attribute) and so does not say what happened. This gate will not call it a compiler defect on evidence that cannot distinguish them. Either way it is not a fixture property and no manifest column excuses it: $cdiag"
        ;;
    esac
    continue
  elif [ "$pdc_rc" -ne 0 ]; then
    # No translation unit: the FRONT END refused it. This is the only failure a
    # fixture may declare, and it is the `compile` stage.
    #
    # CONTRADICTION CHECK, and it guards the one way a gcc rejection could still
    # be laundered into a declarable stage. Reaching here means "no .c on disk",
    # and the two derivations of that name — this file's `basename` and codegen's
    # `file_stem` (src/codegen/mod.rs:3650-3655) — are independent. If they ever
    # diverge, a real backend failure lands in THIS arm, where `xfail compile` is
    # a stage the validator permits, and the exemption is back through a door the
    # rest of this change locked. So: the log claiming gcc ran while no
    # translation unit exists is a contradiction, not a front-end refusal, and it
    # is refused rather than classified. Unlike a control built on a live defect,
    # this one does not evaporate when the thing it guards against is fixed.
    grep_status F 'gcc compilation failed' "$log"; wrapper_rc=$?
    if [ "$wrapper_rc" -gt 1 ]; then
      printf '%-52s %s\n' "$f" "HARNESS_ERROR"
      fail "$f [HARNESS_ERROR] could not read the compiler log to check it against the emitted translation unit"
      continue
    fi
    if [ "$wrapper_rc" -eq 0 ]; then
      printf '%-52s %s\n' "$f" "HARNESS_ERROR"
      fail "$f [HARNESS_ERROR] the compiler log says gcc ran and the build failed, but no translation unit is at $emitted_c. Those cannot both be true: either codegen names its output differently than this gate derives it, or the file was removed mid-run. Refusing rather than filing this under the front-end 'compile' stage, which a manifest row is allowed to declare: $diag"
      continue
    fi
    stage_act="compile"
    detail=$diag
  elif [ ! -x "$OUT_DIR/$out" ]; then
    # pdc claimed success and produced nothing. Never expectable.
    printf '%-52s %s\n' "$f" "NO_BINARY"
    fail "$f [NO_BINARY] compiler reported success but produced no executable"
    continue
  else
    "$OUT_DIR/$out" >"$TMPROOT/stdout" 2>/dev/null
    run_rc=$?
    if [ "$run_rc" -ne 0 ]; then stage_act="run"; detail="exit=$run_rc"; fi
  fi

  # Transcript diff. Exit 0 says the program did not crash; only this says it
  # computed the right answer.
  out_mismatch=0
  golden="${f%.pd}.expected"
  if [ -z "$stage_act" ] && [ "$class" = "run" ] && [ "$fp" = "expected" ]; then
    if [ "${CONFORMANCE_BLESS:-0}" = "1" ]; then
      cp "$TMPROOT/stdout" "$golden"
      echo "  blessed $golden" >&2
      blessed=$((blessed+1))
    else
      # diff has three outcomes too: 0 identical, 1 differing, >1 could not
      # compare. A missing or unreadable golden must not read as "differs".
      diff -u "$golden" "$TMPROOT/stdout" >"$TMPROOT/diff" 2>&1
      case $? in
        0) ;;
        1) out_mismatch=1
           detail=$(head -20 "$TMPROOT/diff" | tail -12 | tr '\n' '~') ;;
        *) out_mismatch=2
           detail=$(head -3 "$TMPROOT/diff" | tr '\n' '~') ;;
      esac
    fi
  fi

  # Fingerprint comparison, computed before the verdict chain so its THIRD
  # outcome (could not read the log) is distinguishable from "did not match".
  fp_match=1
  if [ -n "$stage_act" ] && [ "$stage_act" != "run" ]; then
    if strip_ansi <"$log" >"$TMPROOT/diag" 2>/dev/null; then
      grep_status F "$fp" "$TMPROOT/diag"; fp_match=$?
    else
      fp_match=2
    fi
  fi

  # ---- verdict -------------------------------------------------------------
  if [ -z "$stage_act" ]; then           # it passed
    case "$class" in
      run)     if [ "$out_mismatch" -eq 2 ]; then
                 printf '%-52s %s\n' "$f" "HARNESS_ERROR"
                 fail "$f [HARNESS_ERROR] could not compare stdout against $golden: $detail"
               elif [ "$out_mismatch" -eq 1 ]; then
                 printf '%-52s %s\n' "$f" "OUTPUT_MISMATCH"
                 fail "$f [OUTPUT_MISMATCH] ran and exited 0, but stdout differs from $golden: $detail"
               else
                 printf '%-52s %s\n' "$f" "PASS_VERIFIED"; verified=$((verified+1))
               fi ;;
      untranscribed)
               printf '%-52s %s\n' "$f" "PASS_UNTRANSCRIBED"; untranscribed=$((untranscribed+1))
               UNTRANSCRIBED_NOTES+=("$f [$owner] $note") ;;
      vacuous) printf '%-52s %s\n' "$f" "PASS_VACUOUS"; vacuous=$((vacuous+1))
               VACUOUS_NOTES+=("$f [$owner] $note") ;;
      xfail)   printf '%-52s %s\n' "$f" "XPASS"
               # NOT "delete the row": the fixture is still on disk, so under the
               # closed inventory a deleted row is UNDECLARED and the gate stays
               # red. Paying off an xfail is a TRANSITION, and this text is the
               # handoff protocol for whoever fixes it.
               fail "$f [XPASS] declared to fail at $stage_exp but now passes. Do NOT delete the row (the fixture still exists, so that would make it UNDECLARED). In $MANIFEST change its row to:  $f<TAB>run<TAB>-<TAB>expected<TAB>-<TAB>-  and add the transcript ${f%.pd}.expected. Bootstrap it in this order, because declaring 'expected' while the file is absent is a manifest error and bless would never get to run:  (1) create it empty: : > ${f%.pd}.expected  (2) CONFORMANCE_BLESS=1 bash scripts/conformance.sh  (3) READ the generated transcript and confirm the values are right before committing. Was: $note" ;;
      reject)  printf '%-52s %s\n' "$f" "REJECT_ACCEPTED"
               fail "$f [REJECT_ACCEPTED] the compiler must refuse this program at $stage_exp ('$fp') but accepted it: $note" ;;
      skip)    printf '%-52s %s\n' "$f" "SKIP_IS_A_PROGRAM"
               fail "$f [SKIP_IS_A_PROGRAM] declared a non-program, but the compiler accepted and built it. It is a program and must be gated — change its class from skip to run and add a transcript." ;;
    esac
  elif [ "$class" != "xfail" ] && [ "$class" != "reject" ] && [ "$class" != "skip" ]; then
    # No LINK_FAIL arm: `link` is no longer a stage this runner can reach. A
    # build that got past the front end and then failed was already answered
    # above, as BACKEND_REJECT or HARNESS_ERROR, and neither returns here.
    case "$stage_act" in
      compile) v=COMPILE_FAIL ;;
      *)       v=RUN_FAIL ;;
    esac
    printf '%-52s %s\n' "$f" "$v"
    fail "$f [$v] $detail"
  elif [ "$stage_act" != "$stage_exp" ]; then
    printf '%-52s %s\n' "$f" "${MM}_MISMATCH"
    fail "$f [${MM}_MISMATCH] declared to fail at '$stage_exp' but failed at '$stage_act': $detail"
  elif [ "$stage_act" = "run" ] && [ "$detail" != "$fp" ]; then
    printf '%-52s %s\n' "$f" "${MM}_MISMATCH"
    fail "$f [${MM}_MISMATCH] declared '$fp' but got '$detail'"
  elif [ "$stage_act" != "run" ] && [ "$fp_match" -gt 1 ]; then
    printf '%-52s %s\n' "$f" "HARNESS_ERROR"
    fail "$f [HARNESS_ERROR] could not read the compiler log to check the declared fingerprint"
  elif [ "$stage_act" != "run" ] && [ "$fp_match" -ne 0 ]; then
    printf '%-52s %s\n' "$f" "${MM}_MISMATCH"
    fail "$f [${MM}_MISMATCH] failed at the declared stage but not with the declared diagnostic; expected fingerprint '$fp', actual: $detail"
  elif [ "$class" = "reject" ]; then
    printf '%-52s %s\n' "$f" "REJECTED"
    reject=$((reject+1))
    REJECT_NOTES+=("$f [refused at $stage_exp: $fp] $note")
  elif [ "$class" = "skip" ]; then
    printf '%-52s %s\n' "$f" "SKIP"
    skip=$((skip+1))
  else
    printf '%-52s %s\n' "$f" "XFAIL"
    xfail=$((xfail+1))
    XFAIL_NOTES+=("$f [$owner, fails at $stage_exp: $fp] $note")
  fi
done < "$FIND_OUT"

# --------------------------------------------------------------------------
# Reconcile the closed inventory: declared-in-scope but never discovered
# --------------------------------------------------------------------------
declared_in_scope=0; out_of_scope=0
k=0
while [ "$k" -lt "${#M_PATH[@]}" ]; do
  if in_scope "${M_PATH[$k]}"; then
    declared_in_scope=$((declared_in_scope+1))
    if [ "${M_SEEN[$k]}" -eq 0 ]; then
      fail "${M_PATH[$k]} [MISSING] declared in $MANIFEST:${M_LINE[$k]} but not found on disk — deleted, renamed, or moved out of the gate"
    fi
  else
    out_of_scope=$((out_of_scope+1))
  fi
  k=$((k+1))
done

# Milestone gate: a milestone is finished when nothing is still owed to it.
if [ -n "$FORBID_OWNER" ]; then
  k=0
  while [ "$k" -lt "${#M_PATH[@]}" ]; do
    if [ "${M_SEEN[$k]}" -eq 1 ] && [ "${M_OWNER[$k]}" = "$FORBID_OWNER" ]; then
      fail "${M_PATH[$k]} [OWED_TO_$FORBID_OWNER] class=${M_CLASS[$k]} is still owed to $FORBID_OWNER: ${M_NOTE[$k]}"
    fi
    k=$((k+1))
  done
fi

evaluated=$((verified+untranscribed+vacuous+xfail+reject+skip))
echo
echo "=============================================="
echo "fixtures=$n declared_in_scope=$declared_in_scope evaluated=$evaluated"
echo "verified=$verified untranscribed=$untranscribed vacuous=$vacuous xfail=$xfail reject=$reject skip=$skip failures=$hard_fail"
# OBSERVATIONAL, NOT A VERDICT (GI-12 su1). Counted from the shared parser over
# the SEPARATE stderr capture, for every refusal-witness row. It says how far the
# code rollout has got, and it is the live-corpus exercise of the parser the
# cutover will make authoritative. `make check-diagnostic-codes` owns the
# judgements; nothing here fails on these numbers.
echo "diagnostic-codes(observational): coded=$diag_coded uncoded=$diag_uncoded malformed=$diag_malformed unreadable=$diag_unreadable"
echo "  verified   = ran AND its stdout matched the recorded transcript byte for"
echo "               byte. Only this column can see a wrong answer."
echo "  untranscribed = ran and exited 0, but has NO transcript, so a wrong answer"
echo "               would be invisible: a tail-return miscompile (D3) prints"
echo "               garbage and still exits 0. Each one is a declared, owned"
echo "               debt — there is no silent way into this column."
echo "  vacuous    = declared placeholder: runs, but only prints that its feature"
echo "               is unimplemented. NOT evidence the feature works."
echo "  xfail      = declared failing at a specific stage with a specific"
echo "               diagnostic, and still failing in exactly that way"
echo "  reject     = negative test: the compiler correctly refused it with the"
echo "               declared diagnostic. This IS coverage."
echo "  skip       = declared non-program, PROVEN so by the compiler refusing it"
echo "               with the declared diagnostic — not by pattern-matching the text"
if [ "$out_of_scope" -gt 0 ]; then
  echo "  note: $out_of_scope declared fixture(s) lie outside ${SCOPES[*]} and were not checked"
fi
echo "=============================================="

if [ ${#XFAIL_NOTES[@]} -gt 0 ]; then
  echo
  echo "Expected failures (XFAIL) — what is owed:"
  printf '  %s\n' "${XFAIL_NOTES[@]}"
fi
if [ ${#REJECT_NOTES[@]} -gt 0 ]; then
  echo
  echo "Negative tests — the compiler correctly refused these:"
  printf '  %s\n' "${REJECT_NOTES[@]}"
fi
if [ ${#UNTRANSCRIBED_NOTES[@]} -gt 0 ]; then
  echo
  echo "No transcript — ran, but a wrong answer would not be caught:"
  printf '  %s\n' "${UNTRANSCRIBED_NOTES[@]}"
fi
if [ ${#VACUOUS_NOTES[@]} -gt 0 ]; then
  echo
  echo "Vacuous — these RAN but prove nothing. Coverage of the named feature is ZERO:"
  printf '  %s\n' "${VACUOUS_NOTES[@]}"
fi
if [ ${#FAILED[@]} -gt 0 ]; then
  echo
  echo "Failures:"
  printf '  %s\n' "${FAILED[@]}"
fi

if [ "${CONFORMANCE_BLESS:-0}" = "1" ]; then
  echo
  echo "BLESS MODE: rewrote $blessed transcript(s) from THIS build's output."
  echo "This is not a gate run and does not exit 0. A blessed transcript is only"
  echo "as good as the compiler that produced it — read the diff before committing."
  exit 2
fi

[ "$hard_fail" -eq 0 ]
