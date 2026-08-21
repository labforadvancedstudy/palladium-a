#!/usr/bin/env bash
# Palladium expected-failure gate for the Rust test suite.
#
# `scripts/conformance.sh` settled what an expected failure means for a .pd
# program: it is declared with a mandatory reason; a declared failure that still
# fails is XFAIL and is fine; a declared failure that PASSES is XPASS and fails
# the gate; and a declared entry that is never evaluated is STALE_XFAIL and also
# fails the gate, because "never ran" must not be indistinguishable from "failed
# as expected". This script applies the same three rules to the Rust tests,
# where the declaration mechanism is `#[ignore = "…"]` rather than a manifest.
#
# THE INVENTORY MUST BE CLOSED IN BOTH DIRECTIONS.
# Reading declarations and running cargo is not enough: an #[ignore] behind a
# `cfg`, in a module nobody links, or in a target that failed to build is
# neither run nor reported, and a gate that only counted what it saw would call
# that green. So every declaration must be observed exactly once, and every
# ignored test observed must have a declaration behind it. Both are keyed by
# <target>::<test name> — `lib` for src/, the file stem for tests/*.rs — so the
# same test name in two binaries is two entries, not one.
#
# TAGS. Rust overloads #[ignore] for two unrelated things, so the reason says
# which:
#
#   XFAIL: <missing feature> (owned by <milestone>)
#           Cannot pass yet. If it passes, that is XPASS: delete the #[ignore]
#           and let it join the regression net.
#           <milestone> is M1..M9, or the literal `unscheduled` for the work
#           MILESTONES.md files under "Not scheduled, and why". Nothing else.
#
#   SLOW:  <why, and roughly how slow>
#           Passes today; excluded only for cost. Allowed to pass, and a failure
#           is a real regression. Because relabelling an XPASS as SLOW would
#           silently retire it from the suite, the SLOW set is an explicit
#           allowlist below, and adding to it means editing this file.
#
# Fails the gate: XPASS, STALE, UNDECLARED, a bare or mistagged #[ignore], an
# XFAIL whose reason names no valid owner, a SLOW test that is off the allowlist
# or that failed, a target that produced no result, and any cargo exit status
# not explained by a declared test failure.
#
# Usage: scripts/test-xfail.sh

set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

# Reviewed allowlist of tests that may be #[ignore]d for cost alone.
SLOW_ALLOWLIST="
stress_test::test_extremely_large_program
"

SOURCES=$(find src tests -name '*.rs' 2>/dev/null | sort)

fail=0
note() { echo "error: $*" >&2; fail=$((fail+1)); }

