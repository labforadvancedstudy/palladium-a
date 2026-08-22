#!/usr/bin/env bash
# Regression tests for the `cmd:` and `conformance:` evidence checks.
#
# WHY THIS EXISTS
# `cmd:` evidence was a SHAPE check for as long as it existed: the string had to look
# like `cmd: X -> Y`, and X was never run. It passed every day, over nine false items,
# because passing cost nothing. A gate is worth its exit code and nothing else, so the
# only way to know this one has one is to hand it a lie and require it to say so.
#
# HOW A NEGATIVE CONTROL FAILS VACUOUSLY, AND WHAT IS DONE ABOUT IT
# This repository has already caught a control that counted its own breakage as a
# detection. Three rules here, all mechanical:
#
#   1. THE GREEN CONTROL RUNS FIRST AND IS FATAL. Case 1 is a TRUE index that must pass.
#      If it does not, the harness itself is broken and every later "detected!" would be
#      the harness failing, not the gate working. The script aborts rather than counting
#      those as passes.
#   2. EVERY RED CASE ASSERTS A SPECIFIC MESSAGE, not merely a non-zero exit. Red for the
#      wrong reason is a failure here.
#   3. EVERY INDEX HOLDS EXACTLY ONE ROW, named uniquely, and the assertion names it. A
#      failure that leaked in from another row cannot satisfy it.
#
# Usage: bash scripts/test-doc-evidence.sh

set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

TMP=$(mktemp -d) || exit 2
trap 'rm -rf "$TMP"' EXIT
GREEN=$'\033[0;32m'; RED=$'\033[0;31m'; NC=$'\033[0m'
pass=0; fail=0

# index <name> <implementation> <evidence-item>...  -> writes $TMP/<name>.toml
index() {
  local name=$1 impl=$2; shift 2
  {
    printf '[probe.%s]\n' "$name"
    printf 'description = "throwaway row for scripts/test-doc-evidence.sh"\n'
    printf 'spec = "../../specification/language-spec.md"\n'
    printf 'implementation = "%s"\n' "$impl"
    printf 'evidence = [\n'
    local e
    for e in "$@"; do printf '  "%s",\n' "$e"; done
    printf ']\n'
  } > "$TMP/$name.toml"
}

# run <name> -> sets RC and OUT
run() {
  OUT=$(python3 scripts/check_doc_evidence.py --index-only --index "$TMP/$1.toml" 2>&1)
  RC=$?
}

# expect_green <name> <case>
expect_green() {
  run "$1"
  if [ "$RC" -eq 0 ]; then
    printf '  %sok%s   %s\n' "$GREEN" "$NC" "$2"; pass=$((pass+1))
  else
    printf '  %sFAIL%s %s -- expected exit 0, got %s\n' "$RED" "$NC" "$2" "$RC"
    printf '%s\n' "$OUT" | sed 's/^/         | /' | head -12
    fail=$((fail+1))
  fi
}

# expect_red <name> <case> <substring the message MUST contain>...
expect_red() {
  local name=$1 case=$2; shift 2
  run "$name"
  if [ "$RC" -eq 0 ]; then
    printf '  %sFAIL%s %s -- gate stayed GREEN on a deliberately false item\n' "$RED" "$NC" "$case"
    fail=$((fail+1)); return
  fi
  # Red for the RIGHT reason, and about THIS row.
  local want missing=0
  if ! printf '%s\n' "$OUT" | grep -qF -- "probe.$name"; then
    printf '  %sFAIL%s %s -- went red without naming the row probe.%s\n' \
      "$RED" "$NC" "$case" "$name"
    printf '%s\n' "$OUT" | sed 's/^/         | /' | head -12
    fail=$((fail+1)); return
  fi
  for want in "$@"; do
    if ! printf '%s\n' "$OUT" | grep -qF -- "$want"; then
      printf '  %sFAIL%s %s -- message lacks %s\n' "$RED" "$NC" "$case" "$want"
      missing=1
    fi
  done
  if [ "$missing" -eq 1 ]; then
    printf '%s\n' "$OUT" | sed 's/^/         | /' | head -14
    fail=$((fail+1)); return
  fi
  printf '  %sok%s   %s\n' "$GREEN" "$NC" "$case"; pass=$((pass+1))
}

echo "== the harness must be able to say YES before any NO is worth anything =="

# CASE 1 (FATAL IF RED). Two true items: a positive count and an ABSENCE PROOF. The
# absence proof is here on purpose. `exit 1, 0 lines` is the success case for most of
# this corpus, and a checker written around `check_returncode()` would invert exactly
# these -- passing the liars and failing the honest rows.
index truth implemented \
  "cmd: ls scripts/ -> exit 0, $(ls scripts/ | wc -l | tr -d ' ') lines" \
  "cmd: grep -rn zzz_no_such_identifier_anywhere src/ --include='*.rs' -> exit 1, 0 lines -- the shape most of this corpus uses" \
  "conformance: tests/reject/try_block.pd reject"
expect_green truth "a TRUE index passes (count, absence proof, conformance class)"
if [ "$fail" -ne 0 ]; then
  echo
  echo "${RED}ABORT${NC}: the green control failed, so this harness cannot distinguish"
  echo "       'the gate detected the lie' from 'the harness is broken'. Every case"
  echo "       below would be meaningless. Fix the control first."
  exit 1
fi

echo
echo "== a false result must fail, loudly, with claimed AND actual =="

# CASE 2. The defect that started this: a claim of one line over a command producing many.
# This is the shape of the real item `grep -rn 'effects::' ... -> 1 line`, which produced 8.
index wrong_count unimplemented \
  "cmd: grep -rn 'pub fn' src/lexer/ --include='*.rs' -> exit 0, 1 lines -- deliberately false"
