#!/usr/bin/env bash
# Regression tests for scripts/conformance.sh.
#
# The conformance runner is a gate, and a gate is only worth its exit code. Every
# defect it has had was of one shape: it could be made to LOOK AT LESS and still
# exit 0. A green run over the real corpus does not exercise any of that — it only
# proves the happy path. So each case below builds a throwaway repo, breaks
# exactly one thing, and asserts the runner goes RED (and green again when the
# break is removed).
#
# Each temp repo gets: scripts/conformance.sh (the real one), symlinks to the
# real target/release/pdc and runtime/ (pdc links the runtime by relative path),
# a build_output/, and whatever fixtures the case needs.
#
# Usage: bash scripts/test-conformance-runner.sh

set -uo pipefail
cd "$(dirname "$0")/.." || exit 2
REPO=$PWD

if [ ! -x "$REPO/target/release/pdc" ]; then
  echo "error: target/release/pdc not built. Run: cargo build --release" >&2
  exit 2
fi

TMPROOT=$(mktemp -d) || exit 2
trap 'rm -rf "$TMPROOT"' EXIT

tests_run=0; tests_failed=0
CASE=""

# --- harness ---------------------------------------------------------------

# new_repo <name> -> echoes the path of a fresh throwaway repo
new_repo() {
  local d=$TMPROOT/$1
  mkdir -p "$d/scripts" "$d/target/release" "$d/build_output" "$d/tests"
  cp "$REPO/scripts/conformance.sh" "$d/scripts/conformance.sh"
  ln -sf "$REPO/target/release/pdc" "$d/target/release/pdc"
  ln -sf "$REPO/runtime" "$d/runtime"
  printf '%s' "$d"
}

# fixture <repo> <relpath> <body> [expected-stdout]
# Also writes the sibling .expected transcript, because class=run now requires
# one. Defaults to what $good_program prints. Harmless for fixtures declared with
# another class: the golden is simply never consulted.
fixture() {
  mkdir -p "$(dirname "$1/$2")"
  printf '%s\n' "$3" > "$1/$2"
  printf '%s\n' "${4-ok}" > "$1/${2%.pd}.expected"
}

good_program='fn main() {
    print("ok");
}'
bad_program='fn main() {
    print("ok");
}
EOF < /dev/null'
runtime_fail_program='fn main() -> i64 {
    print("boom");
    return 3;
}'
vacuous_program='//@ vacuous: prints that a feature is unimplemented
fn main() {
    print("not yet implemented");
}'
library_module='pub fn helper(a: i64) -> i64 {
    return a + 1;
}'

# manifest <repo> <lines...>   (each line uses | which is translated to TAB)
manifest() {
  local d=$1; shift
  : > "$d/tests/conformance-manifest.txt"
  local l
  for l in "$@"; do
    printf '%s\n' "$l" | tr '|' '\t' >> "$d/tests/conformance-manifest.txt"
  done
}

# run_case <repo> [scope...] -> sets RC and OUT.
# With no scope given, use `tests` explicitly: these throwaway repos have no
# examples/ root, and the point of each case is one specific behaviour, not the
# default scope list.
run_case() {
  local d=$1; shift
  if [ "$#" -eq 0 ]; then set -- tests; fi
  OUT=$( cd "$d" && bash scripts/conformance.sh "$@" 2>&1 )
  RC=$?
}

start() { CASE=$1; tests_run=$((tests_run+1)); }

ok()   { printf '  \033[0;32mPASS\033[0m %s\n' "$CASE"; }
bad()  {
  printf '  \033[0;31mFAIL\033[0m %s\n         %s\n' "$CASE" "$1"
  printf '%s\n' "$OUT" | sed 's/^/         | /' | head -25
  tests_failed=$((tests_failed+1))
}

expect_rc() {
  if [ "$RC" -eq "$1" ]; then return 0; fi
  bad "expected exit $1, got $RC"; return 1
}
expect_out() {
  case "$OUT" in *"$1"*) return 0 ;; esac
  bad "expected output to contain '$1'"; return 1
}

echo "conformance runner regression tests"
echo

# ---------------------------------------------------------------------------
# Baseline: a well-formed repo is green. Everything below is this, broken once.
# ---------------------------------------------------------------------------
start "baseline: fully declared corpus is green"
D=$(new_repo baseline)
fixture "$D" tests/a.pd "$good_program"
fixture "$D" tests/b.pd "$good_program"
manifest "$D" 'tests/a.pd|run|-|expected|-|-' 'tests/b.pd|run|-|expected|-|-'
run_case "$D"
expect_rc 0 && expect_out "verified=2" && ok

