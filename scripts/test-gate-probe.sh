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

# A conditional INSIDE a body. The top-level scan never sees it — parse_block
# records `#if` as an ordinary statement — so the refusal has to be repeated in
# unmodelled_construct(). Without it the `return` counts and the build with
# FEATURE off falls off the end while this reader calls the file clean. A
# refusal enforced only where it was convenient to look is a docstring.
printf 'long long f(long long n) {\n#if FEATURE\n    return 1;\n#endif\n}\n' >"$TMP/cpp_in_body.c"
$PROBE generated-c "$TMP/cpp_in_body.c" >"$TMP/o" 2>&1
check "Net A, #if inside a function body" 2 $?
grep -q 'preprocessor directive inside a function body' "$TMP/o"
check "  and the malfunction names it" 0 $?
# ...including one nested inside a compound, which the statement walk only
# reaches by recursing into both arms.
printf 'long long g(long long n) {\n    if (n) {\n#ifdef X\n        return 1;\n#endif\n        return 3;\n    } else {\n        return 2;\n    }\n}\n' >"$TMP/cpp_nested.c"
$PROBE generated-c "$TMP/cpp_nested.c" >"$TMP/o" 2>&1
check "Net A, a directive nested inside a compound" 2 $?

# The other direction: unreachable code after a `return` must NOT be reported as
# a fall-through. The parser accepts this shape (src/parser/mod.rs,
# already_terminates), so a Net A that refused it would go red on valid output.
printf 'long long f(long long n) {\n    if (n) {\n        return 1;\n        n = n + 2;\n    } else {\n        return 2;\n    }\n}\n' >"$TMP/unreachable.c"
$PROBE generated-c "$TMP/unreachable.c" >"$TMP/o" 2>&1
check "Net A, statements after a return are unreachable, not a fall-through" 0 $?

# REACHABILITY REACHES INTO BREAK DETECTION TOO. `while (1) { return 2; break; }`
# cannot be left: the `break` is dead text. Counting every syntactically present
# break made this a FINDING, and the parser refused the Palladium program that
# produces it — both analyses wrong in the same way, which is the risk of
# mirroring two hand-written readers.
printf 'long long f(long long n) {\n    if (n) {\n        return 1;\n    } else {\n        while (1) {\n            return 2;\n            break;\n        }\n    }\n}\n' >"$TMP/dead_break.c"
$PROBE generated-c "$TMP/dead_break.c" >"$TMP/o" 2>&1
check "Net A, a break after a return does not make a loop escapable" 0 $?
# ...and the guard, with the break FIRST: now it runs, the loop IS escapable,
# and the function really can fall off its end. gcc agrees, so this is a finding
# from both nets.
printf 'long long f(long long n) {\n    if (n) {\n        return 1;\n    } else {\n        while (1) {\n            break;\n        }\n    }\n}\n' >"$TMP/live_break.c"
$PROBE generated-c "$TMP/live_break.c" >"$TMP/o" 2>&1
check "Net A, a reachable break still makes it a finding" 1 $?

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
echo "== a producer noisier than one pipe buffer must still conclude =="
# THE HARNESS MUST NOT MANUFACTURE ITS OWN MALFUNCTION. `run()` waits on the
# CHILD rather than draining its output, which is right — a grandchild can hold
# a pipe open past the timeout — but with a pipe that means nobody empties the
# 64 KiB buffer while the child runs, so a producer that writes more BLOCKS in
# write(2), the parent blocks in wait(), and the harness reports "timed out
# after 300s". That verdict is indistinguishable from a producer that really
# hung, and it is false.
#
# Measured at fcbabca: `cargo test --release --no-fail-fast -- --ignored` emits
# 78296 bytes; through the pipe form it took the full 300s and returned 124,
# through the file form 45s and its real 101. Every caller until then stayed
# under 64 KiB, so the bound was on the INPUTS, not on the harness — which is
# why this case exists rather than a comment saying "outputs are small".
mk pdc_verbose '#!/bin/sh
awk "BEGIN{for(i=0;i<40000;i++) print \"noise line, wider than one pipe buffer in total\"}"
echo "error: No main function found" >&2
exit 1'
start=$(date +%s)
$PROBE pdc-verdict stdlib/std/option.pd --pdc "$TMP/pdc_verbose" --out t_noisy >"$TMP/o" 2>&1
rc=$?; elapsed=$(( $(date +%s) - start ))
check "1.7MB of producer output is classified, not timed out" 0 $rc
if [ "$elapsed" -lt 60 ]; then
  printf '  %sok%s   and it did not stall (%ss)\n' "$GREEN" "$NC" "$elapsed"; pass=$((pass+1))
