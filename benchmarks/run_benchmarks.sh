#!/usr/bin/env bash
#
# Palladium benchmark suite -- one command, clean state to raw data.
#
#   bash benchmarks/run_benchmarks.sh
#
# Stages:
#   0. preflight   -- locate pdc, rustc, gcc; fail loudly if any is missing
#   1. clean       -- wipe benchmarks/build/ so nothing stale can be measured
#   2. build       -- Palladium (pdc default AND pdc-C-recompiled-with-gcc-O2),
#                     C (gcc -O2), Rust (rustc -O), plus the fairness variants
#   3. verify      -- run every binary and require byte-identical stdout per
#                     benchmark. A benchmark whose implementations disagree is
#                     void, and this script refuses to time it.
#   4. measure     -- benchmarks/measure.py: N timed runs of every binary and of
#                     every compile command, written to benchmarks/results/
#
# Environment knobs:
#   BENCH_RUNTIME_RUNS  (default 10)
#   BENCH_COMPILE_RUNS  (default 10)
#
# Honesty notes that belong next to the code, not only in the report:
#   * `pdc compile X.pd -o Y` forks gcc with NO optimization flag
#     (src/main.rs:99-105), so the "palladium" variant is an -O0 binary. That is
#     what a Palladium user gets today. `pdc -O` is accepted and ignored
#     (src/main.rs:76: the parameter is bound as `_optimize` and never read).
#   * the "palladium_gccO2" variant recompiles pdc's own generated C with
#     gcc -O2. It measures the C backend's ceiling, not pdc's current output.
#   * pdc always writes its generated C to the repo-global build_output/<stem>.c,
#     keyed by file stem alone. Another process compiling a same-named .pd will
#     clobber it, so every compile below is immediately followed by a copy and a
#     content check.

set -euo pipefail

BENCH_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$BENCH_DIR/.." && pwd)"
cd "$ROOT"

BUILD="$BENCH_DIR/build"
BIN="$BUILD/bin"
GEN="$BUILD/gen"
PDC="$ROOT/target/release/pdc"
RUNTIME_C="$ROOT/runtime/palladium_runtime.c"

BENCHMARKS=(fibonacci bubble_sort matrix_multiply string_concat)

GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; NC='\033[0m'
say()  { printf "%b\n" "$*"; }
ok()   { printf "${GREEN}%b${NC}\n" "$*"; }
warn() { printf "${YELLOW}%b${NC}\n" "$*"; }
die()  { printf "${RED}%b${NC}\n" "$*" >&2; exit 1; }

# ---------------------------------------------------------------- 0. preflight
say "=== Palladium benchmark suite ==="
say ""
say "--- preflight"
[ -x "$PDC" ] || die "Palladium compiler not found at $PDC -- run 'cargo build --release' first"
[ -f "$RUNTIME_C" ] || die "Palladium runtime not found at $RUNTIME_C"
command -v rustc >/dev/null || die "rustc not found"
command -v gcc   >/dev/null || die "gcc not found"
command -v python3 >/dev/null || die "python3 not found (needed by measure.py)"

say "  pdc    : $("$PDC" --version 2>/dev/null | tail -1 | tr -d '\r')  ($(git rev-parse --short HEAD 2>/dev/null || echo 'no git'))"
say "  rustc  : $(rustc --version)"
say "  gcc    : $(gcc --version | head -1)"
if command -v hyperfine >/dev/null; then
  say "  hyperfine: $(hyperfine --version)"
else
  warn "  hyperfine: not installed -- measure.py falls back to its own timing loop"
fi
say "  host   : $(uname -m) / $(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo unknown)"
say "  load   : $(uptime | sed 's/.*load averages*://')"
say ""

# -------------------------------------------------------------------- 1. clean
say "--- clean"
rm -rf "$BUILD"
mkdir -p "$BIN" "$GEN" "$BUILD/compiletest"
ok "  wiped $BUILD"
say ""

# -------------------------------------------------------------------- 2. build
say "--- build"

