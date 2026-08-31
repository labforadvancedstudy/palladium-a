#!/usr/bin/env bash
# M1's exit criterion, as a command — GI-08. GREEN today, and it has to stay
# green for a reason it can state.
#
# WHY THIS IS A SCRIPT AND NOT SIX LINES OF MAKEFILE. It was six lines of
# Makefile — three inventories aggregated with `|| rc=1` — and GI-08 says
# "Every milestone exit reads BOTH debt inventories and this manifest", which
# `m1-exit` did not do. The Makefile said so in a comment and called it
# deliberate: `docs/contributing/1.0-requirements.tsv` has ZERO rows owned by M1
# (measured: `awk -F'\t' '$2=="M1"' docs/contributing/1.0-requirements.tsv`
# prints nothing), so `REQ_MILESTONE=M1 python3 scripts/requirements.py` abstains
# — exit 2, NO_VERDICT — and appending it with `|| rc=1` would have turned a
# legitimately green target RED for a reason that says nothing about M1.
#
# THAT WAS THE RIGHT MEASUREMENT AND THE WRONG CONCLUSION. "Do not read the
# inventory" and "read it and know what its abstention means" are different
# answers, and only the second one can tell the two rc=2 shapes apart. Both were
# probed before this file was written:
#
#   $ REQ_MILESTONE=M1 python3 scripts/requirements.py          # rc=2, STDOUT
#   NO_VERDICT: no row of docs/contributing/1.0-requirements.tsv is owned by M1.
#   A filter whose subject matches nothing clears everything, so this is refused
#   rather than reported as 'nothing owed'.
#
#   $ REQ_MILESTONE=M1 python3 scripts/requirements.py --manifest /nonexistent/no.tsv
#   NO_VERDICT: cannot read /nonexistent/no.tsv: [Errno 2] No such file ...  # rc=2, STDERR
#
# Same exit code, different facts. The first says the inventory RAN and has
# nothing of M1's to own; the second says the inventory could not be read at all.
# `scripts/requirements.py:499` prints the first from `report()`; `:207` raises
# the ManifestError that `:788-791` prints as the second, and `:515` prints a
# third ("every M1 row is `satisfied`, but the steps listed above did not run"),
# which is an abstention about EVIDENCE and is not tolerable either.
#
# THE MAPPING, WHICH IS THE WHOLE DECISION:
#
#   rc=0                                   CLEAR
#   rc=1                                   OWED       — reddens m1-exit
#   rc=2 AND the zero-row sentence for M1  TOLERATED  — folds to CLEAR, and the
#                                                       sentence is REPRINTED
#                                                       verbatim, never swallowed
#   rc=2 otherwise                         NO_VERDICT — fail closed
#
# A milestone owes nothing to an inventory that has nothing of its to own, and
# the sentence saying so is the receipt. An abstention that is tolerated in
# silence is indistinguishable from an inventory that was never consulted, which
# is the defect GI-08 exists to close — so the tolerance is conditional on the
# sentence being PRINTED, and the runner asserts it is there.
#
# THE TOLERANCE IS KEYED ON THE EXIT CODE FIRST AND THE SENTENCE SECOND, in that
# order, and the self-test below has a control for the inversion: an rc=1 run
# whose text happens to contain the sentence must still be OWED. A tolerance that
# fired on text alone could mask a measured debt, which is the one thing it must
# never do.
#
# MACHINE CONTRACT. Three-valued, the same three values and the same lattice as
# scripts/m2-exit.sh, because a second dialect for the same idea is how two gates
# come to disagree:
#
#   0  CLEAR       every inventory ran and none of them is owed anything by M1
#   1  OWED        at least one inventory MEASURED that M1 still owes something
#   2  NO_VERDICT  no inventory says OWED, but at least one would not measure
#
# This replaces a two-valued contract, and that is a widening rather than a
# break: 0 still means the same thing, and every previous 1 is still nonzero.
# The Makefile used to record the collapse as a live residual — "`m1-exit` HAS
# THE SAME AMBIGUITY AND IS DELIBERATELY NOT CHANGED HERE... Recorded, not
# fixed." It is fixed here, because the mapping above needs a state that means
# "would not measure" and folding it onto OWED would report an abstention as a
# measurement — the exact defect scripts/m2-exit.sh was written to remove.
#
# `make m1-exit` CANNOT CARRY THAT: Make maps every nonzero recipe status to 2.
# Consumers that need the distinction must either call this script directly, or
# read the last line of stdout, which is
#
#     M1_EXIT_RESULT <code> <name>
#
# THE CONTRACT ON A CONSUMER, copied deliberately from scripts/m2-exit.sh:
#
#   * AWAIT PROCESS COMPLETION before reading. A partial stream may not have it.
#   * Accept exactly ONE occurrence, ANCHORED at the start of a line, as the
#     FINAL line of STDOUT. Do not first-match: a merged stream can end with
#     Make's own `*** [m1-exit] Error 2`, and prose above may quote the token.
#   * Read STDOUT only. The line is never written to stderr.
#
# HOW EACH INVENTORY'S STATUS IS READ, stated exactly, because two of the four
# cannot express the third state:
#
#   one   scripts/conformance.sh   0 CLEAR · 1 OWED · 2 NO_VERDICT (manifest
#                                  error: the runner refuses, nothing established)
#   two   scripts/test-xfail.py    0 CLEAR · nonzero OWED. TWO-VALUED. Its own
#                                  "the ignored run did not conclude" path also
#                                  exits 1, so an abstention there is
#                                  indistinguishable from a debt HERE. Read as
#                                  OWED, the conservative direction (never
#                                  CLEAR); a residual, not a claim.
#   three cargo test               0 CLEAR · nonzero OWED. TWO-VALUED, same
#                                  caveat: a build failure and a test failure are
#                                  one code.
#   four  scripts/requirements.py  0 CLEAR · 1 OWED · 2 NO_VERDICT natively, plus
#                                  the TOLERATED reading of one named rc=2 shape.
#
# All four RUN even when an earlier one is red: stopping at the first failure
# reports part of the debt and costs a round trip to discover the rest.
#
# Usage: bash scripts/m1-exit.sh   (= make m1-exit)