# The test target a source file compiles into ("" = not a target of its own).
target_of() {
  case "$1" in
    tests/*/*)  echo "" ;;
    tests/*.rs) b=${1#tests/}; echo "${b%.rs}" ;;
    src/*)      echo "lib" ;;
    *)          echo "" ;;
  esac
}

# --- 1. read the declarations ----------------------------------------------
# One record per #[ignore]: "<tag> <target>::<name> <file>:<line> <attribute>".
DECLS=$(
  for f in $SOURCES; do
    t=$(target_of "$f")
    [ -n "$t" ] || continue
    awk -v file="$f" -v target="$t" '
      /^[[:space:]]*#\[ignore/ { attr=$0; attrline=NR; pending=1; next }
      pending && /^[[:space:]]*(async[[:space:]]+)?fn[[:space:]]/ {
        name=$0
        sub(/^[[:space:]]*(async[[:space:]]+)?fn[[:space:]]+/, "", name)
        sub(/[^A-Za-z0-9_].*$/, "", name)
        tag="UNTAGGED"
        if (attr ~ /#\[ignore[[:space:]]*=[[:space:]]*"XFAIL:/)      tag="XFAIL"
        else if (attr ~ /#\[ignore[[:space:]]*=[[:space:]]*"SLOW:/)  tag="SLOW"
        else if (attr ~ /#\[ignore\]/)                               tag="BARE"
        printf "%s %s::%s %s:%d %s\n", tag, target, name, file, attrline, attr
        pending=0; next
      }
      pending && /^[[:space:]]*#\[/ { next }   # other attributes may intervene
      pending { pending=0 }
    ' "$f"
  done
)

while IFS= read -r rec; do
  [ -n "$rec" ] || continue
  tag=${rec%% *}; rest=${rec#* }
  key=${rest%% *}; rest=${rest#* }
  loc=${rest%% *}; attr=${rest#* }
  case "$tag" in
    BARE)
      note "$loc: bare #[ignore] on $key — every expected failure needs a reason: #[ignore = \"XFAIL: <missing feature> (owned by M<n>)\"] or #[ignore = \"SLOW: <why>\"]" ;;
    UNTAGGED)
      note "$loc: #[ignore] reason on $key must start with 'XFAIL: ' or 'SLOW: '" ;;
    XFAIL)
      if ! printf '%s' "$attr" | grep -Eq '\(owned by (M[1-9]|unscheduled)([,;: )]|$)'; then
        note "$loc: XFAIL reason on $key names no valid owner. Use '(owned by M<n>)' with n in 1..9, or '(owned by unscheduled…)' for work under MILESTONES.md 'Not scheduled, and why'."
      fi ;;
    SLOW)
      if ! printf '%s\n' "$SLOW_ALLOWLIST" | grep -qx "$key"; then
        note "$loc: $key is tagged SLOW but is not on the reviewed allowlist in scripts/test-xfail.sh. A passing test must not be retired from the suite by relabelling it."
      fi ;;
  esac
done <<< "$DECLS"

DECLARED_XFAIL=$(printf '%s\n' "$DECLS" | awk '$1=="XFAIL"{print $2}' | sort)
DECLARED_SLOW=$(printf '%s\n'  "$DECLS" | awk '$1=="SLOW"{print $2}'  | sort)
DECLARED_ALL=$(printf '%s\n%s\n' "$DECLARED_XFAIL" "$DECLARED_SLOW" | grep -v '^$' | sort)

dupes=$(printf '%s\n' "$DECLARED_ALL" | uniq -d)
if [ -n "$dupes" ]; then
  note "the same key is declared twice, so a result cannot be attributed: $(printf '%s ' $dupes)"
fi

# --- 2. run the ignored set ------------------------------------------------
log=$(mktemp)
cargo test --release --no-fail-fast -- --ignored >"$log" 2>&1
cargo_rc=$?

# Attribute every result line to the target whose "Running …" line preceded it.
OBSERVED=$(awk '
  /^[[:space:]]*Running unittests src\/lib\.rs/ { target="lib"; next }
  /^[[:space:]]*Running unittests/              { target="bin"; next }
  /^[[:space:]]*Running tests\// {
    t=$2; sub(/^tests\//, "", t); sub(/\.rs$/, "", t); target=t; next
  }
  /^test .* \.\.\. ok$/     { printf "ok %s::%s\n",     target, $2; next }
  /^test .* \.\.\. FAILED$/ { printf "FAILED %s::%s\n", target, $2; next }
' "$log")

# Every target that started must also have reported. Matched on cargo's own
# line shape — `Running <what> (target/…)` — because the compiler under test
# prints its own "   Running Constant Folding" to stdout during these runs.
UNREPORTED=$(awk '
  /^[[:space:]]*(Running|Doc-tests)[^(]*\(target\// || /^[[:space:]]*Doc-tests / {
    if (open) print pending
    pending=$0; sub(/^[[:space:]]+/, "", pending); open=1; next
  }
  /^test result:/ { open=0 }
  END { if (open) print pending }
' "$log")
if [ -n "$UNREPORTED" ]; then
  note "a test target started and never reported a result — it did not run at all:
$(printf '  %s\n' "$UNREPORTED")"
fi

# A non-zero cargo status must be explained by a failing test, not by a build
# or harness error.
if grep -q '^error\[' "$log" || grep -qE '^error: (could not compile|failed to|expected)' "$log"; then
  note "the ignored set did not build: $(grep -m3 -E '^error' "$log" | tr '\n' ' ')"
elif [ "$cargo_rc" -ne 0 ] && ! printf '%s\n' "$OBSERVED" | grep -q '^FAILED '; then
  note "cargo exited $cargo_rc with no failing test to explain it: $(grep -m3 -E '^error' "$log" | tr '\n' ' ')"
fi

OBSERVED_KEYS=$(printf '%s\n' "$OBSERVED" | awk 'NF{print $2}' | sort)

# --- 3. reconcile, both directions -----------------------------------------
XPASS_LIST=(); STALE_LIST=(); UNDECLARED_LIST=(); SLOWFAIL_LIST=()
xfail=0; xpass=0; slow_pass=0

while IFS= read -r rec; do
  [ -n "$rec" ] || continue
  outcome=${rec%% *}; key=${rec#* }
  if printf '%s\n' "$DECLARED_XFAIL" | grep -qx "$key"; then
    if [ "$outcome" = "ok" ]; then xpass=$((xpass+1)); XPASS_LIST+=("$key")
    else xfail=$((xfail+1)); fi
  elif printf '%s\n' "$DECLARED_SLOW" | grep -qx "$key"; then
    if [ "$outcome" = "ok" ]; then slow_pass=$((slow_pass+1))
    else SLOWFAIL_LIST+=("$key"); fi
  else
    UNDECLARED_LIST+=("$key ($outcome)")
  fi
done <<< "$OBSERVED"

while IFS= read -r key; do
  [ -n "$key" ] || continue
  printf '%s\n' "$OBSERVED_KEYS" | grep -qx "$key" || STALE_LIST+=("$key")
done <<< "$DECLARED_ALL"

n_declared_xfail=$(printf '%s\n' "$DECLARED_XFAIL" | grep -c '[^[:space:]]')
n_declared_slow=$(printf '%s\n'  "$DECLARED_SLOW"  | grep -c '[^[:space:]]')
n_observed=$(printf '%s\n' "$OBSERVED_KEYS" | grep -c '[^[:space:]]')

echo "=============================================="
echo "declared: xfail=$n_declared_xfail slow=$n_declared_slow   observed: $n_observed"
echo "ran:      xfail=$xfail xpass=$xpass slow_pass=$slow_pass"
echo "  xfail      = declared missing-feature test, still failing — as expected"
echo "  xpass      = declared failing but PASSED — a stale expectation, fails the gate"
echo "  slow       = passes, excluded only for cost, on the reviewed allowlist"
echo "  stale      = declared but never ran — indistinguishable from failing, fails the gate"
echo "  undeclared = an ignored test with no declaration behind it, fails the gate"
echo "=============================================="

show() {
  local title=$1; shift
  [ "$#" -gt 0 ] || return 0
  echo; echo "$title"; printf '  %s\n' "$@"; fail=$((fail+1))
}

if [ ${#XPASS_LIST[@]} -gt 0 ]; then
  echo
  echo "XPASS — these now pass; delete the #[ignore] so they join the regression net:"
  for key in "${XPASS_LIST[@]}"; do
    printf '  %s\n' "$key"
    printf '%s\n' "$DECLS" | awk -v k="$key" '$2==k{ $1=""; $2=""; $3=""; sub(/^ +/,""); print "      was: " $0 }'
  done
  fail=$((fail+1))
fi

show "STALE — declared expected-failures that never ran (cfg'd out, unlinked, or in a target that did not report):" "${STALE_LIST[@]+"${STALE_LIST[@]}"}"
show "UNDECLARED — ignored tests with no #[ignore] declaration this script could read:" "${UNDECLARED_LIST[@]+"${UNDECLARED_LIST[@]}"}"
show "SLOW test failed — it is declared as passing-but-expensive, so this is a real regression:" "${SLOWFAIL_LIST[@]+"${SLOWFAIL_LIST[@]}"}"

rm -f "$log"

if [ "$fail" -eq 0 ]; then
  echo "✓ every declared expected failure ran, and every one of them is still failing"
  exit 0
fi
echo >&2
echo "$fail problem(s) above." >&2
exit 1
