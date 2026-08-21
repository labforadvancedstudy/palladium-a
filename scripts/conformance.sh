#!/usr/bin/env bash
# Palladium conformance runner.
# Compiles + runs every .pd under tests/ and examples/ with the Rust pdc,
# and reports a per-file COMPILE/LINK/RUN verdict.
#
# This is the evidence source for docs/specification/ — a spec claim without a
# green row here is a claim, not a fact.
#
# Two mechanisms exist so that a green run cannot overstate what works:
#
#   XFAIL   tests/conformance-xfail.txt lists programs that are known to fail,
#           each with a mandatory reason. A listed file that fails is XFAIL and
#           does not fail the gate; its reason is reprinted on every run. A
#           listed file that PASSES is XPASS and DOES fail the gate, because an
#           expectation that quietly went stale is the failure mode this repo
#           exists to kill. A listed file that is missing or skipped is STALE
#           and also fails the gate.
#
#   VACUOUS A program whose first line is `//@ vacuous: <reason>` compiles and
#           runs, but proves nothing about the feature it is named after — the
#           placeholder tests that merely print "not yet implemented". These are
#           counted apart from real passes so that `pass=N` means N programs
#           that actually exercise something. A misplaced marker (present but
#           not on line 1) fails the gate rather than being ignored.
#
# Neither mechanism can make the gate check less: every path out of them is
# either a real pass, a declared-and-still-true failure, or an error.
#
# Usage: scripts/conformance.sh [subdir ...]     (default: tests examples)

set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

PDC=./target/release/pdc
OUT_DIR=build_output
XFAIL_MANIFEST=tests/conformance-xfail.txt
if [ "$#" -gt 0 ]; then DIRS=("$@"); else DIRS=(tests examples); fi

if [ ! -x "$PDC" ]; then
  echo "error: $PDC not built. Run: cargo build --release" >&2
  exit 2
fi

# --- expected-failure manifest ---------------------------------------------
XF_PATH=(); XF_REASON=(); XF_HIT=()
manifest_errors=0
if [ -f "$XFAIL_MANIFEST" ]; then
  lineno=0
  while IFS= read -r line || [ -n "$line" ]; do
    lineno=$((lineno+1))
    line=${line%$'\r'}
    line=${line#"${line%%[![:space:]]*}"}   # ltrim
    case "$line" in ''|'#'*) continue ;; esac
    xpath=${line%%[[:space:]]*}
    xreason=${line#"$xpath"}
    xreason=${xreason#"${xreason%%[![:space:]]*}"}
    if [ -z "$xreason" ]; then
      echo "error: $XFAIL_MANIFEST:$lineno: entry '$xpath' has no reason" >&2
      manifest_errors=$((manifest_errors+1))
      continue
    fi
    if [ ! -f "$xpath" ]; then
      echo "error: $XFAIL_MANIFEST:$lineno: '$xpath' does not exist" >&2
      manifest_errors=$((manifest_errors+1))
      continue
    fi
    XF_PATH+=("$xpath"); XF_REASON+=("$xreason"); XF_HIT+=(0)
  done < "$XFAIL_MANIFEST"
fi

# Echo the manifest index of $1, or -1. (bash 3.2: no associative arrays.)
xf_index() {
  local target=$1 i
  i=0
  while [ "$i" -lt "${#XF_PATH[@]}" ]; do
    if [ "${XF_PATH[$i]}" = "$target" ]; then echo "$i"; return 0; fi
    i=$((i+1))
  done
  echo -1
}

pass=0; vacuous=0; xfail=0; xpass=0
compile_fail=0; link_fail=0; run_fail=0; marker_error=0; skipped=0
declare -a FAILED
declare -a XFAIL_NOTES
declare -a VACUOUS_NOTES

printf '%-52s %s\n' "FILE" "VERDICT"
printf '%s\n' "-------------------------------------------------- -------"

while IFS= read -r f; do
  base=$(basename "$f" .pd)

  # Library modules and package manifests are not standalone programs; the
  # driver requires a main function, so running them through the executable
  # gate would report a harness artifact as a language failure.
  if ! grep -qE '^[[:space:]]*(pub[[:space:]]+)?fn[[:space:]]+main[[:space:]]*\(' "$f"; then
    printf '%-52s %s\n' "$f" "SKIP_NO_MAIN"
    skipped=$((skipped+1))
    continue
  fi

  idx=$(xf_index "$f")
  expected_fail=0; expected_reason=""
  if [ "$idx" -ge 0 ]; then
    expected_fail=1
    expected_reason=${XF_REASON[$idx]}
    XF_HIT[idx]=1
  fi

  # A vacuous marker is only honoured on line 1; anywhere else it would be
  # silently ignored and the file would count as a real pass, so that is an error.
  vac_reason=""
  marker_at=$(grep -nE '^[[:space:]]*//@[[:space:]]*vacuous:' "$f" | head -1 | cut -d: -f1)
  if [ -n "$marker_at" ]; then
    if [ "$marker_at" != "1" ]; then
      printf '%-52s %s\n' "$f" "MARKER_MISPLACED"
      FAILED+=("$f [MARKER_MISPLACED] '//@ vacuous:' found on line $marker_at; it is only honoured on line 1")
      marker_error=$((marker_error+1))
      continue
    fi
    vac_reason=$(sed -n '1s|^[[:space:]]*//@[[:space:]]*vacuous:[[:space:]]*||p' "$f")
    if [ -z "$vac_reason" ]; then
      printf '%-52s %s\n' "$f" "MARKER_MISPLACED"
      FAILED+=("$f [MARKER_MISPLACED] '//@ vacuous:' has no reason")
      marker_error=$((marker_error+1))
      continue
    fi
  fi

  # --- compile, link, run; record the outcome without judging it yet --------
  log=$(mktemp)
  outcome=""; detail=""
  if ! "$PDC" compile "$f" -o "$base" >"$log" 2>&1; then
    if grep -q "gcc compilation failed\|Linking" "$log"; then
      outcome="LINK_FAIL"
    else
      outcome="COMPILE_FAIL"
    fi
    detail=$(grep -m1 -E '^\x1b\[1;31merror|error:' "$log" | head -c 160)
  elif [ -x "$OUT_DIR/$base" ]; then
    "$OUT_DIR/$base" >/dev/null 2>&1
    rc=$?
    if [ "$rc" -eq 0 ]; then
      outcome="PASS"
    else
      outcome="RUN_FAIL"; detail="exit=$rc"
    fi
  else
    outcome="NO_BINARY"; detail="compiler reported success but produced no executable"
  fi
  rm -f "$log"

  # --- turn the outcome into a verdict -------------------------------------
  if [ "$outcome" = "PASS" ]; then
    if [ "$expected_fail" -eq 1 ]; then
      verdict="XPASS"; xpass=$((xpass+1))
      FAILED+=("$f [XPASS] now passes; remove it from $XFAIL_MANIFEST (was: $expected_reason)")
    elif [ -n "$vac_reason" ]; then
      verdict="PASS_VACUOUS"; vacuous=$((vacuous+1))
      VACUOUS_NOTES+=("$f — $vac_reason")
    else
      verdict="PASS"; pass=$((pass+1))
    fi
  elif [ "$expected_fail" -eq 1 ]; then
    verdict="XFAIL"; xfail=$((xfail+1))
    XFAIL_NOTES+=("$f [$outcome] $expected_reason")
  else
    verdict="$outcome"
    case "$outcome" in
      COMPILE_FAIL) compile_fail=$((compile_fail+1)) ;;
      LINK_FAIL|NO_BINARY) link_fail=$((link_fail+1)) ;;
      RUN_FAIL) run_fail=$((run_fail+1)) ;;
    esac
    FAILED+=("$f [$outcome] $detail")
  fi

  printf '%-52s %s\n' "$f" "$verdict"