# ---------------------------------------------------------------------------
# ITEM 4 — closed inventory. A fixture that vanishes or appears unnoticed is
# the same disease as the .gitignore traps this repo has been bitten by twice.
# ---------------------------------------------------------------------------
start "item4: an UNDECLARED fixture fails the gate"
D=$(new_repo undeclared)
fixture "$D" tests/a.pd "$good_program"
fixture "$D" tests/sneaky.pd "$good_program"
manifest "$D" 'tests/a.pd|run|-|expected|-|-'
run_case "$D"
expect_rc 1 && expect_out "UNDECLARED" && ok

start "item4: a declared fixture deleted from disk fails the gate (was: silent shrink)"
D=$(new_repo missing)
fixture "$D" tests/a.pd "$good_program"
fixture "$D" tests/b.pd "$good_program"
manifest "$D" 'tests/a.pd|run|-|expected|-|-' 'tests/b.pd|run|-|expected|-|-'
rm "$D/tests/b.pd"
run_case "$D"
expect_rc 1 && expect_out "MISSING" && ok

start "item4: retiring a fixture requires deleting BOTH file and row (a tracked diff)"
manifest "$D" 'tests/a.pd|run|-|expected|-|-'
run_case "$D"
# Now consistent again: this is the intended way to retire a fixture, and it is
# visible in the diff of a tracked file rather than invisible in a scan.
expect_rc 0 && expect_out "fixtures=1" && ok

start "item4: a missing manifest is fatal, not an empty pass"
D=$(new_repo nomanifest)
fixture "$D" tests/a.pd "$good_program"
run_case "$D"
expect_rc 2 && expect_out "closed" && ok

# ---------------------------------------------------------------------------
# ITEM 2 — enumeration status must not be swallowed.
# ---------------------------------------------------------------------------
start "item2: a nonexistent scope is fatal (was: exit 0 with total=0)"
D=$(new_repo badscope)
fixture "$D" tests/a.pd "$good_program"
manifest "$D" 'tests/a.pd|run|-|expected|-|-'
run_case "$D" tests/does_not_exist
expect_rc 2 && expect_out "is not a directory" && ok

start "item2: an unreadable scope is fatal"
D=$(new_repo unreadable)
fixture "$D" tests/a.pd "$good_program"
mkdir -p "$D/tests/locked"
manifest "$D" 'tests/a.pd|run|-|expected|-|-'
chmod 000 "$D/tests/locked"
run_case "$D"
chmod 755 "$D/tests/locked"
expect_rc 2 && expect_out "enumeration failed" && ok

start "item2: a scope with no fixtures is fatal, not a pass"
D=$(new_repo emptyscope)
fixture "$D" tests/a.pd "$good_program"
mkdir -p "$D/empty"
manifest "$D" 'tests/a.pd|run|-|expected|-|-'
run_case "$D" empty
expect_rc 2 && expect_out "no .pd fixtures" && ok

# ---------------------------------------------------------------------------
# ITEM 3 — path spelling must not change the verdict.
# ---------------------------------------------------------------------------
start "item3: './tests' spelling still matches manifest entries (was: xfail=0)"
D=$(new_repo canon)
fixture "$D" tests/a.pd "$good_program"
fixture "$D" tests/vac.pd "$vacuous_program"
manifest "$D" 'tests/a.pd|run|-|expected|-|-' 'tests/vac.pd|vacuous|-|-|M4|claims: traits. Coverage is ZERO.'
run_case "$D" ./tests
expect_rc 0 && expect_out "vacuous=1" && ok

start "item3: 'tests//' spelling behaves identically"
run_case "$D" 'tests//'
expect_rc 0 && expect_out "vacuous=1" && ok

start "item3: 'tests/../tests' resolves (was: unresolved '..' matched nothing)"
run_case "$D" tests/../tests
expect_rc 0 && expect_out "vacuous=1" && ok

start "item3: an absolute path to the scope resolves"
OUT=$( cd "$D" && bash scripts/conformance.sh "$D/tests" 2>&1 ); RC=$?
expect_rc 0 && expect_out "vacuous=1" && ok

start "item3: a symlinked scope directory resolves to its target"
ln -sfn tests "$D/tests_link"
run_case "$D" tests_link
expect_rc 0 && expect_out "vacuous=1" && ok

start "item3: a scope outside the repository is refused, not silently empty"
OUT=$( cd "$D" && bash scripts/conformance.sh "$TMPROOT" 2>&1 ); RC=$?
expect_rc 2 && expect_out "outside the repository" && ok

