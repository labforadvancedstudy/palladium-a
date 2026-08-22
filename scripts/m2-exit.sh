#!/usr/bin/env bash
# M2's exit criterion, as a command — GI-08. RED until items 1-7 of M2 land.
#
# WHY THIS IS A SCRIPT AND NOT FOUR LINES OF MAKEFILE. It was four lines of
# Makefile, and the aggregation there was `|| rc=1` per inventory. That destroyed
# the tri-state TWICE, and the result was not merely lossy — it was wrong.
# Measured on the first version:
#
#     REQ_MILESTONE=M2 python3 scripts/requirements.py   ->  1   (OWED)
#     make m2-exit                                       ->  2   (NO_VERDICT)
#
# `|| rc=1` folded a NO_VERDICT into OWED on the way in, and then Make folded
# every nonzero recipe status to 2 on the way out. So the truth was "M2 owes 43
# rows" — a measurement — and the exit code said, in this repository's own
# vocabulary, "nothing may be inferred". A gate that cannot tell those apart has
# already lied once, which is the argument scripts/thesis-exit.sh makes about
# itself.
#
# MACHINE CONTRACT. Three-valued, the same three values as thesis-exit.sh:
#
#   0  CLEAR       every inventory ran and none of them is owed anything by M2
#   1  OWED        at least one inventory MEASURED that M2 still owes something
#   2  NO_VERDICT  no inventory says OWED, but at least one would not measure
#
# AGGREGATION, AND WHY OWED DOMINATES NO_VERDICT. If any inventory measured a
# debt, the answer to "is M2 finished" is NO, and another inventory's abstention
# cannot make that false — so OWED wins. If nothing is owed but something
# abstained, the milestone cannot be certified and the answer is NO_VERDICT. Only
# an unbroken sweep of CLEAR is CLEAR. The lattice is therefore
# OWED > NO_VERDICT > CLEAR by precedence, and both nonzero states are nonzero,
# so no consumer that only reads "zero or not" can be made worse by it.
#
# `make m2-exit` CANNOT CARRY THAT: Make maps every nonzero recipe status to 2.
# Consumers that need the distinction must either call this script directly, or
# read the last line of stdout, which is
#
#     M2_EXIT_RESULT <code> <name>
#
# THE CONTRACT ON A CONSUMER, copied deliberately from thesis-exit.sh because a
# second dialect for the same idea is how two gates come to disagree:
#
#   * AWAIT PROCESS COMPLETION before reading. A partial stream may not have it.
#   * Accept exactly ONE occurrence, ANCHORED at the start of a line, as the
#     FINAL line of STDOUT. Do not first-match: a merged stream can end with
#     Make's own `*** [m2-exit] Error 2`, and prose above may quote the token.
#   * Read STDOUT only. The line is never written to stderr.
#
# HOW EACH INVENTORY'S STATUS IS READ, stated exactly, because two of the four
# cannot express the third state and pretending otherwise would be the same
# defect one level down:
#
#   one   scripts/conformance.sh   0 CLEAR · 1 OWED · 2 NO_VERDICT (manifest
#                                  error: the runner refuses, nothing established)
#   two   scripts/test-xfail.py    0 CLEAR · nonzero OWED. TWO-VALUED. Its own
#                                  "the ignored run did not conclude, nothing was
#                                  established" path also exits 1, so an
#                                  abstention there is indistinguishable from a
#                                  debt HERE. Read as OWED, which is the
#                                  conservative direction (never CLEAR) and is a
#                                  residual, not a claim.
#   three cargo test               0 CLEAR · nonzero OWED. TWO-VALUED, same
#                                  caveat: a build failure and a test failure are
#                                  one code.
#   four  scripts/requirements.py  0 CLEAR · 1 OWED · 2 NO_VERDICT, natively.
#
# All four RUN even when an earlier one is red: stopping at the first failure
# reports part of the debt and costs a round trip to discover the rest.
#
# Usage: bash scripts/m2-exit.sh   (= make m2-exit)

set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

YELLOW='\033[1;33m'; GREEN='\033[0;32m'; RED='\033[0;31m'; NC='\033[0m'
CARGO=${CARGO:-cargo}

CLEAR=0; OWED=1; NO_VERDICT=2
verdict=$CLEAR

# fold <code> — OWED dominates NO_VERDICT dominates CLEAR.
fold() {
  case "$1" in
    "$OWED")       verdict=$OWED ;;
    "$NO_VERDICT") [ "$verdict" -eq "$OWED" ] || verdict=$NO_VERDICT ;;
  esac
}

