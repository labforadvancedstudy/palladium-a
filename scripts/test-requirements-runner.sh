#!/usr/bin/env bash
# GI-09: the owner filter is self-tested — it detects a row planted for the
# milestone under test.
#
# `make m2-exit` is a claim about a milestone, and a claim is worth its ability
# to be false. The other two owner filters already carry their negative controls
# (`CONFORMANCE_FORBID_OWNER` in scripts/test-conformance-runner.sh item7,
# `TEST_XFAIL_FORBID_OWNER` inside scripts/test-xfail.py's self-test), so the
# fourth inventory arrived owing the same proof, and GI-09 is that debt written
# down.
#
# Two halves, and the second is the one that is easy to leave out:
#
#   1. THE FILTER. A row planted for the milestone under test must turn the
#      runner RED and be NAMED; a row planted for a different milestone must
#      not; a milestone with nothing owed must NOT come back green, because the
#      steps that would resolve its evidence do not exist yet.
#
#   2. THE TARGET. `make m2-exit` must still READ all four inventories. Deleting
#      one line of that recipe is invisible to every other gate in the repo —
#      the deleted inventory simply stops being consulted and everything stays
#      green. So the recipe is checked through `make -n`, which is what Make
#      would actually run, rather than by reading the Makefile as text.
#
# Usage: bash scripts/test-requirements-runner.sh   (= make test-requirements-runner)

set -uo pipefail
cd "$(dirname "$0")/.." || exit 2
REPO=$PWD

RED='\033[0;31m'; GREEN='\033[0;32m'; NC='\033[0m'

TMPROOT=$(mktemp -d) || exit 2
trap 'rm -rf "$TMPROOT"' EXIT

tests_run=0; tests_failed=0
CASE=""; OUT=""; RC=0

start() { CASE=$1; tests_run=$((tests_run+1)); }
ok()    { printf "  ${GREEN}PASS${NC} %s\n" "$CASE"; }
bad()   {
  printf "  ${RED}FAIL${NC} %s\n         %s\n" "$CASE" "$1"
  printf '%s\n' "$OUT" | sed 's/^/         | /' | head -20
  tests_failed=$((tests_failed+1))
}
expect_rc()  { if [ "$RC" -eq "$1" ]; then return 0; fi; bad "expected exit $1, got $RC"; return 1; }
expect_out() { case "$OUT" in *"$1"*) return 0 ;; esac; bad "expected output to contain '$1'"; return 1; }
reject_out() { case "$OUT" in *"$1"*) bad "output must NOT contain '$1'"; return 1 ;; esac; return 0; }

# plant <name> <lines...> -> echoes a manifest path. `|` becomes TAB, so the
# nine columns stay readable in the case below.
plant() {
  local f=$TMPROOT/$1.tsv; shift
  {
    echo "# planted by scripts/test-requirements-runner.sh"
    local l
    for l in "$@"; do printf '%s\n' "$l" | tr '|' '\t'; done
  } > "$f"
  printf '%s' "$f"
}

# run_case <manifest-or-empty> <milestone-or-empty> -> sets RC and OUT
run_case() {
  local m=$1 owner=$2
  OUT=$( cd "$REPO" && REQUIREMENTS_MANIFEST="$m" REQ_MILESTONE="$owner" \
         python3 scripts/requirements.py 2>&1 )
  RC=$?
}

OWED='X-01|M2|N5|a planted requirement|fixture|tests/planted.pd|owed|1.0|-'
SAT='X-01|M2|N5|a planted requirement|fixture|tests/planted.pd|satisfied|1.0|-'
OTHER='Y-01|M7|N5|another milestone|fixture|tests/other.pd|owed|1.0|-'

echo "requirements runner regression tests (GI-09)"
echo

# ---------------------------------------------------------------------------
# The runner's own parser and verdict map. It refuses to run its real job until
# these pass, so this is the first thing that must hold.
# ---------------------------------------------------------------------------
start "the runner's built-in self-test passes"
OUT=$(cd "$REPO" && python3 scripts/requirements.py --self-test 2>&1); RC=$?
expect_rc 0 && expect_out "self-test:" && ok

# ---------------------------------------------------------------------------
# HALF ONE — the filter.
# ---------------------------------------------------------------------------
start "planted: a row owed to the milestone under test turns the gate RED"
M=$(plant owed "$OWED" "$OTHER")
run_case "$M" M2
expect_rc 1 && expect_out "OWED_TO_M2 X-01" && ok

start "planted: the RED names the requirement, not just a count"
expect_out "a planted requirement" && ok

start "planted: and it names the evidence that has to start passing"
expect_out "evidence: fixture tests/planted.pd" && ok

start "planted: a row owed to ANOTHER milestone does not trip this one"
M=$(plant other "$SAT" "$OTHER")
run_case "$M" M2
reject_out "OWED_TO_M2" && ok

start "planted: ...and that same manifest IS red for the milestone that owns it"
run_case "$M" M7
expect_rc 1 && expect_out "OWED_TO_M7 Y-01" && ok

start "planted: 'blocked' is not 'satisfied' — a row that is a question is owed"
M=$(plant blocked 'X-01|M2|N5|a blocked requirement|decision|MILESTONES.md|blocked|1.0|-')
run_case "$M" M2
expect_rc 1 && expect_out "OWED_TO_M2 X-01" && ok