start "item3: a symlinked FIXTURE stays a distinct declared fixture"
# tests/integration/test.pd in the real repo is a symlink; folding it into its
# target would silently shrink the corpus by one.
D=$(new_repo symlinkfixture)
fixture "$D" tests/real.pd "$good_program"
printf 'ok\n' > "$D/tests/real.expected"
ln -sfn real.pd "$D/tests/alias.pd"
printf 'ok\n' > "$D/tests/alias.expected"
manifest "$D" 'tests/real.pd|run|-|expected|-|-' 'tests/alias.pd|run|-|expected|-|-'
run_case "$D"
expect_rc 0 && expect_out "verified=2" && ok

# --- the repository root as a scope ----------------------------------------
# `.` resolves to the repo root, which contains every repo-relative path. Scope
# membership did not know that, so declared_in_scope was 0 and MISSING never
# fired: the closed inventory failing open in the invocation a person is most
# likely to reach for.
start "root scope: '.' contains repo-relative paths (was: declared_in_scope=0)"
D=$(new_repo rootscope)
fixture "$D" tests/a.pd "$good_program"
fixture "$D" tests/b.pd "$good_program"
manifest "$D" 'tests/a.pd|run|-|expected|-|-' 'tests/b.pd|run|-|expected|-|-'
run_case "$D" .
expect_rc 0 && expect_out "declared_in_scope=2" && ok

start "root scope: a deleted fixture is caught under '.' (was: escaped MISSING)"
rm "$D/tests/b.pd"
run_case "$D" .
expect_rc 1 && expect_out "MISSING" && ok

start "root scope: the absolute repository root behaves identically"
OUT=$( cd "$D" && bash scripts/conformance.sh "$D" 2>&1 ); RC=$?
expect_rc 1 && expect_out "MISSING" && ok

# --- overlapping scopes ------------------------------------------------------
# `find tests ./tests` visited every fixture twice and still exited 0, so the
# coverage number could be doubled by repeating an argument.
start "overlap: the same scope twice is refused (was: everything counted twice)"
D=$(new_repo overlap)
fixture "$D" tests/a.pd "$good_program"
mkdir -p "$D/tests/inner"
fixture "$D" tests/inner/b.pd "$good_program"
manifest "$D" 'tests/a.pd|run|-|expected|-|-' 'tests/inner/b.pd|run|-|expected|-|-'
run_case "$D" tests ./tests
expect_rc 2 && expect_out "given more than once" && ok

start "overlap: a nested scope pair is refused"
run_case "$D" tests tests/inner
expect_rc 2 && expect_out "overlap" && ok

start "overlap: '.' plus any other scope is refused"
run_case "$D" . tests
expect_rc 2 && expect_out "overlap" && ok

start "overlap: the non-overlapping baseline still counts each fixture once"
run_case "$D" tests
expect_rc 0 && expect_out "fixtures=2" && ok

# --- transport ---------------------------------------------------------------
start "transport: a scope path containing a newline is refused"
run_case "$D" "$(printf 'tests\ntests')"
expect_rc 2 && expect_out "newline" && ok

start "transport: a dangling symlink is enumerated, not mistaken for a split path"
D=$(new_repo dangling)
fixture "$D" tests/a.pd "$good_program"
ln -sfn nonexistent.pd "$D/tests/broken_link.pd"
manifest "$D" 'tests/a.pd|run|-|expected|-|-'
run_case "$D" tests
expect_rc 1 && expect_out "UNDECLARED" && ok

# ===========================================================================
# CLASS REGRESSIONS. Four rounds of review found the same species of bug five
# times: bash string/pattern handling silently producing the PERMISSIVE answer.
# These cases are organised by class rather than by instance, so a new site that
# reintroduces the class is caught even though its symptom is new.
# ===========================================================================

# --- CLASS 1: a variable used as a PATTERN where it must be a literal --------
start "class/pattern: an empty scope named with a regex metacharacter cannot hide"
# `grep -q "^$d/"` read the scope name as a regex, so empty `fooba.` matched
# populated `foobar/...` and evaded the fatal empty-scope check.
D=$(new_repo clspattern)
mkdir -p "$D/foobar" "$D/fooba."
fixture "$D" foobar/a.pd "$good_program"
manifest "$D" 'foobar/a.pd|run|-|expected|-|-'
run_case "$D" foobar 'fooba.'
expect_rc 2 && expect_out "no .pd fixtures under scope 'fooba.'" && ok