set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

YELLOW='\033[1;33m'; GREEN='\033[0;32m'; RED='\033[0;31m'; NC='\033[0m'
CARGO=${CARGO:-cargo}

CLEAR=0; OWED=1; NO_VERDICT=2
MILESTONE=M1
verdict=$CLEAR

# fold <code> — OWED dominates NO_VERDICT dominates CLEAR.
fold() {
  case "$1" in
    "$OWED")       verdict=$OWED ;;
    "$NO_VERDICT") [ "$verdict" -eq "$OWED" ] || verdict=$NO_VERDICT ;;
  esac
}

# The one rc=2 shape this target tolerates, as the sentence that identifies it.
#
# ANCHORED AT BOTH ENDS, over the WHOLE sentence, and NAMING THE MILESTONE. The
# first version anchored only the start and stopped at the period after the
# milestone, which made it a PREFIX test: probed, `NO_VERDICT: no row of X is
# owned by M1. parsing then failed` matched it, and so did the genuine sentence
# with anything at all appended. A prefix of the right line is not the right
# line. `report()` (scripts/requirements.py:499) writes the whole thing as ONE
# line — the period after the milestone is mid-sentence, not the end of it — so
# the tail anchor has to sit after `'nothing owed'.` and nowhere earlier. Only
# `.*` for the manifest path is left free, because that path is an argument.
TOLERATED_RE="^NO_VERDICT: no row of .* is owned by ${MILESTONE}\. A filter whose subject matches nothing clears everything, so this is refused rather than reported as 'nothing owed'\.$"

