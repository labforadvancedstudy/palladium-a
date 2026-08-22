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
printf '}\nint main(void) {\n    return 0;\n}\n' >>"$TMP/deep.c"
$PROBE generated-c "$TMP/deep.c" >"$TMP/o" 2>&1
check "Net A, analyser raises (RecursionError)" 2 $?

echo
echo "== Net A coverage must be CLOSED, not sampled =="
# THE HOLE THIS CLOSES. Net A's only coverage alarm used to be "ZERO functions
# recognised", which cannot fire on a MIXED file: one definition in the shape it
# knows makes the denominator nonzero while every other definition is skipped in
# silence. Each fixture below pairs one recognised definition with one shape the
# reader cannot account for, and every one of them must MALFUNCTION (2) — a
# verdict of 0 would be the same disease as the `} else if` false negative that
# certified a shipped milestone.
mixed() {  # mixed <name> <second-definition-text>
  { printf 'long long ok(long long n) {\n    return n;\n}\n'; printf '%b' "$2"; } >"$TMP/$1.c"
  $PROBE generated-c "$TMP/$1.c" >"$TMP/o" 2>&1
  check "Net A, mixed file: $1" 2 $?
}
mixed same_line_body     'long long hidden(void) { return 1; }\n'
mixed multiline_def      'long long\nhidden(void)\n{\n    return 1;\n}\n'
mixed conditional_cpp    '#ifdef X\nlong long hidden(void) {\n}\n#endif\n'
mixed unclosed_body      'long long hidden(void) {\n    return 1;\n'
# A `goto` out of `while (1)` makes the loop escapable; `contains_break` only
# looks for `break`. This function DOES fall off its end (gcc: "control reaches
# end of non-void function"), and with the loop as the first item of the body
# the terminator scan would answer "cannot fall through" — a silent false
# negative, and the only one the whole-list scan introduced. Detecting the
# construct and malfunctioning is what pays for that scan.
printf 'long long jumps(long long n) {\n    while (1) {\n        goto done;\n    }\ndone:\n    ;\n}\n' >"$TMP/goto.c"
$PROBE generated-c "$TMP/goto.c" >"$TMP/o" 2>&1
check "Net A, goto out of while(1) is not read as inescapable" 2 $?
grep -q 'goto' "$TMP/o"
check "  and the malfunction names the construct" 0 $?

# The other direction: unreachable code after a `return` must NOT be reported as
# a fall-through. The parser accepts this shape (src/parser/mod.rs,
# already_terminates), so a Net A that refused it would go red on valid output.
printf 'long long f(long long n) {\n    if (n) {\n        return 1;\n        n = n + 2;\n    } else {\n        return 2;\n    }\n}\n' >"$TMP/unreachable.c"
$PROBE generated-c "$TMP/unreachable.c" >"$TMP/o" 2>&1
check "Net A, statements after a return are unreachable, not a fall-through" 0 $?

echo
echo "== unrelated failures must not be read as the expected finding =="
# The definition is multi-line ON PURPOSE: written on one line, Net A stops the
# file as unaccounted-for and Net B — the thing this case is about — is never
# reached. Measured at 199c7bd: the one-line form exited 2 from "no function
# definitions recognised", so this check passed without ever running a C
# compiler.
printf '#error no return statement\nlong long f(void) {\n    return 1;\n}\n' >"$TMP/wording.c"
$PROBE generated-c "$TMP/wording.c" >"$TMP/o" 2>&1
check "Net B, unrelated error containing return-type wording" 2 $?
grep -q 'Net B' "$TMP/o"
check "  and it really was Net B that objected" 0 $?

echo
echo "== exec-layer faults must be malfunctions, not findings =="
printf '#!/bin/sh\necho hi\n' >"$TMP/noperm"; chmod 000 "$TMP/noperm"
$PROBE pdc-verdict stdlib/std/option.pd --pdc "$TMP/noperm" --out t_p1 >"$TMP/o" 2>&1
check "producer not executable (PermissionError)" 2 $?
printf 'not a program\n' >"$TMP/notexec"; chmod +x "$TMP/notexec"
$PROBE pdc-verdict stdlib/std/option.pd --pdc "$TMP/notexec" --out t_p2 >"$TMP/o" 2>&1
check "producer is not executable format (ENOEXEC)" 2 $?
$PROBE pdc-verdict stdlib/std/option.pd --pdc "$TMP" --out t_p3 >"$TMP/o" 2>&1
check "producer is a directory" 2 $?

