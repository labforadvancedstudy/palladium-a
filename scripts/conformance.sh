#!/usr/bin/env bash
# Palladium conformance runner.
# Compiles + runs every .pd under tests/ and examples/ with the Rust pdc,
# and reports a per-file COMPILE/LINK/RUN verdict.
#
# This is the evidence source for docs/specification/ — a spec claim without a
# green row here is a claim, not a fact.
#
# Usage: scripts/conformance.sh [subdir ...]     (default: tests examples)

set -uo pipefail
cd "$(dirname "$0")/.."

PDC=./target/release/pdc
OUT_DIR=build_output
DIRS=("${@:-tests examples}")

if [ ! -x "$PDC" ]; then
  echo "error: $PDC not built. Run: cargo build --release" >&2
  exit 2
fi

pass=0; compile_fail=0; link_fail=0; run_fail=0
declare -a FAILED

printf '%-52s %s\n' "FILE" "VERDICT"
printf '%s\n' "-------------------------------------------------- -------"

skipped=0
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
  log=$(mktemp)
  if ! "$PDC" compile "$f" -o "$base" >"$log" 2>&1; then
    if grep -q "gcc compilation failed\|Linking" "$log"; then
      verdict="LINK_FAIL"; link_fail=$((link_fail+1))
    else
      verdict="COMPILE_FAIL"; compile_fail=$((compile_fail+1))
    fi
    FAILED+=("$f [$verdict] $(grep -m1 -E '^\x1b\[1;31merror|error:' "$log" | head -c 160)")
  elif [ -x "$OUT_DIR/$base" ]; then
    if "$OUT_DIR/$base" >/dev/null 2>&1; then
      verdict="PASS"; pass=$((pass+1))
    else
      verdict="RUN_FAIL"; run_fail=$((run_fail+1))
      FAILED+=("$f [RUN_FAIL] exit=$?")
    fi
  else
    verdict="NO_BINARY"; link_fail=$((link_fail+1))
    FAILED+=("$f [NO_BINARY]")
  fi
  rm -f "$log"
  printf '%-52s %s\n' "$f" "$verdict"
done < <(find ${DIRS[@]} -name '*.pd' 2>/dev/null | sort)

total=$((pass+compile_fail+link_fail+run_fail))
echo
echo "=============================================="
echo "total=$total pass=$pass compile_fail=$compile_fail link_fail=$link_fail run_fail=$run_fail skipped=$skipped"
echo "=============================================="

if [ ${#FAILED[@]} -gt 0 ]; then
  echo
  echo "Failures:"
  printf '  %s\n' "${FAILED[@]}"
fi

[ "$pass" -eq "$total" ]