start "class/pattern: a scope whose literal name holds a metacharacter still works"
mkdir -p "$D/a+b"
fixture "$D" 'a+b/x.pd' "$good_program"
manifest "$D" 'foobar/a.pd|run|-|expected|-|-' 'a+b/x.pd|run|-|expected|-|-'
run_case "$D" 'a+b'
expect_rc 0 && expect_out "verified=1" && ok

start "class/pattern: a manifest path holding a metacharacter matches only itself"
run_case "$D" foobar 'a+b'
expect_rc 0 && expect_out "verified=2" && ok

# --- CLASS 2: an exit status with a THIRD meaning treated as yes/no ----------
start "class/status: an unreadable fixture is a harness failure, not a 'skip'"
# grep rc 2 collapsed into has_main=0, so a file the gate could not read passed
# as a declared non-program.
D=$(new_repo clsstatus)
fixture "$D" tests/a.pd "$good_program"
fixture "$D" tests/locked.pd "$good_program"
chmod 000 "$D/tests/locked.pd"
manifest "$D" 'tests/a.pd|run|-|expected|-|-' 'tests/locked.pd|skip|-|-|-|claims to be a non-program'
run_case "$D" tests
chmod 644 "$D/tests/locked.pd"
expect_rc 1 && expect_out "UNREADABLE" && ok

start "class/status: a declared dangling symlink is a harness failure, not a 'skip'"
D=$(new_repo clsdangle2)
fixture "$D" tests/a.pd "$good_program"
ln -sfn nowhere.pd "$D/tests/dangle.pd"
manifest "$D" 'tests/a.pd|run|-|expected|-|-' 'tests/dangle.pd|skip|-|-|-|declared non-program'
run_case "$D" tests
expect_rc 1 && expect_out "UNREADABLE" && ok

start "class/status: an unreadable transcript is a harness failure, not a mismatch"
D=$(new_repo clsgolden)
fixture "$D" tests/a.pd "$good_program"
chmod 000 "$D/tests/a.expected"
manifest "$D" 'tests/a.pd|run|-|expected|-|-'
run_case "$D" tests
chmod 644 "$D/tests/a.expected"
expect_rc 1 && expect_out "HARNESS_ERROR" && ok

# --- CLASS 3: data crossing a newline/whitespace-delimited boundary ---------
start "class/delimiter: a newline in a fixture name is refused, not split"
# find|read split `a.pd<LF>b.pd` into two paths; both happened to exist, so the
# real fixture vanished and another was counted twice (verified=4 over 3 files).
D=$(new_repo clsdelim)
fixture "$D" tests/a.pd "$good_program"
printf 'fn main() {\n    print("ok");\n}\n' > "$D/tests/split.pd"$'\nb.pd'
manifest "$D" 'tests/a.pd|run|-|expected|-|-'
run_case "$D" tests
expect_rc 2 && expect_out "contains a newline" && ok

start "class/delimiter: a SPACE in a fixture name survives enumeration intact"
D=$(new_repo clsspace)
fixture "$D" 'tests/with space.pd' "$good_program"
manifest "$D" 'tests/with space.pd|run|-|expected|-|-'
run_case "$D" tests
expect_rc 0 && expect_out "verified=1" && ok

start "class/delimiter: a TAB in a fixture name is refused (manifest is TSV)"
D=$(new_repo clstab)
fixture "$D" tests/a.pd "$good_program"
printf 'fn main() {\n    print("ok");\n}\n' > "$D/tests/tabbed"$'\t'"x.pd"
manifest "$D" 'tests/a.pd|run|-|expected|-|-'
run_case "$D" tests
expect_rc 1 && expect_out "UNDECLARED" && ok

# --- COMBINED scope cases (each component was tested; the combinations were not)
start "combined: a scope plus a symlink pointing at it is an overlap"
D=$(new_repo combosym)
fixture "$D" tests/a.pd "$good_program"
ln -sfn tests "$D/tests_alias"
manifest "$D" 'tests/a.pd|run|-|expected|-|-'
run_case "$D" tests tests_alias
expect_rc 2 && expect_out "given more than once" && ok

start "combined: absolute 'tests' plus '.' is an overlap"
OUT=$( cd "$D" && bash scripts/conformance.sh "$D/tests" . 2>&1 ); RC=$?
expect_rc 2 && expect_out "overlap" && ok

start "combined: a nonexistent scope beside a valid one aborts before find"
run_case "$D" tests tests/nope
expect_rc 2 && expect_out "is not a directory" && ok