# classify_four <rc> <stdout-file> — echoes CLEAR|OWED|TOLERATED|NO_VERDICT.
#
# STDOUT ONLY, and that is not incidental: the two fail-closed rc=2 shapes that
# come from `main()` print their explanation on STDERR, and the tolerable one
# comes from `report()` on STDOUT. Reading a merged stream would let a stderr
# line be tested for a stdout contract.
# THE EXIT CODE IS AN ALLOW-LIST, NOT A FALL-THROUGH. Every stated contract says
# "rc=2 AND the sentence"; the first version said 0, 1, and then applied the
# pattern to everything else, so rc=3 / 126 / 127 / 130 / 139 carrying the
# sentence were all TOLERATED. `python3` can exit on a signal after it has
# already printed, and a process that did not finish is not an abstention.
classify_four() {
  case "$1" in
    0) echo CLEAR; return ;;
    1) echo OWED;  return ;;      # a MEASUREMENT. Never tolerated, whatever the text says.
    2) ;;                         # the ONLY code from which TOLERATED is reachable
    *) echo NO_VERDICT; return ;; # any other code, whatever the text says
  esac
  if grep -qE "$TOLERATED_RE" "$2"; then echo TOLERATED; else echo NO_VERDICT; fi
}

# --self-test: THE LATTICE AND THE MAPPING, EXERCISED WHERE THE REAL RUN CANNOT
# REACH THEM.
#
# On this tree no inventory returns OWED and only inventory four abstains, so
# most of the fold and every fail-closed branch of `classify_four` is dead code
# during a real run. A rule no run exercises is a rule with no control. The fold
# is driven over all 81 combinations of four tri-state inventories against the
# lattice in the header, and the classifier is driven over every rc=2 shape
# `scripts/requirements.py` can produce — three of them PRODUCED LIVE by that
# script rather than pasted here, so that a reworded sentence breaks this test
# instead of silently un-tolerating the real run.
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
  verdict=$CLEAR; fold $OWED; fold $NO_VERDICT; first=$verdict
  verdict=$CLEAR; fold $NO_VERDICT; fold $OWED; second=$verdict
  runs=$((runs+1))
  if [ "$first" -ne "$OWED" ] || [ "$second" -ne "$OWED" ]; then
    fails=$((fails+1))
    echo "FAIL OWED must win regardless of order: got $first and $second"
  fi

  T=$(mktemp -d) || exit 2
  trap 'rm -rf "$T"' EXIT

  # check <label> <want> <rc> <stdout-file>
  check() {
    runs=$((runs+1))
    got=$(classify_four "$3" "$4")
    if [ "$got" = "$2" ]; then
      echo "  m1-exit classify: $1 -> $got"
    else
      fails=$((fails+1)); echo "FAIL classify: $1 -> $got, want $2"
    fi
  }

  # LIVE shape 1 — the one this target tolerates. Produced by the real reader
  # against the real manifest, so the sentence is not a copy.
  REQ_MILESTONE=$MILESTONE python3 scripts/requirements.py \
      >"$T/zero.out" 2>"$T/zero.err"; zrc=$?
  runs=$((runs+1))
  if [ "$zrc" -ne 2 ]; then
    fails=$((fails+1))
    echo "FAIL the zero-row probe must exit 2, got $zrc"
  fi
  check "live zero-row abstention for $MILESTONE (rc=$zrc)" TOLERATED "$zrc" "$T/zero.out"

  # LIVE shape 2 — the manifest cannot be read. FAIL CLOSED.
  REQ_MILESTONE=$MILESTONE python3 scripts/requirements.py \
      --manifest "$T/absent.tsv" >"$T/unread.out" 2>"$T/unread.err"; urc=$?
  check "live unreadable manifest (rc=$urc)" NO_VERDICT "$urc" "$T/unread.out"

  # LIVE shape 3 — every row satisfied but no evidence resolved. Also rc=2, also
  # NOT tolerable: it is an abstention about EVIDENCE, not about ownership.
  printf '# planted by scripts/m1-exit.sh --self-test\n' > "$T/sat.tsv"
  printf 'X-01\t%s\tN5\ta planted requirement\tfixture\ttests/planted.pd\tsatisfied\t1.0\t-\n' \
    "$MILESTONE" >> "$T/sat.tsv"
  REQ_MILESTONE=$MILESTONE python3 scripts/requirements.py \
      --manifest "$T/sat.tsv" >"$T/sat.out" 2>"$T/sat.err"; src=$?
  check "live all-satisfied, evidence unresolved (rc=$src)" NO_VERDICT "$src" "$T/sat.out"

  # THE INVERSION CONTROL. The tolerance keys on the exit code FIRST. An rc=1 run
  # carrying the tolerated sentence verbatim is still OWED — a tolerance that
  # fired on text alone could mask a measured debt.
  cp "$T/zero.out" "$T/owed-with-sentence.out"
  echo "OWED_TO_${MILESTONE} X-01 [owed] a planted requirement" >> "$T/owed-with-sentence.out"
  check "rc=1 carrying the tolerated sentence is still OWED" OWED 1 "$T/owed-with-sentence.out"

  # THE WRONG-MILESTONE CONTROL. The sentence names the milestone, so M2's
  # abstention may not be read as M1's.
  sed "s/is owned by ${MILESTONE}\./is owned by M2./" "$T/zero.out" > "$T/other.out"
  check "another milestone's abstention is NOT tolerated" NO_VERDICT 2 "$T/other.out"

  # THE EXIT-CODE CONTROLS. Every stated contract — this header, the Makefile
  # block above `m1-exit`, MILESTONES item 8 — says "rc=2 AND the sentence". The
  # first version of `classify_four` handled 0 and 1 and then applied the pattern
  # to EVERY other code, so rc=3 (the compiler's own gcc-failure code), and 126,
  # 127, 130 and 139 — could-not-execute, not-found, SIGKILL, SIGSEGV — were all
  # tolerated whenever the text happened to carry the sentence. A python3 that
  # dies on a signal after printing is not an abstention; it is an inventory that
  # did not finish, which is the one thing fail-closed exists for.
  for badrc in 3 126 127 130 139; do
    check "rc=$badrc carrying the genuine tolerated sentence is NOT tolerated" \
      NO_VERDICT "$badrc" "$T/zero.out"
  done

  # THE TAIL-ANCHOR CONTROLS. The pattern must match the WHOLE sentence and not a
  # prefix of a line, or any line that merely begins the right way is tolerated.
  # Both near-misses below were probed against the unanchored pattern and both
  # matched it.
  printf 'NO_VERDICT: no row of %s is owned by %s. parsing then failed\n' \
    "docs/contributing/1.0-requirements.tsv" "$MILESTONE" > "$T/truncated.out"
  check "a truncated sentence with other text after it is NOT tolerated" \
    NO_VERDICT 2 "$T/truncated.out"

  sed "s/reported as 'nothing owed'\./reported as 'nothing owed'. and then it died/" \
    "$T/zero.out" > "$T/tail.out"
  check "the genuine sentence with text appended after it is NOT tolerated" \
    NO_VERDICT 2 "$T/tail.out"

  check "rc=0 is CLEAR" CLEAR 0 "$T/zero.out"

  if [ "$fails" -eq 0 ]; then
    echo "m1-exit self-test: $runs checks green (the aggregation lattice over all"
    echo "  81 four-inventory combinations, plus order independence; and the"
    echo "  inventory-four mapping over every rc=2 shape scripts/requirements.py"
    echo "  can produce, three of them generated live, plus the controls on both"
    echo "  edges of the tolerance: it cannot mask a debt, cannot read another"
    echo "  milestone's abstention as M1's, is unreachable from any exit code but"
    echo "  2, and matches the WHOLE sentence rather than a prefix of its line)"
    exit 0
  fi
  echo "m1-exit self-test FAILED: $fails of $runs"
  exit 2