else
  printf '  %sFAIL%s the harness blocked for %ss on its own capture buffer\n' "$RED" "$NC" "$elapsed"
  fail=$((fail+1))
fi

echo
echo "== the boundary must promise only what it delivers =="
# THE CLAIM THAT WAS TOO BIG. gate_probe.py said reading the output of a producer
# that did not conclude was structurally unexpressible, and Withheld's repr said
# the same. It is not: `r._out` and `v.withheld._b` both hold the bytes —
# deliberately, because `spill()` needs them and because `run()` must hand them
# back before `classify()` can read them. The prose now says exactly that. These
# checks keep prose and code in step: each fails if a future edit re-grows a
# promise the class cannot keep, or lets the bytes back onto a parsed stream.
python3 - >"$TMP/o" 2>&1 <<'PY'
import pathlib, sys, tempfile
sys.path.insert(0, "scripts")
import gate_probe as gp

bad = []
r = gp.Run(["x"], -9, "SECRET")
v = gp.classify(r, reject_codes=())

# 1. The verdict type carries no text, so `res.text` on a possible malfunction
#    is an AttributeError rather than a silent empty string.
if not isinstance(v, gp.Malfunction) or hasattr(v, "text"):
    bad.append("Malfunction has a `text` attribute again")

# 2. No formatting path may carry the bytes: interpolation into a printed line
#    is exactly how output reaches a stream a shell greps.
for how, s in (("repr", repr(v.withheld)), ("str", str(v.withheld)),
               ("f-string", f"{v.withheld}"), ("format", "{}".format(v.withheld)),
               ("Malfunction repr", repr(v))):
    if "SECRET" in s:
        bad.append("%s of a withheld value carries the bytes" % how)

# 3. It must not behave like a string by accident.
if isinstance(v.withheld, str):
    bad.append("Withheld is a str")
try:
    iter(v.withheld)
    bad.append("Withheld is iterable")
except TypeError:
    pass

# 4. The honest channel must keep working, or people route around it.
with tempfile.TemporaryDirectory() as d:
    if v.withheld.spill(pathlib.Path(d) / "spilled").read_text() != "SECRET":
        bad.append("spill() no longer writes the output it withheld")

# 5. And the prose must not re-grow the claim. Only the text BEFORE the
#    "WHAT THIS IS NOT" section is checked: that section names these phrases in
#    order to retract them.
head = pathlib.Path("scripts/gate_probe.py").read_text().split("WHAT THIS IS NOT")[0]
for phrase in ("no accessor", "unexpressible", "cannot be reached",
               "impossible to read", "the only way to the bytes"):
    if phrase in head.lower():
        bad.append("gate_probe.py claims %r again" % phrase)

print("\n".join(bad) if bad else "ok")
sys.exit(1 if bad else 0)
PY
check "the boundary's promises match its code" 0 $?
grep -v '^ok$' "$TMP/o" | sed 's/^/        /' || true

# THE RULE THAT IS ENFORCED, since unreachability is not enforceable in Python:
# reaching for the bytes takes a private name, and no consumer outside
# gate_probe.py may write one. A grep is the whole point — the dishonest path
# has to appear somewhere a gate can see it.
#
# Two files are exempt and both are load-bearing rather than convenient:
# gate_probe.py OWNS the slots, and this file is the ENFORCER — it must spell
# the names in its pattern in order to search for them. Neither forms a verdict
# from producer text, which is the thing being prevented; a reviewer checking
# this rule is reading exactly these two files anyway.
#
# COMMENT LINES ARE EXCLUDED, and that is not a loophole: the retraction this
# whole section exists to make REQUIRES naming `Run._out` and `Withheld._b` in
# prose, in gate_probe.py and in test-xfail.py, to say that they hold the bytes.
# A rule that forbade writing the truth down would push the truth back out of
# the comments, which is the failure mode. Access is what is forbidden, and
# access cannot hide on a line that begins with `#` or `//`.
leaks=$(grep -rnE '\._out\b|\._b\b|\.withheld\._' scripts tests src \
          --include='*.py' --include='*.sh' --include='*.rs' 2>/dev/null \
        | grep -v '^scripts/gate_probe.py:' \
        | grep -v '^scripts/test-gate-probe.sh:' \
        | grep -vE '^[^:]+:[0-9]+:[[:space:]]*(#|//)' || true)
if [ -z "$leaks" ]; then
  printf '  %sok%s   no consumer outside gate_probe.py names a withheld private slot\n' "$GREEN" "$NC"
  pass=$((pass+1))
else
  printf '  %sFAIL%s a consumer reaches past the boundary for withheld bytes:\n' "$RED" "$NC"
  printf '%s\n' "$leaks" | sed 's/^/        /'
  fail=$((fail+1))
fi

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