# --self-test: THE LATTICE, EXERCISED WHERE THE REAL RUN CANNOT REACH IT.
#
# On this tree no inventory returns NO_VERDICT, so the whole OWED-vs-NO_VERDICT
# precedence is dead code during a real run — measured: inverting `fold` so that
# NO_VERDICT dominates changed nothing about `make m2-exit`, which still said
# `M2_EXIT_RESULT 1 OWED`. A rule no run exercises is a rule with no control, and
# that is the defect this file was rewritten to remove, one level in. So the fold
# is driven over all 81 combinations of four tri-state inventories and compared
# against the lattice stated in the header: max(OWED > NO_VERDICT > CLEAR).
if [ "${1-}" = "--self-test" ]; then
  fails=0; runs=0
  for a in 0 1 2; do for b in 0 1 2; do for c in 0 1 2; do for d in 0 1 2; do
    verdict=$CLEAR
    fold $a; fold $b; fold $c; fold $d
    want=$CLEAR
    case "$a$b$c$d" in *2*) want=$NO_VERDICT ;; esac
    case "$a$b$c$d" in *1*) want=$OWED ;; esac
    runs=$((runs+1))
    if [ "$verdict" -ne "$want" ]; then
      fails=$((fails+1))
      echo "FAIL fold($a,$b,$c,$d) = $verdict, want $want"
    fi
  done; done; done; done
  # Order independence: the lattice is a max, so the answer cannot depend on
  # which inventory ran first. Checked explicitly because `fold` is stateful.
  verdict=$CLEAR; fold $OWED; fold $NO_VERDICT; first=$verdict
  verdict=$CLEAR; fold $NO_VERDICT; fold $OWED; second=$verdict
  runs=$((runs+1))
  if [ "$first" -ne "$OWED" ] || [ "$second" -ne "$OWED" ]; then
    fails=$((fails+1))
    echo "FAIL OWED must win regardless of order: got $first and $second"
  fi
  if [ "$fails" -eq 0 ]; then
    echo "m2-exit self-test: $runs checks green (the aggregation lattice over all"
    echo "  81 four-inventory combinations, plus order independence: any OWED wins,"
    echo "  else any NO_VERDICT wins, else CLEAR)"
    exit 0
  fi
  echo "m2-exit self-test FAILED: $fails of $runs"
  exit 2
fi

# Inventory one — tri-valued at the source.
printf "${YELLOW}== inventory one of four: .pd fixtures (tests/conformance-manifest.txt) ==${NC}\n"
CONFORMANCE_FORBID_OWNER=M2 bash scripts/conformance.sh tests examples
case $? in 0) fold $CLEAR ;; 1) fold $OWED ;; *) fold $NO_VERDICT ;; esac
echo

# Inventory two — two-valued; see the header.
printf "${YELLOW}== inventory two of four: Rust debt (tests/rust-debt-manifest.txt + #[ignore] reasons) ==${NC}\n"
TEST_XFAIL_FORBID_OWNER=M2 python3 scripts/test-xfail.py
[ $? -eq 0 ] && fold $CLEAR || fold $OWED
echo

# Inventory three — two-valued; see the header.
printf "${YELLOW}== inventory three of four: the ordinary Rust suite (nothing here is allowed to fail) ==${NC}\n"
$CARGO test --release --no-fail-fast
[ $? -eq 0 ] && fold $CLEAR || fold $OWED
echo

# Inventory four — tri-valued at the source. No `--manifest`: the milestone's own
# exit criterion reads the manifest in the repository and nothing else.
printf "${YELLOW}== inventory four of four: requirements (docs/contributing/1.0-requirements.tsv) ==${NC}\n"
REQ_MILESTONE=M2 python3 scripts/requirements.py
case $? in 0) fold $CLEAR ;; 1) fold $OWED ;; *) fold $NO_VERDICT ;; esac
echo

case $verdict in
  0) name=CLEAR
     printf "${GREEN}✓ M2 exit criterion met — nothing in any inventory is owed to M2${NC}\n" ;;
  1) name=OWED
     printf "${RED}✗ M2 is NOT finished — see the OWED_TO_M2 / failure line(s) above${NC}\n" ;;
  *) name=NO_VERDICT
     printf "${RED}✗ NO VERDICT — nothing is reported owed, but an inventory would not measure. Nothing may be inferred about M2.${NC}\n" ;;
esac
echo
echo "M2_EXIT_RESULT $verdict $name"
exit $verdict
