#!/usr/bin/env bash
# Palladium expected-failure gate for the Rust test suite.
#
# `scripts/conformance.sh` already settled what an expected failure means for a
# .pd program: it is declared with a mandatory reason, a declared failure that
# still fails is XFAIL and is fine, and a declared failure that PASSES is XPASS
# and fails the gate, because a quietly stale expectation is the failure mode
# this repo exists to kill. This script applies the same rule to the Rust tests,
# where the declaration mechanism is `#[ignore = "…"]` instead of a manifest.
#
# Rust overloads `#[ignore]` for two unrelated things — "this cannot pass yet"
# and "this is too slow to run every time" — so the reason must start with a tag
# that says which:
#
#   XFAIL: <missing feature> (owned by M<n>)
#           A test that cannot pass because the language feature does not exist.
#           If it passes, that is XPASS and the gate goes red: delete the
#           #[ignore] and let it join the regression net.
#
#   SLOW:  <why, and roughly how slow>
#           A test that does pass and is excluded only for cost. It is allowed
#           to pass.
#
# Three things fail the gate:
#   XPASS            an XFAIL-tagged test passed
#   UNTAGGED         an #[ignore] with no reason, or with neither tag
#   NO_MILESTONE     an XFAIL reason that does not name its owning milestone
#
# Usage: scripts/test-xfail.sh

set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

SOURCES=$(find src tests -name '*.rs' 2>/dev/null | sort)

# --- 1. every #[ignore] must carry a tagged reason -------------------------
tag_errors=0
while IFS= read -r hit; do
  [ -n "$hit" ] || continue
  file=${hit%%:*}
  rest=${hit#*:}
  line=${rest%%:*}
  text=${rest#*:}
  case "$text" in
    *'#[ignore]'*)
      echo "error: $file:$line: bare #[ignore] — every expected failure needs a reason:" >&2
      echo "       #[ignore = \"XFAIL: <missing feature> (owned by M<n>)\"]  or  #[ignore = \"SLOW: <why>\"]" >&2
      tag_errors=$((tag_errors+1))
      ;;
    *'#[ignore = "XFAIL:'*)
      case "$text" in
        *'(owned by '*) ;;
        *)
          echo "error: $file:$line: XFAIL reason does not name a milestone; add '(owned by M<n>)'" >&2
          tag_errors=$((tag_errors+1))
          ;;
      esac
      ;;
    *'#[ignore = "SLOW:'*) ;;
    *)
      echo "error: $file:$line: #[ignore] reason must start with 'XFAIL: ' or 'SLOW: '" >&2
      tag_errors=$((tag_errors+1))
      ;;
  esac
done < <(grep -n '#\[ignore' $SOURCES 2>/dev/null | grep -v ':[[:space:]]*//')

# --- 2. collect the XFAIL / SLOW test names --------------------------------
# The name is the `fn` on the line after the attribute; libtest reports it with
# its module path, so we compare on the trailing segment.
collect() {
  local tag=$1
  # shellcheck disable=SC2086
  grep -A2 -h "^[[:space:]]*#\[ignore = \"$tag:" $SOURCES 2>/dev/null \
    | sed -n 's/^[[:space:]]*\(async[[:space:]]*\)\{0,1\}fn[[:space:]]\{1,\}\([a-zA-Z0-9_]\{1,\}\).*/\2/p' \
    | sort -u
}
XFAIL_NAMES=$(collect XFAIL)
SLOW_NAMES=$(collect SLOW)

# Declarations, not names: the same test name may legitimately appear in two
# test binaries (both declared XFAIL), and both instances have to be run.
count_decls() {
  # shellcheck disable=SC2086
  grep -c -h "^[[:space:]]*#\[ignore = \"$1:" $SOURCES 2>/dev/null | paste -sd+ - | bc
}

overlap=$(comm -12 <(printf '%s\n' "$XFAIL_NAMES") <(printf '%s\n' "$SLOW_NAMES"))
if [ -n "$overlap" ]; then
  echo "error: these test names are tagged both XFAIL and SLOW, so a pass cannot be judged:" >&2
  printf '  %s\n' $overlap >&2
  tag_errors=$((tag_errors+1))
fi

xfail_declared=$(count_decls XFAIL)
slow_declared=$(count_decls SLOW)

# --- 3. run the ignored set and judge each pass ----------------------------
log=$(mktemp)
cargo test --release --no-fail-fast -- --ignored >"$log" 2>&1

declare -a XPASS_LIST
declare -a UNKNOWN_PASS
xfail=0; xpass=0; slow_pass=0; slow_fail=0

while IFS= read -r name; do
  short=${name##*::}
  if printf '%s\n' "$XFAIL_NAMES" | grep -qx "$short"; then
    xpass=$((xpass+1))
    XPASS_LIST+=("$name")
  elif printf '%s\n' "$SLOW_NAMES" | grep -qx "$short"; then
    slow_pass=$((slow_pass+1))
  else
    UNKNOWN_PASS+=("$name")
  fi
done < <(sed -n 's/^test \([A-Za-z0-9_:]*\) \.\.\. ok$/\1/p' "$log")

while IFS= read -r name; do
  short=${name##*::}
  if printf '%s\n' "$SLOW_NAMES" | grep -qx "$short"; then
    slow_fail=$((slow_fail+1))
  else
    xfail=$((xfail+1))
  fi
done < <(sed -n 's/^test \([A-Za-z0-9_:]*\) \.\.\. FAILED$/\1/p' "$log")

echo "=============================================="
echo "declared: xfail=$xfail_declared slow=$slow_declared"
echo "ran:      xfail=$xfail xpass=$xpass slow_pass=$slow_pass slow_fail=$slow_fail"
echo "  xfail = declared missing-feature test, still failing — as expected"
echo "  xpass = declared missing-feature test that PASSED — a stale expectation, fails the gate"
echo "  slow  = passes, excluded only for cost"
echo "=============================================="

rc=0
if [ ${#XPASS_LIST[@]} -gt 0 ]; then
  echo
  echo "XPASS — these now pass; delete the #[ignore] so they join the regression net:"
  for name in "${XPASS_LIST[@]}"; do
    printf '  %s\n' "$name"
    grep -B1 -h "fn ${name##*::}(" $SOURCES 2>/dev/null | grep '#\[ignore' | sed 's/^[[:space:]]*/      was: /'
  done
  rc=1
fi

if [ ${#UNKNOWN_PASS[@]} -gt 0 ]; then
  echo
  echo "UNTAGGED — ignored tests that passed but carry neither tag:" >&2
  printf '  %s\n' "${UNKNOWN_PASS[@]}" >&2
  rc=1
fi

if [ "$slow_fail" -gt 0 ]; then
  echo
  echo "A SLOW test failed — it is declared as passing-but-expensive, so this is a real regression:" >&2
  sed -n 's/^test \([A-Za-z0-9_:]*\) \.\.\. FAILED$/  \1/p' "$log" >&2
  rc=1
fi

if [ "$tag_errors" -gt 0 ]; then
  echo
  echo "$tag_errors #[ignore] declaration error(s) above." >&2
  rc=1
fi

if grep -q '^error: could not compile\|^error\[' "$log"; then
  echo
  echo "The ignored set did not build:" >&2
  grep -m5 '^error' "$log" >&2
  rc=1
fi

rm -f "$log"
[ "$rc" -eq 0 ] && echo "✓ every expected failure is still failing"
exit $rc