for b in "${BENCHMARKS[@]}"; do
  src="$BENCH_DIR/palladium/$b.pd"
  [ -f "$src" ] || die "missing $src"

  # pdc's own pipeline. -o takes an absolute path so the artifact lands here
  # instead of in the repo-global build_output/.
  if ! out=$("$PDC" compile "$src" -o "$BIN/${b}_pd" 2>&1); then
    say "$out" | tail -20
    die "pdc failed to compile $src"
  fi
  [ -x "$BIN/${b}_pd" ] || die "pdc produced no executable for $b"

  # Snapshot the generated C immediately -- build_output/ is shared and keyed by
  # stem only, so a concurrent build elsewhere in the repo can overwrite it.
  cp "$ROOT/build_output/$b.c" "$GEN/$b.c"
  grep -q "benchmark: $b" "$GEN/$b.c" \
    || die "build_output/$b.c does not contain this benchmark -- it was clobbered by a concurrent compile; rerun"

  # Same generated C, hand-compiled at -O2: the C backend's ceiling.
  gcc -O2 "$GEN/$b.c" "$RUNTIME_C" -o "$BIN/${b}_pd_O2"

  ok "  palladium  $b  (pdc default -> ${b}_pd, gcc -O2 -> ${b}_pd_O2, generated C $(wc -c < "$GEN/$b.c" | tr -d ' ') B)"
done

for b in "${BENCHMARKS[@]}"; do
  gcc -O2 "$BENCH_DIR/c/$b.c" -o "$BIN/${b}_c"
  ok "  c          $b  (gcc -O2)"
done

RUST_SRCS=(fibonacci bubble_sort bubble_sort_unchecked matrix_multiply
           matrix_multiply_unchecked string_concat string_concat_pushstr)
for r in "${RUST_SRCS[@]}"; do
  rustc -O "$BENCH_DIR/rust/$r.rs" -o "$BIN/${r}_rs"
  ok "  rust       $r  (rustc -O)"
done
say ""

# ------------------------------------------------------------------- 3. verify
# A benchmark whose implementations disagree is void. Nothing gets timed until
# every variant of it prints exactly the same bytes.
say "--- verify output equivalence"
EQUIV_JSON="$BUILD/equivalence.json"
printf '{\n  "method": "every variant is executed and its stdout compared byte-for-byte against the pdc-default build",\n  "benchmarks": {\n' > "$EQUIV_JSON"

first_bench=1
fail=0
for b in "${BENCHMARKS[@]}"; do
  case "$b" in
    fibonacci)       variants=("${b}_pd" "${b}_pd_O2" "${b}_c" "${b}_rs") ;;
    string_concat)   variants=("${b}_pd" "${b}_pd_O2" "${b}_c" "${b}_rs" "${b}_pushstr_rs") ;;
    *)               variants=("${b}_pd" "${b}_pd_O2" "${b}_c" "${b}_rs" "${b}_unchecked_rs") ;;
  esac

  ref="$BUILD/${b}.expected"
  "$BIN/${b}_pd" > "$ref"
  refsha=$(shasum -a 256 < "$ref" | cut -d' ' -f1)

  say "  $b -> $(tr '\n' '|' < "$ref")"
  [ $first_bench -eq 1 ] || printf ',\n' >> "$EQUIV_JSON"
  first_bench=0
  printf '    "%s": { "stdout_sha256": "%s", "stdout": %s, "variants": {' \
    "$b" "$refsha" "$(python3 -c 'import json,sys;print(json.dumps(open(sys.argv[1]).read()))' "$ref")" >> "$EQUIV_JSON"

  first_v=1
  for v in "${variants[@]}"; do
    got="$BUILD/${v}.actual"
    "$BIN/$v" > "$got"
    [ $first_v -eq 1 ] || printf ',' >> "$EQUIV_JSON"
    first_v=0
    if cmp -s "$ref" "$got"; then
      ok "      MATCH    $v"
      printf ' "%s": "match"' "$v" >> "$EQUIV_JSON"
    else
      printf "${RED}      MISMATCH %s${NC}\n" "$v"
      diff "$ref" "$got" | head -10
      printf ' "%s": "MISMATCH"' "$v" >> "$EQUIV_JSON"
      fail=1
    fi
  done
  printf ' } }' >> "$EQUIV_JSON"
done
printf '\n  }\n}\n' >> "$EQUIV_JSON"

[ $fail -eq 0 ] || die "output equivalence FAILED -- these numbers would be meaningless, refusing to measure"
ok "  all variants byte-identical per benchmark"
say ""

# ------------------------------------------------------------------ 4. measure
say "--- measure (runtime + compile)"
python3 "$BENCH_DIR/measure.py"
