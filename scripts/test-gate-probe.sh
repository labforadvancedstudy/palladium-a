#!/usr/bin/env bash
# Fault injection for the typed-result boundary.
#
# The defect this whole boundary exists to kill: a producer prints the diagnostic
# the gate is looking for and is THEN killed, and the gate reads the text and
# issues a green verdict from a process that never finished.
#
#     $ sh -c 'echo "error: No main function found" >&2; kill -9 $$'
#     exit 137, expected diagnostic already on stderr
#
# So every producer gets that case, not just one: `pdc` (Phase 1 verdicts, the
# forced-import probe, the UNUSABLE probes, driver compilation), the Python
# analysis (Net A), and the C compiler (Net B). Each must answer MALFUNCTION (2),
# never a verdict.
#
# Two signal conventions are exercised, because a check written for one silently
# never fires on the other: subprocess reports -9, a POSIX shell reports 137.
#
# Usage: scripts/test-gate-probe.sh

set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

PROBE="python3 scripts/gate_probe.py"
TMP=$(mktemp -d) || exit 2
trap 'rm -rf "$TMP"' EXIT
GREEN=$'\033[0;32m'; RED=$'\033[0;31m'; NC=$'\033[0m'
pass=0; fail=0

check() {  # check <name> <expected_exit> <actual_exit> [detail]
  if [ "$2" = "$3" ]; then
    printf '  %sok%s   %s\n' "$GREEN" "$NC" "$1"; pass=$((pass+1))
  else
    printf '  %sFAIL%s %s — expected exit %s, got %s %s\n' "$RED" "$NC" "$1" "$2" "$3" "${4:-}"
    fail=$((fail+1))
  fi
}

# --- saboteurs -------------------------------------------------------------
# Each prints text the gate would otherwise believe, then dies.
mk() { printf '%s\n' "$2" >"$TMP/$1"; chmod +x "$TMP/$1"; }

mk pdc_sigkill '#!/bin/sh
echo "error: No main function found" >&2
kill -9 $$'
mk pdc_exit137 '#!/bin/sh
echo "error: No main function found" >&2
exit 137'
mk pdc_blocker_sigkill '#!/bin/sh
echo "error: Expected '"'"'fn'"'"' for method, but found '"'"'pub'"'"'" >&2
kill -9 $$'
mk pdc_gcc_sigkill '#!/bin/sh
echo "error: gcc compilation failed:" >&2
echo "error: incompatible integer to pointer conversion" >&2
kill -9 $$'
mk pdc_weird '#!/bin/sh
echo "error: No main function found" >&2
exit 42'
mk cc_sigkill '#!/bin/sh
echo "x.c:3:1: error: non-void function does not return a value [-Werror,-Wreturn-type]" >&2
kill -9 $$'
mk cc_exit137 '#!/bin/sh
echo "x.c:3:1: error: non-void function does not return a value [-Werror,-Wreturn-type]" >&2
exit 137'

echo "== a signaled pdc that ALREADY printed the expected diagnostic =="
# Phase 1 verdict classification.
$PROBE pdc-verdict stdlib/std/option.pd --pdc "$TMP/pdc_sigkill" --out t_v1 >"$TMP/o" 2>&1
check "pdc-verdict, SIGKILL (-9)" 2 $?
$PROBE pdc-verdict stdlib/std/option.pd --pdc "$TMP/pdc_exit137" --out t_v2 >"$TMP/o" 2>&1
check "pdc-verdict, shell-reported 137" 2 $?
$PROBE pdc-verdict stdlib/std/option.pd --pdc "$TMP/pdc_weird" --out t_v3 >"$TMP/o" 2>&1
check "pdc-verdict, unpinned exit 42" 2 $?
$PROBE pdc-verdict stdlib/std/option.pd --pdc /nonexistent/pdc --out t_v4 >"$TMP/o" 2>&1
check "pdc-verdict, producer missing" 2 $?

# The forced-import probe and the UNUSABLE probes share pdc-reject.
$PROBE pdc-reject stdlib/std/option.pd --pdc "$TMP/pdc_blocker_sigkill" --out t_r1 \
  --expect-stage compile --require "Expected 'fn' for method, but found 'pub'" >"$TMP/o" 2>&1
check "pdc-reject, SIGKILL after the expected blocker" 2 $?
$PROBE pdc-reject stdlib/std/option.pd --pdc "$TMP/pdc_gcc_sigkill" --out t_r2 \
  --expect-stage link --require "incompatible integer to pointer conversion" >"$TMP/o" 2>&1
check "pdc-reject, SIGKILL after the expected link diagnostic" 2 $?

echo
echo "== a signaled C compiler that ALREADY printed a return-type error =="
$PROBE generated-c build_output/stdlib_vec_i64.c --cc "$TMP/cc_sigkill" >"$TMP/o" 2>&1
check "Net B, SIGKILL (-9)" 2 $?
$PROBE generated-c build_output/stdlib_vec_i64.c --cc "$TMP/cc_exit137" >"$TMP/o" 2>&1
check "Net B, shell-reported 137" 2 $?
$PROBE generated-c build_output/stdlib_vec_i64.c --cc /nonexistent/cc >"$TMP/o" 2>&1
check "Net B, compiler missing" 2 $?

echo
echo "== the Python analysis (Net A) =="
printf '// no functions at all\n' >"$TMP/empty.c"
$PROBE generated-c "$TMP/empty.c" >"$TMP/o" 2>&1
check "Net A, zero functions recognised" 2 $?
$PROBE generated-c /nonexistent/x.c >"$TMP/o" 2>&1
check "Net A, missing input" 2 $?
printf 'long long deep(long long n) {\n' >"$TMP/deep.c"
for _ in $(seq 1 4000); do printf '    if (n) {\n' >>"$TMP/deep.c"; done
printf '    return 1;\n' >>"$TMP/deep.c"
for _ in $(seq 1 4000); do printf '    }\n' >>"$TMP/deep.c"; done
printf '}\nint main(void){return 0;}\n' >>"$TMP/deep.c"
$PROBE generated-c "$TMP/deep.c" >"$TMP/o" 2>&1
check "Net A, analyser raises (RecursionError)" 2 $?

echo
echo "== unrelated failures must not be read as the expected finding =="
printf '#error no return statement\nlong long f(void){return 1;}\n' >"$TMP/wording.c"
$PROBE generated-c "$TMP/wording.c" >"$TMP/o" 2>&1
check "Net B, unrelated error containing return-type wording" 2 $?

echo
echo "== and the boundary must still report real outcomes correctly =="
$PROBE generated-c build_output/stdlib_vec_i64.c >"$TMP/o" 2>&1
check "clean generated C" 0 $?
$PROBE pdc-verdict stdlib/std/option.pd --pdc ./target/release/pdc --out t_ok >"$TMP/o" 2>&1
check "real pdc rejection is classified, not malfunctioned" 0 $?
grep -q '^VERDICT COMPILE_FAIL' "$TMP/o"
check "  and carries its verdict" 0 $?
grep -q '^BLOCKER PUB_FN_IN_IMPL' "$TMP/o"
check "  and its blocker category" 0 $?

echo
echo "=============================================="
if [ "$fail" -eq 0 ]; then
  printf '%s✓ gate-probe fault injection: %d/%d%s\n' "$GREEN" "$pass" "$((pass+fail))" "$NC"
  echo "=============================================="
  exit 0
fi
printf '%s✗ gate-probe fault injection: %d of %d FAILED%s\n' "$RED" "$fail" "$((pass+fail))" "$NC"
echo "=============================================="
exit 1