start "item3: an XPASS is still caught through an alternate spelling"
D=$(new_repo canonxpass)
fixture "$D" tests/a.pd "$good_program"
fixture "$D" tests/vac.pd "$vacuous_program"
manifest "$D" 'tests/a.pd|xfail|compile|whatever|M1|declared failing' 'tests/vac.pd|vacuous|-|-|M4|claims: traits. Coverage is ZERO.'
run_case "$D" ./tests
expect_rc 1 && expect_out "XPASS" && ok

# ---------------------------------------------------------------------------
# ITEM 1 — an xfail must be pinned to a stage AND a diagnostic.
# ---------------------------------------------------------------------------
start "item1: xfail matching its declared stage+fingerprint is XFAIL (green)"
D=$(new_repo fingerprint)
fixture "$D" tests/broken.pd "$bad_program"
manifest "$D" 'tests/broken.pd|xfail|compile|Expected function, struct, enum|M1|known parse failure'
run_case "$D"
expect_rc 0 && expect_out "xfail=1" && ok

start "item1: same file failing with a DIFFERENT diagnostic fails the gate"
manifest "$D" 'tests/broken.pd|xfail|compile|Unsupported type in reference parameter|M1|stale excuse from another defect'
run_case "$D"
expect_rc 1 && expect_out "XFAIL_MISMATCH" && ok

start "item1: a compile-stage expectation is not satisfied by a runtime failure"
D=$(new_repo stagepin)
fixture "$D" tests/rt.pd "$runtime_fail_program"
manifest "$D" 'tests/rt.pd|xfail|compile|anything|M1|wrong stage'
run_case "$D"
expect_rc 1 && expect_out "XFAIL_MISMATCH" && ok

start "item1: pinning the right stage and exit code makes it XFAIL"
manifest "$D" 'tests/rt.pd|xfail|run|exit=3|M1|program returns 3 by design'
run_case "$D"
expect_rc 0 && expect_out "xfail=1" && ok

start "item1: a wrong exit code fails the gate"
manifest "$D" 'tests/rt.pd|xfail|run|exit=1|M1|wrong code'
run_case "$D"
expect_rc 1 && expect_out "XFAIL_MISMATCH" && ok

# ---------------------------------------------------------------------------
# ITEM 5 — a stale binary must never satisfy the executable check.
# ---------------------------------------------------------------------------
start "item5: a stale build_output binary cannot turn a compile failure into PASS"
D=$(new_repo stalebin)
fixture "$D" tests/broken.pd "$bad_program"
manifest "$D" 'tests/broken.pd|run|-|expected|-|-'
# Plant an executable under every name the runner might plausibly use.
for nm in broken cf_1_tests_broken_pd; do
  printf '#!/bin/sh\nexit 0\n' > "$D/build_output/$nm"; chmod +x "$D/build_output/$nm"
done
run_case "$D"
expect_rc 1 && expect_out "COMPILE_FAIL" && ok

start "item5: two fixtures with the same basename do not share an output"
D=$(new_repo basename)
fixture "$D" tests/one/dup.pd "$good_program"
fixture "$D" tests/two/dup.pd "$runtime_fail_program"
manifest "$D" 'tests/one/dup.pd|run|-|expected|-|-' 'tests/two/dup.pd|run|-|expected|-|-'
run_case "$D"
# The second must be reported RUN_FAIL; under basename-only outputs it could be
# satisfied by the first fixture's binary.
expect_rc 1 && expect_out "RUN_FAIL" && ok

# ---------------------------------------------------------------------------
# ITEM 6 — vacuousness is declared, not inferred from a missing string.
# ---------------------------------------------------------------------------
start "item6 setup: a correctly-marked vacuous fixture is green"
D=$(new_repo vacuous)
fixture "$D" tests/vac.pd "$vacuous_program"
manifest "$D" 'tests/vac.pd|vacuous|-|-|M4|claims: traits. Coverage is ZERO.'
run_case "$D"
expect_rc 0 && expect_out "vacuous=1" && ok

start "item6: deleting a vacuous marker no longer silently upgrades it to a pass"
fixture "$D" tests/vac.pd "$good_program"      # marker removed
run_case "$D"
expect_rc 1 && expect_out "MARKER_MISSING" && ok

start "item6: a marker on a fixture declared class=run fails the gate"
D=$(new_repo markerundeclared)
fixture "$D" tests/vac.pd "$vacuous_program"
manifest "$D" 'tests/vac.pd|run|-|expected|-|-'
run_case "$D"
expect_rc 1 && expect_out "MARKER_UNDECLARED" && ok