expect_red wrong_count "a false LINE COUNT is rejected, showing claimed and actual" \
  "claimed: exit 0, 1 line(s)" "actual:" "output:"

# CASE 3. Count right, exit status wrong. Checking only one of the two numbers would let
# this through, and the exit status is the half that carries an absence.
index wrong_exit unimplemented \
  "cmd: grep -rn zzz_no_such_identifier_anywhere src/ --include='*.rs' -> exit 0, 0 lines"
expect_red wrong_exit "a false EXIT STATUS is rejected even when the count matches" \
  "claimed: exit 0, 0 line(s)" "actual:  exit 1, 0 line(s)"

# CASE 4. The absence proof must not be invertible. A command that CANNOT LOOK exits 2 and
# prints to stderr; read as a boolean that is "no match", which is how a probe for the LLVM
# backend's CLI flag sat recorded as `exit 1, 0 lines` having never searched anything.
index could_not_look unimplemented \
  "cmd: grep -rn 'x' src/no_such_directory_here/ -> exit 1, 0 lines"
expect_red could_not_look "a command that COULD NOT LOOK is not a proof of absence" \
  "does not exist"

# CASE 4b. THE SAME COMMAND WITH --include, WHICH IS THE DANGEROUS ONE. Measured on BSD
# grep 2.6.0, a missing directory plus --include exits 1, prints nothing, and writes no
# stderr: byte for byte a true absence proof. Twenty-odd real items have this exact shape,
# so a directory rename would leave all of them green over nothing. Found by this harness,
# not by review -- the first version of the executor passed this case.
index missing_dir_include unimplemented \
  "cmd: grep -rn 'x' src/no_such_directory_here/ --include='*.rs' -> exit 1, 0 lines"
expect_red missing_dir_include "an absence over a MISSING PATH is rejected (--include hides it)" \
  "does not exist" "not an absence"

# CASE 4c. And the same shape over a path that DOES exist must still pass, so 4b is a
# check on the path and not a blanket refusal of the corpus's dominant form.
index missing_dir_ok unimplemented \
  "cmd: grep -rn zzz_no_such_identifier_anywhere src/lexer/ --include='*.rs' -> exit 1, 0 lines"
expect_green missing_dir_ok "the same shape over an EXISTING path still passes"

# CASE 5. Prose in the result position. This is the pre-2026-08-22 spelling: it is exactly
# what the old shape check accepted, so it must now be named as unrunnable.
index prose_result unimplemented \
  "cmd: grep -rn 'Region' src/ --include='*.rs' -> 1 line, src/driver/mod.rs:147"
expect_red prose_result "a prose result is rejected: there is nothing to compare" \
  "must be" "exit <N>, <M> lines"

# CASE 6. A quoted claim the output does not contain. Same rule, and same reason, as the
# `src:` check: a citation whose excerpt lacks the thing being claimed.
index bad_quote unimplemented \
  "cmd: ls scripts/ -> exit 0, $(ls scripts/ | wc -l | tr -d ' ') lines -- including \`no_such_file_at_all.py\`"
expect_red bad_quote "a quote absent from the output is rejected" \
  "but the command's output does not contain it"

echo
echo "== a cmd: may not become a second engine for a question a gate already owns =="

# CASE 7-9. The migration is enforced, not merely performed. Without these, the sixteen
# `pdc compile '<program>'` items could walk straight back in.
index inline_program unimplemented \
  "cmd: pdc compile 'fn main() { try { } }' -> exit 1, 1 lines"
expect_red inline_program "an inline pdc program is refused and sent to conformance" \
  "not an observation" "conformance:"

index builds unimplemented \
  "cmd: cargo build --release -> exit 0, 0 lines"
expect_red builds "a build is refused and sent to gate:" "gate: cargo"

index artifact unimplemented \
  "cmd: grep -c '#line' build_output/01_lexical_comments.c -> exit 1, 0 lines"
expect_red artifact "a build artifact is refused: not reproducible from a checkout" \
  "build artifact"

# CASE 10. No shell. A `cmd:` is argv, and an operator is not a hermetic observation.
index operator unimplemented \
  "cmd: grep -rn 'x' src/ ; ls -> exit 0, 0 lines"
expect_red operator "a shell operator is refused" "shell operator"

# CASE 11. Not on the allowlist.
index not_allowed unimplemented \
  "cmd: python3 -c pass -> exit 0, 0 lines"
expect_red not_allowed "a command outside the allowlist is refused" "allowlist"

echo
echo "== conformance: evidence must agree with the manifest that runs it =="

# CASE 12. The class must be what the manifest declares. `PASS` over a `vacuous` fixture
# is how a placeholder that only prints "unimplemented" got counted as coverage; the
# manifest separates those two columns precisely so that cannot happen.
index wrong_class unimplemented \
  "conformance: tests/07_traits_basic.pd run"
expect_red wrong_class "a class the manifest disagrees with is rejected" \
  "declares it 'vacuous'"

# CASE 13. A fixture nobody declared is run by no gate.
index undeclared unimplemented \
  "conformance: tests/reject/../reject/try_block.pd reject"
expect_red undeclared "an undeclared fixture is rejected" "not declared in"

echo
echo "=============================================="
if [ "$fail" -eq 0 ]; then
  echo "${GREEN}doc-evidence gate probe green${NC} -- $pass case(s): the gate passes a true"
  echo "index and goes red, for the stated reason, on every falsehood above."
  echo "=============================================="
  exit 0
fi
echo "${RED}doc-evidence gate probe FAILED${NC} -- $fail of $((pass+fail)) case(s)"
echo "=============================================="
exit 1
