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
#            reported as a debt on every run. It exists so that "no transcript"
#            is always a decision somebody signed, never a default.
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
# Manifest format: 6 TAB-separated columns, every column non-empty, `-` = N/A.
#   1 path         repo-root-relative. Tabs are the delimiter, so spaces are safe.
#   2 class        run | untranscribed | vacuous | xfail | reject | skip
#   3 stage        compile | link | run          (xfail/reject only, else `-`)
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

if [ "$#" -gt 0 ]; then SCOPES=("$@"); else SCOPES=(tests examples); fi
i=0
while [ "$i" -lt "${#SCOPES[@]}" ]; do
  raw=${SCOPES[$i]}
  if [ -z "$raw" ]; then echo "error: empty scope argument" >&2; exit 2; fi
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

# --------------------------------------------------------------------------
# Manifest
# --------------------------------------------------------------------------
M_PATH=(); M_CLASS=(); M_STAGE=(); M_FP=(); M_OWNER=(); M_NOTE=()
M_LINE=(); M_SEEN=()
manifest_errors=0

merr() { echo "error: $MANIFEST:$1: $2" >&2; manifest_errors=$((manifest_errors+1)); }

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
      case "$ms" in compile|link|run) ;; *) merr "$lineno" "class=xfail needs stage compile|link|run, got '$ms'" ;; esac
      [ "$mf" != "-" ] || merr "$lineno" "class=xfail needs a diagnostic fingerprint"
      if [ "$ms" = "run" ]; then
        case "$mf" in exit=[0-9]*) ;; *) merr "$lineno" "stage=run needs fingerprint 'exit=<N>', got '$mf'" ;; esac
      fi
      case "$mo" in M[1-9]|unscheduled) ;; *) merr "$lineno" "class=xfail needs an owner M1..M9 or 'unscheduled', got '$mo'" ;; esac
      [ "$mn" != "-" ] || merr "$lineno" "class=xfail needs a note"
      ;;
    reject)
      case "$ms" in compile|link|run) ;; *) merr "$lineno" "class=reject needs stage compile|link|run, got '$ms'" ;; esac
      [ "$mf" != "-" ] || merr "$lineno" "class=reject needs a diagnostic fingerprint"
      if [ "$ms" = "run" ]; then
        case "$mf" in exit=[0-9]*) ;; *) merr "$lineno" "stage=run needs fingerprint 'exit=<N>', got '$mf'" ;; esac
      fi
      # A negative test is correct behaviour, so it is owed to no milestone.
      [ "$mo" = "-" ] || merr "$lineno" "class=reject must have owner '-' (it is coverage, not a debt), got '$mo'"
      [ "$mn" != "-" ] || merr "$lineno" "class=reject needs a note"
      ;;
    skip)
      [ "$ms" = "-" ] || merr "$lineno" "class=skip must have stage '-', got '$ms'"
      [ "$mf" = "-" ] || merr "$lineno" "class=skip must have fingerprint '-', got '$mf'"
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
    case "$p" in "$d"/*|"$d") return 0 ;; esac
  done
  return 1
}

# --------------------------------------------------------------------------
# Enumeration — status captured, never discarded
# --------------------------------------------------------------------------
FIND_OUT=$TMPROOT/found
FIND_ERR=$TMPROOT/found.err
# No -type f: tests/integration/test.pd is a symlink, and silently dropping a
# fixture is precisely the failure this manifest exists to prevent.
find "${SCOPES[@]}" -name '*.pd' >"$FIND_OUT" 2>"$FIND_ERR"
find_rc=$?
if [ "$find_rc" -ne 0 ] || [ -s "$FIND_ERR" ]; then
  echo "error: enumeration failed (find exit $find_rc):" >&2
  sed 's/^/       /' "$FIND_ERR" >&2
  exit 2
fi
sort "$FIND_OUT" -o "$FIND_OUT"
# Per scope, not just overall: a valid-but-wrong directory contributing nothing
# would otherwise ride along invisibly behind a scope that did find something.
for d in "${SCOPES[@]}"; do
  if ! grep -q "^$d/" "$FIND_OUT"; then
    echo "error: no .pd fixtures under scope '$d'" >&2
    echo "       an empty corpus is a broken invocation, not a pass." >&2
    exit 2
  fi
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
while IFS= read -r raw_f; do
  f=$(canon "$raw_f")
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
  if [ "$class" = "reject" ]; then MM=REJECT; else MM=XFAIL; fi

  has_main=0
  if grep -qE '^[[:space:]]*(pub[[:space:]]+)?fn[[:space:]]+main[[:space:]]*\(' "$f"; then has_main=1; fi

  # The vacuous marker is documentation for the reader; the manifest is what the
  # gate believes. Requiring them to agree keeps either from drifting alone.
  marker_at=$(grep -nE '^[[:space:]]*//@[[:space:]]*vacuous:' "$f" | head -1 | cut -d: -f1)
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

  if [ "$class" = "skip" ]; then
    if [ "$has_main" -eq 1 ]; then
      printf '%-52s %s\n' "$f" "CLASS_MISMATCH"
      fail "$f [CLASS_MISMATCH] declared skip but defines fn main — it is a program and must be gated"
      continue
    fi
    printf '%-52s %s\n' "$f" "SKIP"
    skip=$((skip+1))
    continue
  fi
  if [ "$has_main" -eq 0 ]; then
    printf '%-52s %s\n' "$f" "CLASS_MISMATCH"
    fail "$f [CLASS_MISMATCH] declared $class but defines no fn main"
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

  log=$TMPROOT/log
  "$PDC" compile "$f" -o "$out" >"$log" 2>&1
  pdc_rc=$?
  diag=$(strip_ansi <"$log" | grep -m1 -E 'error' | head -c 200)

  stage_act=""; detail=""
  if [ "$pdc_rc" -ne 0 ]; then
    if grep -q "gcc compilation failed\|Linking" "$log"; then stage_act="link"; else stage_act="compile"; fi
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
    elif ! diff -u "$golden" "$TMPROOT/stdout" >"$TMPROOT/diff" 2>&1; then
      out_mismatch=1
      detail=$(head -20 "$TMPROOT/diff" | tail -12 | tr '\n' '~')
    fi
  fi

  # ---- verdict -------------------------------------------------------------
  if [ -z "$stage_act" ]; then           # it passed
    case "$class" in
      run)     if [ "$out_mismatch" -eq 1 ]; then
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
               fail "$f [XPASS] declared to fail at $stage_exp but now passes. Do NOT delete the row (the fixture still exists, so that would make it UNDECLARED). In $MANIFEST change its row to:  $f<TAB>run<TAB>-<TAB>expected<TAB>-<TAB>-  and add the transcript ${f%.pd}.expected (generate with CONFORMANCE_BLESS=1, then READ it before committing). Was: $note" ;;
      reject)  printf '%-52s %s\n' "$f" "REJECT_ACCEPTED"
               fail "$f [REJECT_ACCEPTED] the compiler must refuse this program at $stage_exp ('$fp') but accepted it: $note" ;;
    esac
  elif [ "$class" != "xfail" ] && [ "$class" != "reject" ]; then
    case "$stage_act" in
      compile) v=COMPILE_FAIL ;;
      link)    v=LINK_FAIL ;;
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
  elif [ "$stage_act" != "run" ] && ! strip_ansi <"$log" | grep -qF -- "$fp"; then
    printf '%-52s %s\n' "$f" "${MM}_MISMATCH"
    fail "$f [${MM}_MISMATCH] failed at the declared stage but not with the declared diagnostic; expected fingerprint '$fp', actual: $detail"
  elif [ "$class" = "reject" ]; then
    printf '%-52s %s\n' "$f" "REJECTED"
    reject=$((reject+1))
    REJECT_NOTES+=("$f [refused at $stage_exp: $fp] $note")
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
echo "  skip       = declared non-program (no fn main): library module or manifest"
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