done < <(find "${DIRS[@]}" -name '*.pd' 2>/dev/null | sort)

# --- a manifest entry that was in scope but never evaluated is stale --------
# "In scope" matters: on a partial run (`conformance.sh tests/misc`) an entry
# under another directory was simply not visited, which is not evidence of
# staleness. On the full run every entry is in scope, so nothing escapes.
stale=0
out_of_scope=0
i=0
while [ "$i" -lt "${#XF_PATH[@]}" ]; do
  if [ "${XF_HIT[$i]}" -eq 0 ]; then
    in_scope=0
    for d in "${DIRS[@]}"; do
      case "${XF_PATH[$i]}" in "$d"/*|"$d") in_scope=1; break ;; esac
    done
    if [ "$in_scope" -eq 1 ]; then
      stale=$((stale+1))
      FAILED+=("${XF_PATH[$i]} [STALE_XFAIL] listed in $XFAIL_MANIFEST but never evaluated (it has no fn main, so it is skipped and the entry is meaningless)")
    else
      out_of_scope=$((out_of_scope+1))
    fi
  fi
  i=$((i+1))
done

total=$((pass+vacuous+xfail+xpass+compile_fail+link_fail+run_fail+marker_error))
echo
echo "=============================================="
echo "total=$total pass=$pass (vacuous=$vacuous) xfail=$xfail xpass=$xpass compile_fail=$compile_fail link_fail=$link_fail run_fail=$run_fail skipped=$skipped"
echo "  pass    = compiled, linked, ran, and exercises the feature it is named for"
echo "  vacuous = ran, but only prints that its feature is unimplemented (not coverage)"
echo "  xfail   = declared in $XFAIL_MANIFEST and still failing (see reasons below)"
echo "  xpass   = declared failing but now passes — a stale expectation, fails the gate"
if [ "$out_of_scope" -gt 0 ]; then
  echo "  note: $out_of_scope xfail entr(y/ies) lie outside the scanned dirs and were not checked"
fi
echo "=============================================="

if [ ${#XFAIL_NOTES[@]} -gt 0 ]; then
  echo
  echo "Expected failures (XFAIL) — what is owed:"
  printf '  %s\n' "${XFAIL_NOTES[@]}"
fi

if [ ${#VACUOUS_NOTES[@]} -gt 0 ]; then
  echo
  echo "Vacuous passes — green, but not evidence of a feature:"
  printf '  %s\n' "${VACUOUS_NOTES[@]}"
fi

if [ ${#FAILED[@]} -gt 0 ]; then
  echo
  echo "Failures:"
  printf '  %s\n' "${FAILED[@]}"
fi

[ $((compile_fail + link_fail + run_fail + xpass + marker_error + stale + manifest_errors)) -eq 0 ]