start "item6: a marker below line 1 fails rather than being ignored"
D=$(new_repo markerplace)
fixture "$D" tests/vac.pd '// leading comment
//@ vacuous: too late to count
fn main() {
    print("ok");
}'
manifest "$D" 'tests/vac.pd|vacuous|-|-|M4|claims: traits. Coverage is ZERO.'
run_case "$D"
expect_rc 1 && expect_out "MARKER_MISPLACED" && ok

# ---------------------------------------------------------------------------
# Transcript verification. THE point: a wrong ANSWER with exit 0 is invisible to
# an exit-code verdict. Measured on this machine, at both -O0 and -O2:
#     long long add_tail(long long a, long long b) { (a + b); }
#   -> prints 8261746944, EXIT=0
# That is defect D3's signature, and it is why D3 survived a year.
# ---------------------------------------------------------------------------
answer_right='fn main() {
    print_int(8);
}'
answer_wrong='fn main() {
    print_int(8261746944);
}'

start "transcript: a matching transcript is PASS_VERIFIED"
D=$(new_repo golden)
fixture "$D" tests/answer.pd "$answer_right"
printf '8\n' > "$D/tests/answer.expected"
manifest "$D" 'tests/answer.pd|run|-|expected|-|-'
run_case "$D"
expect_rc 0 && expect_out "verified=1" && ok

start "transcript: a WRONG ANSWER that still exits 0 goes RED (D3 signature)"
fixture "$D" tests/answer.pd "$answer_wrong"
run_case "$D"
expect_rc 1 && expect_out "OUTPUT_MISMATCH" && ok

start "transcript: the exit-code-only opt-out is GONE (class=run demands one)"
# `run` with observable `-` used to mean "check the exit code and nothing else",
# a documented bypass of the very protection above. It is now a manifest error.
manifest "$D" 'tests/answer.pd|run|-|-|-|-'
run_case "$D"
expect_rc 2 && expect_out "must be 'expected'" && ok

start "untranscribed: the allowance still cannot see a wrong answer..."
# The hole is real, which is why it must be declared rather than defaulted into.
# Same wrong-answer fixture, exit 0, no transcript -> green.
manifest "$D" 'tests/answer.pd|untranscribed|-|-|M1|why: output is machine dependent'
run_case "$D"
expect_rc 0 && expect_out "untranscribed=1" && ok

start "untranscribed: ...so it is reported as a debt on every run"
expect_out "No transcript" && ok

start "untranscribed: it requires an owner"
manifest "$D" 'tests/answer.pd|untranscribed|-|-|-|why: no owner given'
run_case "$D"
expect_rc 2 && expect_out "needs an owner" && ok

start "untranscribed: it requires a 'why:' reason"
manifest "$D" 'tests/answer.pd|untranscribed|-|-|M1|just because'
run_case "$D"
expect_rc 2 && expect_out "must begin 'why:" && ok

start "untranscribed: CONFORMANCE_FORBID_OWNER can drive the count to zero"
manifest "$D" 'tests/answer.pd|untranscribed|-|-|M1|why: output is machine dependent'
OUT=$( cd "$D" && CONFORMANCE_FORBID_OWNER=M1 bash scripts/conformance.sh tests 2>&1 ); RC=$?
expect_rc 1 && expect_out "OWED_TO_M1" && ok

start "transcript: declaring 'expected' with no golden on disk is a manifest error"
D=$(new_repo nogolden)
fixture "$D" tests/answer.pd "$answer_right"
rm -f "$D/tests/answer.expected"
manifest "$D" 'tests/answer.pd|run|-|expected|-|-'
run_case "$D"
expect_rc 2 && expect_out "does not exist" && ok

start "transcript: bless mode never exits 0, so it cannot be used as a green gate"
D=$(new_repo bless)
fixture "$D" tests/answer.pd "$answer_right"
: > "$D/tests/answer.expected"
manifest "$D" 'tests/answer.pd|run|-|expected|-|-'
OUT=$( cd "$D" && CONFORMANCE_BLESS=1 bash scripts/conformance.sh tests 2>&1 ); RC=$?
expect_rc 2 && expect_out "BLESS MODE" && ok

start "transcript: the blessed file is then accepted by a normal run"
run_case "$D"
expect_rc 0 && expect_out "verified=1" && ok