echo
echo "== Net A import failure is a malfunction =="
mv scripts/check-c-returns.py "$TMP/neta.hidden"
$PROBE generated-c build_output/stdlib_vec_i64.c >"$TMP/o" 2>&1
check "Net A analyser missing" 2 $?
printf 'def check_file(  # syntax error\n' >scripts/check-c-returns.py
$PROBE generated-c build_output/stdlib_vec_i64.c >"$TMP/o" 2>&1
check "Net A analyser does not import" 2 $?
printf 'X = 1\n' >scripts/check-c-returns.py
$PROBE generated-c build_output/stdlib_vec_i64.c >"$TMP/o" 2>&1
check "Net A analyser lacks check_file()" 2 $?
mv "$TMP/neta.hidden" scripts/check-c-returns.py

echo
echo "== the reconciliation cannot exit 1 without a structured finding =="
$PROBE reconcile --src /nonexistent/builtins.rs --manifest tests/stdlib/BUILTINS.tsv >"$TMP/o" 2>&1
check "reconcile, unreadable registry" 2 $?
$PROBE reconcile --src src/builtins.rs --manifest /nonexistent/BUILTINS.tsv >"$TMP/o" 2>&1
check "reconcile, unreadable manifest" 2 $?
sed 's/name:/ident:/g; s/"\([a-z_0-9]*\) param/"\1 FIELD/g; s/"\([a-z_0-9]*\) return/"\1 FIELD/g' \
  src/builtins.rs >"$TMP/broken_builtins.rs"
$PROBE reconcile --src "$TMP/broken_builtins.rs" --manifest tests/stdlib/BUILTINS.tsv >"$TMP/o" 2>&1
check "reconcile, parsing contract broken" 2 $?

echo
echo "== a descendant holding the pipe must not outlive the timeout =="
# The direct child exits immediately; a grandchild keeps the merged pipe open.
# Without process-group kill the read blocks past the timeout instead of
# returning a malfunction.
mk slow_desc '#!/bin/sh
sh -c "sleep 600" &
exit 0'
start=$(date +%s)
$PROBE pdc-verdict stdlib/std/option.pd --pdc "$TMP/slow_desc" --out t_d1 >"$TMP/o" 2>&1
rc=$?; elapsed=$(( $(date +%s) - start ))
if [ "$elapsed" -lt 30 ]; then
  printf '  %sok%s   descendant does not stall the read (%ss, exit %s)\n' "$GREEN" "$NC" "$elapsed" "$rc"
  pass=$((pass+1))
else
  printf '  %sFAIL%s descendant stalled the read for %ss — process-group kill did not work\n' "$RED" "$NC" "$elapsed"
  fail=$((fail+1))
fi
pkill -f "sleep 600" 2>/dev/null || true

echo
echo "== the malfunction path must not republish producer text =="
$PROBE pdc-verdict stdlib/std/option.pd --pdc "$TMP/pdc_sigkill" --out t_w1 >"$TMP/o" 2>&1
if grep -q "No main function found" "$TMP/o"; then
  printf '  %sFAIL%s malfunction output republished the producer diagnostic — it is greppable as a verdict\n' "$RED" "$NC"
  fail=$((fail+1))
else
  printf '  %sok%s   malfunction output withholds the producer diagnostic\n' "$GREEN" "$NC"
  pass=$((pass+1))
fi

echo
echo "== and the boundary must still report real outcomes correctly =="
$PROBE generated-c build_output/stdlib_vec_i64.c >"$TMP/o" 2>&1
check "clean generated C" 0 $?
$PROBE reconcile --src src/builtins.rs --manifest tests/stdlib/BUILTINS.tsv >"$TMP/o" 2>&1
check "reconcile, real registry" 0 $?
$PROBE calibrate --pdc ./target/release/pdc --scratch "$TMP/cal" >"$TMP/o" 2>&1
check "calibrate, real pdc" 0 $?
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