fi

# Inventory one — tri-valued at the source.
printf "${YELLOW}== inventory one of four: .pd fixtures (tests/conformance-manifest.txt) ==${NC}\n"
CONFORMANCE_FORBID_OWNER=$MILESTONE bash scripts/conformance.sh tests examples
case $? in 0) fold $CLEAR ;; 1) fold $OWED ;; *) fold $NO_VERDICT ;; esac
echo

# Inventory two — two-valued; see the header.
printf "${YELLOW}== inventory two of four: Rust debt (tests/rust-debt-manifest.txt + #[ignore] reasons) ==${NC}\n"
TEST_XFAIL_FORBID_OWNER=$MILESTONE python3 scripts/test-xfail.py
[ $? -eq 0 ] && fold $CLEAR || fold $OWED
echo

# Inventory three — two-valued; see the header.
printf "${YELLOW}== inventory three of four: the ordinary Rust suite (nothing here is allowed to fail) ==${NC}\n"
$CARGO test --release --no-fail-fast
[ $? -eq 0 ] && fold $CLEAR || fold $OWED
echo

# Inventory four — tri-valued at the source, plus the tolerated reading of one
# named rc=2 shape. No `--manifest`: the milestone's own exit criterion reads the
# manifest in the repository and nothing else.
printf "${YELLOW}== inventory four of four: requirements (docs/contributing/1.0-requirements.tsv) ==${NC}\n"
four_out=$(mktemp) || exit 2
# Installed AT CREATION and not only at the `rm` below: every path out of this
# script between here and there — a signal, a `set -u` trip, an `exit` added
# later — would otherwise leave the file behind.
trap 'rm -f "$four_out"' EXIT INT TERM
REQ_MILESTONE=$MILESTONE python3 scripts/requirements.py > "$four_out"
four_rc=$?
cat "$four_out"
case "$(classify_four "$four_rc" "$four_out")" in
  CLEAR) fold $CLEAR ;;
  OWED)  fold $OWED ;;
  TOLERATED)
    # PRINTED, NEVER SWALLOWED. The tolerance is only defensible while the
    # sentence it rests on is in the transcript, so it is repeated here under a
    # heading that says what was done with it.
    printf "${YELLOW}-- inventory four ABSTAINED, and this exit tolerates that abstention --${NC}\n"
    printf "   %s owns no row of docs/contributing/1.0-requirements.tsv, so the\n" "$MILESTONE"
    printf "   inventory has nothing of %s's to report. It RAN, it was READ, and its\n" "$MILESTONE"
    printf "   own sentence for this case is quoted above and again here:\n"
    grep -E "$TOLERATED_RE" "$four_out" | sed 's/^/     /'
    printf "   Any OTHER exit-2 shape from this inventory is NOT tolerated and reddens\n"
    printf "   this target; scripts/m1-exit.sh --self-test proves both directions.\n"
    fold $CLEAR ;;
  *) fold $NO_VERDICT ;;
esac
rm -f "$four_out"
trap - EXIT INT TERM
echo

case $verdict in
  0) name=CLEAR
     printf "${GREEN}✓ M1 exit criterion met — nothing in any inventory is owed to M1${NC}\n" ;;
  1) name=OWED
     printf "${RED}✗ M1 is NOT finished — see the OWED_TO_M1 / failure line(s) above${NC}\n" ;;
  *) name=NO_VERDICT
     printf "${RED}✗ NO VERDICT — nothing is reported owed, but an inventory would not measure. Nothing may be inferred about M1.${NC}\n" ;;
esac
echo
echo "M1_EXIT_RESULT $verdict $name"
exit $verdict