# ---------------------------------------------------------------------------
# The xfail handoff protocol. Under a closed inventory, paying off an xfail is a
# TRANSITION, not a deletion — the fixture is still on disk, so deleting its row
# makes it UNDECLARED. This is the exact sequence the D9 branch must follow, so
# it is tested rather than merely written down.
# ---------------------------------------------------------------------------
start "handoff: an xfail whose defect is fixed reports XPASS"
D=$(new_repo handoff)
fixture "$D" tests/wasbroken.pd "$good_program"
manifest "$D" 'tests/wasbroken.pd|xfail|compile|Expected function, struct, enum|M1|defect pending'
run_case "$D"
expect_rc 1 && expect_out "XPASS" && ok

start "handoff: the XPASS text forbids deletion and names the replacement row"
expect_out "Do NOT delete the row" && ok

start "handoff: DELETING the row does not work - it becomes UNDECLARED"
manifest "$D" '# intentionally empty'
run_case "$D"
expect_rc 1 && expect_out "UNDECLARED" && ok

start "handoff: TRANSITIONING the row to run+transcript is what makes it green"
manifest "$D" 'tests/wasbroken.pd|run|-|expected|-|-'
run_case "$D"
expect_rc 0 && expect_out "verified=1" && ok

# The XPASS text tells the fixing branch to generate the transcript with bless.
# But bless cannot start from a genuinely absent file: declaring `expected` with
# no golden is a manifest error, so the run aborts before blessing. The text
# therefore has to name a bootstrap step, and that whole sequence is walked here
# from a truly missing transcript — the exact path the D9 branch will take.
start "handoff bootstrap: a transition with NO transcript file aborts"
D=$(new_repo bootstrap)
fixture "$D" tests/wasbroken.pd "$good_program"
rm -f "$D/tests/wasbroken.expected"
manifest "$D" 'tests/wasbroken.pd|run|-|expected|-|-'
run_case "$D"
expect_rc 2 && expect_out "does not exist" && ok

start "handoff bootstrap: ...and blessing cannot rescue it either"
OUT=$( cd "$D" && CONFORMANCE_BLESS=1 bash scripts/conformance.sh tests 2>&1 ); RC=$?
expect_rc 2 && expect_out "does not exist" && ok

start "handoff bootstrap: creating it empty then blessing populates it"
: > "$D/tests/wasbroken.expected"
OUT=$( cd "$D" && CONFORMANCE_BLESS=1 bash scripts/conformance.sh tests 2>&1 ); RC=$?
expect_rc 2 && expect_out "BLESS MODE" && ok

start "handoff bootstrap: the populated transcript then passes a normal run"
run_case "$D"
expect_rc 0 && expect_out "verified=1" && ok

start "handoff bootstrap: and the transcript holds the program's real output"
if [ "$(cat "$D/tests/wasbroken.expected")" = "ok" ]; then ok; else bad "transcript content wrong"; fi

# ---------------------------------------------------------------------------
# class=reject — same fingerprint machinery as xfail, opposite meaning. This is
# how "the compiler must refuse `.await` with a span-carrying diagnostic" gets
# tested, instead of a program that prints prose about async being unimplemented.
# ---------------------------------------------------------------------------
start "reject: a program the compiler correctly refuses counts as coverage"
D=$(new_repo reject)
fixture "$D" tests/refused.pd "$bad_program"
manifest "$D" 'tests/refused.pd|reject|compile|Expected function, struct, enum|-|the compiler must refuse this construct'
run_case "$D"
expect_rc 0 && expect_out "reject=1" && ok

start "reject: it is counted as coverage, NOT as a debt (xfail=0)"
expect_out "xfail=0" && ok

start "reject: a wrong diagnostic fails the gate (REJECT_MISMATCH)"
manifest "$D" 'tests/refused.pd|reject|compile|Unsupported type in reference parameter|-|wrong diagnostic'
run_case "$D"
expect_rc 1 && expect_out "REJECT_MISMATCH" && ok

start "reject: if the compiler ACCEPTS it, that fails the gate"
D=$(new_repo rejectaccept)
fixture "$D" tests/accepted.pd "$good_program"
manifest "$D" 'tests/accepted.pd|reject|compile|should have been refused|-|regression guard'
run_case "$D"
expect_rc 1 && expect_out "REJECT_ACCEPTED" && ok

start "reject: an owner is rejected — a negative test is owed to nobody"
manifest "$D" 'tests/accepted.pd|reject|compile|x|M1|has an owner'
run_case "$D"
expect_rc 2 && expect_out "must have owner" && ok

