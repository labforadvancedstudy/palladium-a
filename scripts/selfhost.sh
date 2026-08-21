#!/usr/bin/env bash
# Palladium self-hosting gate.
#
# Self-hosting is a FIXED POINT, not a demo. The receipt is step 4: the C emitted
# by the stage-1 compiler and the C emitted by the stage-2 compiler must be
# byte-identical. A compiler that merely parses its own source is not self-hosting.
#
#   stage0:  pdc (Rust)  compiles  SRC  ->  pdc1
#   stage1:  pdc1        compiles  SRC  ->  c1   -> pdc2
#   stage2:  pdc2        compiles  SRC  ->  c2
#   gate:    c1 == c2
#
# Usage: scripts/selfhost.sh [source.pd]        (default: bootstrap/pdc.pd)

set -uo pipefail
cd "$(dirname "$0")/.."

SRC=${1:-bootstrap/pdc.pd}
PDC=./target/release/pdc
RUNTIME=runtime/palladium_runtime.c
WORK=build_output/selfhost

fail() { echo; echo "❌ SELF-HOSTING GATE FAILED: $*"; exit 1; }

[ -x "$PDC" ]   || fail "$PDC not built (cargo build --release)"
[ -f "$SRC" ]   || fail "source $SRC does not exist — the bootstrap compiler is not written yet"
[ -f "$RUNTIME" ] || fail "$RUNTIME missing — generated C cannot link"

rm -rf "$WORK"; mkdir -p "$WORK"

echo "== stage0: Rust pdc compiles $SRC =="
"$PDC" compile "$SRC" -o selfhost_stage1 >"$WORK/stage0.log" 2>&1 \
  || { tail -20 "$WORK/stage0.log"; fail "stage0: Rust pdc could not compile $SRC"; }
cp build_output/selfhost_stage1 "$WORK/pdc1" || fail "stage0 produced no binary"
echo "   -> $WORK/pdc1"

echo "== stage1: pdc1 compiles $SRC =="
"$WORK/pdc1" "$SRC" "$WORK/c1.c" >"$WORK/stage1.log" 2>&1 \
  || { tail -20 "$WORK/stage1.log"; fail "stage1: pdc1 could not compile $SRC"; }
[ -s "$WORK/c1.c" ] || fail "stage1 emitted no C"
gcc -Iruntime "$WORK/c1.c" "$RUNTIME" -o "$WORK/pdc2" 2>"$WORK/gcc1.log" \
  || { tail -20 "$WORK/gcc1.log"; fail "stage1: emitted C does not compile"; }
echo "   -> $WORK/c1.c ($(wc -l < "$WORK/c1.c") lines) -> $WORK/pdc2"

echo "== stage2: pdc2 compiles $SRC =="
"$WORK/pdc2" "$SRC" "$WORK/c2.c" >"$WORK/stage2.log" 2>&1 \
  || { tail -20 "$WORK/stage2.log"; fail "stage2: pdc2 could not compile $SRC"; }
[ -s "$WORK/c2.c" ] || fail "stage2 emitted no C"
echo "   -> $WORK/c2.c ($(wc -l < "$WORK/c2.c") lines)"

echo "== gate: stage1 output == stage2 output =="
if cmp -s "$WORK/c1.c" "$WORK/c2.c"; then
  echo
  echo "✅ SELF-HOSTING ACHIEVED — fixed point reached."
  echo "   $(shasum "$WORK/c1.c" | cut -d' ' -f1)  c1.c"
  echo "   $(shasum "$WORK/c2.c" | cut -d' ' -f1)  c2.c"
  exit 0
else
  echo
  diff "$WORK/c1.c" "$WORK/c2.c" | head -40
  fail "stage1 and stage2 output differ — the compiler parses itself but is not a fixed point"
fi
