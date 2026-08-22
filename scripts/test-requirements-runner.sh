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
#   2. THE TARGET, BY ITS EFFECTS. `make m2-exit` must still READ all four
#      inventories, and must publish a verdict the Make layer cannot destroy.
#      Deleting one line of that recipe is invisible to every other gate in the
#      repo — the deleted inventory simply stops being consulted and everything
#      stays green.
#
#      THIS HALF USED TO BE `make -n | grep <command text>`, WHICH WAS THE `@true`
#      RUNG THIS REPOSITORY HAS ALREADY CLIMBED ONCE. A recipe of
#        @echo 'REQ_MILESTONE=M2 python3 scripts/requirements.py'
#      satisfied every assertion in it while reading no inventory at all. So the
#      target is now RUN, and every inventory has to have PRODUCED something —
#      with each number recomputed here, independently, from the same source the
#      inventory reads. An echo cannot make `verified=` appear beside
#      `failures=0`, cannot make cargo report 500+ passing tests, and cannot make
#      the requirement reader's row count equal that of a file it never opened.
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
#
# `--manifest` is an ARGUMENT and not an environment variable, deliberately: with
# it in the environment, `REQUIREMENTS_MANIFEST=/dev/null make m2-exit` redirected
# the milestone's own exit criterion at a file of the caller's choosing, and no
# assertion about the Makefile could see that.
run_case() {
  local m=$1 owner=$2
  if [ -n "$m" ]; then
    OUT=$( cd "$REPO" && REQ_MILESTONE="$owner" \
           python3 scripts/requirements.py --manifest "$m" 2>&1 )
  else
    OUT=$( cd "$REPO" && REQ_MILESTONE="$owner" python3 scripts/requirements.py 2>&1 )
  fi
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
# HALF TWO — THE TARGET, OBSERVED BY ITS EFFECTS.
#
# This half used to be `make -n m2-exit | grep <command text>`, and that was the
# `@true` rung this repository has already climbed once: a recipe of
#   @echo 'REQ_MILESTONE=M2 python3 scripts/requirements.py'
# satisfied every assertion in it while reading no inventory at all. A check
# satisfied by the SHAPE of a command is not a check on the command.
#
# So `make m2-exit` is RUN, once, and each inventory must be shown to have
# PRODUCED SOMETHING — and where the something is a number, this file recomputes
# that number independently from the same source and requires the two to agree.
# An `echo` cannot make `verified=48` appear beside a `failures=0`, cannot make
# `cargo` report 500-plus passing tests, and cannot make the requirement reader's
# row count equal the row count of a file it never opened.
#
# COST: this runs the ordinary Rust suite and the conformance corpus, so it is
# the expensive end of `make gates`. Measured below, and the alternative was a
# gate that could not fail.
# ---------------------------------------------------------------------------
dry() { OUT=$(cd "$REPO" && make -n "$1" 2>&1); RC=$?; }

start "target: make m2-exit exists at all (it did not, and M2's Exit line named it)"
dry m2-exit
expect_rc 0 && ok

# TWO heavy runs and not three, because `make m2-exit` runs the whole Rust suite
# and the whole conformance corpus. stdout is captured SEPARATELY from stderr so
# that the last-line contract can be checked on the stream it is defined over —
# a merged stream ends with Make's own `*** [m2-exit] Error 2`, which is exactly
# the mis-read scripts/thesis-exit.sh warns consumers about.
echo "  .. running \`make m2-exit\` for real (this is the expensive part, twice) .."
M2OUT=$(cd "$REPO" && make m2-exit 2>"$TMPROOT/m2.err"); M2RC=$?
M2ERR=$(cat "$TMPROOT/m2.err")
OUT=$M2OUT; RC=$M2RC

# Independent recomputation from the sources the target is supposed to read.
REQ_TSV=$REPO/docs/contributing/1.0-requirements.tsv
want_rows=$(awk -F'\t' 'NF==9 && $1 !~ /^#/' "$REQ_TSV" | wc -l | tr -d ' ')
want_m2=$(awk -F'\t' 'NF==9 && $2=="M2"' "$REQ_TSV" | wc -l | tr -d ' ')
want_owed=$(awk -F'\t' 'NF==9 && $2=="M2" && $7!="satisfied"' "$REQ_TSV" | wc -l | tr -d ' ')

# --- MF1: the tri-state has to survive to the caller -----------------------
LAST=$(printf '%s\n' "$M2OUT" | tail -1)

start "lattice: the aggregation itself is exercised where a real run cannot reach it"
# On this tree no inventory returns NO_VERDICT, so OWED-vs-NO_VERDICT precedence
# is dead code during a real run — measured: inverting the fold changed nothing
# about `make m2-exit`. The rule needs its own driver, and this is it.
OUT=$(cd "$REPO" && bash scripts/m2-exit.sh --self-test 2>&1); RC=$?
expect_rc 0 && expect_out "81 four-inventory combinations" && ok

start "verdict: make m2-exit publishes a machine-readable verdict on its LAST stdout line"
case "$LAST" in M2_EXIT_RESULT\ *) ok ;; *) bad "last stdout line was '$LAST'" ;; esac

start "verdict: it is OWED (1), not NO_VERDICT — the truth is a MEASUREMENT"
# THE DEFECT THIS REPLACES, measured on the first version of this target:
#   REQ_MILESTONE=M2 python3 scripts/requirements.py  -> 1  (OWED)
#   make m2-exit                                      -> 2  (NO_VERDICT)
# `|| rc=1` folded NO_VERDICT into OWED on the way in and Make folded every
# nonzero to 2 on the way out, so the target announced "nothing may be inferred"
# about a milestone it had just measured 43 outstanding rows of.
case "$LAST" in "M2_EXIT_RESULT 1 OWED") ok ;; *) bad "verdict line was '$LAST'" ;; esac