# ---------------------------------------------------------------------------
# vacuous notes must name the feature they fail to cover.
# ---------------------------------------------------------------------------
start "vacuous: a note that does not name the unproven feature is rejected"
D=$(new_repo claims)
fixture "$D" tests/vac.pd "$vacuous_program"
manifest "$D" 'tests/vac.pd|vacuous|-|-|M4|just a placeholder, names no feature'
run_case "$D"
expect_rc 2 && expect_out "must begin 'claims:" && ok

start "vacuous: a 'claims:' note is accepted and echoed in the summary"
manifest "$D" 'tests/vac.pd|vacuous|-|-|M4|claims: traits. Coverage is ZERO.'
run_case "$D"
expect_rc 0 && expect_out "Coverage of the named feature is ZERO" && ok

start "vacuous: owner 'unscheduled' is allowed for features with no milestone"
manifest "$D" 'tests/vac.pd|vacuous|-|-|unscheduled|claims: async/await. Coverage is ZERO.'
run_case "$D"
expect_rc 0 && expect_out "vacuous=1" && ok

# ---------------------------------------------------------------------------
# ITEM 7 — milestone ownership is a structured, enforceable field.
# ---------------------------------------------------------------------------
start "item7 setup: an owned xfail is green under a plain run"
D=$(new_repo owner)
fixture "$D" tests/broken.pd "$bad_program"
manifest "$D" 'tests/broken.pd|xfail|compile|Expected function, struct, enum|M1|owed to M1'
run_case "$D"
expect_rc 0 && expect_out "xfail=1" && ok

start "item7: CONFORMANCE_FORBID_OWNER turns a milestone exit into a command"
OUT=$( cd "$D" && CONFORMANCE_FORBID_OWNER=M1 bash scripts/conformance.sh tests 2>&1 ); RC=$?
expect_rc 1 && expect_out "OWED_TO_M1" && ok

start "item7: an unrelated milestone does not trip the same check"
OUT=$( cd "$D" && CONFORMANCE_FORBID_OWNER=M3 bash scripts/conformance.sh tests 2>&1 ); RC=$?
expect_rc 0 && ok

# ---------------------------------------------------------------------------
# Class / manifest well-formedness
# ---------------------------------------------------------------------------
start "manifest: a duplicate entry is rejected, naming both lines"
D=$(new_repo dup)
fixture "$D" tests/a.pd "$good_program"
manifest "$D" 'tests/a.pd|run|-|expected|-|-' 'tests/a.pd|run|-|-|-|second declaration'
run_case "$D"
expect_rc 2 && expect_out "duplicate entry" && ok

start "manifest: a malformed row (wrong column count) is rejected"
D=$(new_repo malformed)
fixture "$D" tests/a.pd "$good_program"
manifest "$D" 'tests/a.pd|run|-'
run_case "$D"
expect_rc 2 && expect_out "6 tab-separated" && ok

start "manifest: an xfail without a fingerprint is rejected"
manifest "$D" 'tests/a.pd|xfail|compile|-|M1|no fingerprint'
run_case "$D"
expect_rc 2 && expect_out "needs a diagnostic fingerprint" && ok

start "manifest: an unknown class is rejected"
manifest "$D" 'tests/a.pd|probably|-|-|-|-'
run_case "$D"
expect_rc 2 && expect_out "unknown class" && ok

start "manifest: class=skip on a file that has fn main is rejected"
manifest "$D" 'tests/a.pd|skip|-|-|-|claims not to be a program'
run_case "$D"
expect_rc 1 && expect_out "CLASS_MISMATCH" && ok

start "manifest: class=run on a library module with no fn main is rejected"
D=$(new_repo libclass)
fixture "$D" tests/lib.pd "$library_module"
manifest "$D" 'tests/lib.pd|run|-|expected|-|-'
run_case "$D"
expect_rc 1 && expect_out "CLASS_MISMATCH" && ok

start "manifest: declaring it skip is the correct, explicit resolution"
manifest "$D" 'tests/lib.pd|skip|-|-|-|library module, no fn main by design'
run_case "$D"
expect_rc 0 && expect_out "skip=1" && ok

start "manifest: a path containing spaces round-trips (tab-delimited format)"
D=$(new_repo spaces)
fixture "$D" "tests/with space.pd" "$good_program"
manifest "$D" 'tests/with space.pd|run|-|expected|-|-'
run_case "$D"
expect_rc 0 && expect_out "verified=1" && ok

# ---------------------------------------------------------------------------
echo
echo "=============================================="
echo "runner regression: $((tests_run - tests_failed))/$tests_run passed"
echo "=============================================="
[ "$tests_failed" -eq 0 ]