# THE FAIL-OPEN THIS GATE EXISTS TO REFUSE. Steps 3 and 4 of the specification
# (resolve each evidence locator and RUN it; reconcile the debt inventories by
# `req:` id) are NOT implemented, so "no row says owed" is a statement about the
# status column and not about the compiler. A green here would be the M1 defect
# again in a new inventory.
start "planted: a milestone with nothing owed is NO_VERDICT, not green"
M=$(plant satisfied "$SAT")
run_case "$M" M2
expect_rc 2 && expect_out "NO_VERDICT" && ok

start "planted: ...and it says which specification step did not run"
expect_out "step 3 —" && ok

start "scope: every run states what it did not check, red as well as green"
M=$(plant owed2 "$OWED")
run_case "$M" M2
expect_out "NOT CHECKED HERE" && ok

start "planted: a milestone with no rows at all is NO_VERDICT, never green"
M=$(plant norows "$OTHER")
run_case "$M" M2
expect_rc 2 && expect_out "NO_VERDICT" && ok

start "fail closed: an unset REQ_MILESTONE clears everything, so it is refused"
M=$(plant unset "$SAT")
run_case "$M" ""
expect_rc 2 && expect_out "REQ_MILESTONE is unset" && ok

start "fail closed: a typo'd milestone is refused rather than matching no row"
run_case "$M" M99
expect_rc 2 && expect_out "is not a milestone" && ok

start "fail closed: an absent manifest is NO_VERDICT, not 'nothing owed'"
run_case "$TMPROOT/does-not-exist.tsv" M2
expect_rc 2 && expect_out "cannot read" && ok

start "manifest: a malformed row is reported, and it is not a milestone verdict"
M=$(plant malformed 'X-01|M2|N5|short row|fixture|tests/x.pd|owed|1.0')
run_case "$M" M2
expect_rc 1 && expect_out "tab-separated columns" && ok

start "manifest: an unknown status cannot enter by being typed"
M=$(plant status 'X-01|M2|N5|r|fixture|tests/x.pd|nearly|1.0|-')
run_case "$M" M2
expect_rc 1 && expect_out "has status 'nearly'" && ok

start "manifest: a duplicate id is reported with both lines"
M=$(plant dup "$OWED" "$SAT")
run_case "$M" M2
expect_rc 1 && expect_out "duplicate id X-01" && ok

# ---------------------------------------------------------------------------
# HALF TWO — the target. Everything above would still pass if `make m2-exit`
# stopped calling any of this, or read one inventory instead of four.
# ---------------------------------------------------------------------------
dry() { OUT=$(cd "$REPO" && make -n "$1" 2>&1); RC=$?; }

start "target: make m2-exit exists at all (it did not, and M2's Exit line named it)"
dry m2-exit
expect_rc 0 && ok

start "target: m2-exit reads inventory one — .pd fixtures, filtered to M2"
expect_out "CONFORMANCE_FORBID_OWNER=M2" && ok

start "target: m2-exit reads inventory two — the Rust debt manifest, filtered to M2"
expect_out "TEST_XFAIL_FORBID_OWNER=M2" && ok

start "target: m2-exit reads inventory three — the ordinary Rust suite"
expect_out "test --release --no-fail-fast" && ok

start "target: m2-exit reads inventory four — the requirement manifest, filtered to M2"
expect_out "REQ_MILESTONE=M2 python3 scripts/requirements.py" && ok

start "target: m2-exit reads the REAL manifest (no REQUIREMENTS_MANIFEST redirect)"
reject_out "REQUIREMENTS_MANIFEST" && ok

start "target: m1-exit is untouched and still filtered to M1"
dry m1-exit
expect_rc 0 && expect_out "CONFORMANCE_FORBID_OWNER=M1" && ok

# m1-exit deliberately does NOT get inventory four: the requirement manifest has
# no M1 rows, so the gate would abstain and turn a legitimately green target RED
# for a reason that says nothing about M1. Pinned so that "add it there too"
# is a decision somebody makes on purpose.
start "target: m1-exit does NOT read inventory four, and that is deliberate"
reject_out "REQ_MILESTONE" && ok

start "target: the self-test itself is on the certifying path (make gates)"
dry gates
expect_rc 0 && expect_out "test-requirements-runner" && ok

start "target: m2-exit is NOT in make gates — it is RED by design"
reject_out "m2-exit" && ok

# ---------------------------------------------------------------------------
# THE STATE OF THE TREE. Everything above runs on planted input; this is the
# real manifest, and M2 being RED here is the milestone's actual position.
# ---------------------------------------------------------------------------
start "the real manifest: M2 is RED today, which is the correct state"
OUT=$(cd "$REPO" && REQ_MILESTONE=M2 python3 scripts/requirements.py 2>&1); RC=$?
expect_rc 1 && expect_out "OWED_TO_M2" && ok

start "the real manifest: it parses clean — no OWED line is a manifest error"
reject_out "the manifest itself is malformed" && ok

echo
echo "=============================================="
echo "requirements runner regression: $((tests_run - tests_failed))/$tests_run passed"
echo "=============================================="
[ "$tests_failed" -eq 0 ]