start "verdict: the line is on STDOUT, never on stderr"
case "$M2ERR" in *M2_EXIT_RESULT*) OUT=$M2ERR; bad "the verdict line leaked to stderr" ;; *) ok ;; esac

start "verdict: the script's own exit code carries the same three-valued state"
# This is the number Make erases, so it has to be read from the script itself.
(cd "$REPO" && bash scripts/m2-exit.sh >/dev/null 2>&1); SRC=$?
OUT="scripts/m2-exit.sh exited $SRC"
if [ "$SRC" -eq 1 ]; then ok; else bad "expected 1 (OWED), got $SRC"; fi

start "verdict: make itself is nonzero, so no zero-or-not consumer is made worse"
OUT="make m2-exit exited $M2RC"
if [ "$M2RC" -ne 0 ]; then ok; else bad "make m2-exit exited 0"; fi

start "verdict: and make's own code is 2 — which is why the LINE is the contract"
# Not a defect: Make maps every nonzero recipe status to 2. Pinned so that the
# reason the machine contract is a printed line, and not `$?`, stays visible.
if [ "$M2RC" -eq 2 ]; then ok; else bad "expected make's usual 2, got $M2RC"; fi

# --- MF4: each inventory must have actually produced its own evidence ------
start "effect: inventory one RAN — the conformance runner's own summary is present"
OUT=$M2OUT
expect_out "inventory one of four" && expect_out "verified=" && expect_out "failures=0" && ok

start "effect: inventory two RAN — the xfail runner counted cargo's ignored listing"
OUT=$M2OUT
expect_out "cargo lists " && expect_out " ignored test(s); " && ok

# Counting is done over a FILE rather than through a pipe: `printf … | grep -q`
# kills printf with SIGPIPE the moment grep matches, and under `set -o pipefail`
# a successful match therefore reports failure. That cost a debugging round here
# and would have been a permanently-red gate.
M2FILE=$TMPROOT/m2out.txt
printf '%s\n' "$M2OUT" | sed 's/\x1b\[[0-9;]*m//g' > "$M2FILE"

start "effect: inventory two's OWED count matches the lines it printed"
n_decl=$(sed -n 's/.*TEST_XFAIL_FORBID_OWNER=M2 -> \([0-9]*\) of .*/\1/p' "$M2FILE" | head -1)
n_lines=$(grep -c "\[OWED_TO_M2\] class=xfail" "$M2FILE")
if [ -n "$n_decl" ] && [ "$n_decl" = "$n_lines" ]; then ok
else bad "declared=$n_decl but $n_lines line(s) printed"; fi

start "effect: inventory three RAN — the ordinary Rust suite really executed"
n_pass=$(sed -n 's/^test result: ok\. \([0-9]*\) passed.*/\1/p' "$M2FILE" | awk '{s+=$1} END {print s+0}')
if [ "$n_pass" -ge 500 ]; then ok
else bad "expected 500+ passing tests in the output, got $n_pass"; fi

start "effect: inventory four RAN — and its row count equals this file's own count of the manifest"
if grep -qF "requirement inventory (docs/contributing/1.0-requirements.tsv): $want_rows row(s)" "$M2FILE"; then ok
else bad "no line reporting $want_rows rows — the reported count is not the manifest's"; fi

start "effect: inventory four's owed/owned counts are the manifest's too"
if grep -qF "REQ_MILESTONE=M2 -> $want_owed of $want_m2 row(s)" "$M2FILE"; then ok
else bad "no line reporting '$want_owed of $want_m2 row(s)'"; fi

start "effect: inventory four printed one OWED_TO_M2 line per outstanding row"
n_rows=$(grep -c "^  OWED_TO_M2 " "$M2FILE")
if [ "$n_rows" = "$want_owed" ]; then ok
else bad "printed $n_rows OWED_TO_M2 row line(s), the manifest has $want_owed"; fi

start "effect: inventory four checked the pinned ownership roster on the real manifest"
OUT=$M2OUT
expect_out "ownership roster: " && expect_out "id(s) pinned in scripts/requirements.py, 0 drift(s)" && ok

start "effect: all four inventories ran even though earlier ones were red"
OUT=$M2OUT
expect_out "inventory one of four" && expect_out "inventory two of four" \
  && expect_out "inventory three of four" && expect_out "inventory four of four" && ok

# --- the shape assertions that remain, and what they are for ---------------
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

start "target: the manifest path is an ARGUMENT, so no exported variable can redirect it"
# The hole: with the path read from the environment, `REQUIREMENTS_MANIFEST=…
# make m2-exit` pointed the milestone's exit criterion at a file of the caller's
# choosing. Proved by trying it — the target must ignore the variable entirely,
# which it does by there no longer being one to read.
OUT=$(cd "$REPO" && REQUIREMENTS_MANIFEST=/dev/null REQ_MILESTONE=M2 \
      python3 scripts/requirements.py 2>&1); RC=$?
expect_rc 1 && expect_out "requirement inventory (docs/contributing/1.0-requirements.tsv)" && ok

start "ledger: MILESTONES.md's counts over the manifest are derived and agree"
OUT=$(cd "$REPO" && python3 scripts/requirements.py --check-ledger 2>&1); RC=$?
expect_rc 0 && expect_out "REQUIREMENTS_RESULT 0 CLEAR" && ok

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
