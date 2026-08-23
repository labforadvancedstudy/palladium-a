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

# A WORKING C COMPILER IS A PRECONDITION, not an outcome to be classified.
# Several stubs below run a real gcc and derive an exit code from its status, and
# a missing or unexecutable compiler exits 126/127 — which this branch's contract
# calls a TOOLCHAIN outcome (code 5), not a backend rejection. The stubs map it
# correctly, so without this preflight an absent gcc would surface as controls
# failing to observe BACKEND_REJECT: a true statement about the machine, read as
# a false statement about the classifier. Name the precondition instead.
if ! printf 'int main(void){return 0;}\n' | gcc -x c -o /dev/null - 2>/dev/null; then
  echo "error: no working C compiler on PATH. The fault-injection stubs below run" >&2
  echo "       a real gcc and derive a structured exit code from its status; without" >&2
  echo "       one this suite cannot establish anything and must not report green." >&2
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

# stub_pdc <repo> <body> — replace the repo's pdc with a shell stub.
#
# A CONTROL THAT DIES WITH THE BUG IS NOT A CONTROL. The never-expectable
# post-codegen verdicts can only be observed on a program the front end accepts
# and the backend then fails to build, and the only such programs on a healthy
# tree are live compiler defects — so a control that borrows one evaporates the
# day it is fixed, taking the proof with it. These stubs MANUFACTURE the
# condition instead: they write a translation unit and report a gcc outcome, so
# every branch of the verdict fires on demand, forever, on any tree, with no
# defect required. They are also the only way to reach the branches a real gcc
# will not produce to order (a gcc that dies by signal).
#
# Each stub implements exactly the CLI the runner drives — `pdc compile <file>
# -o <name>` — and writes build_output/<stem>.c, because "did codegen emit a
# translation unit" is what the runner reads to decide the front end accepted.
stub_pdc() {
  rm -f "$1/target/release/pdc"
  printf '%s\n' "$2" > "$1/target/release/pdc"
  chmod +x "$1/target/release/pdc"
}

# Writes C that gcc genuinely refuses, runs a REAL gcc on it, and exits with the
# code derived from gcc's actual status — the producer half of the contract
# scripts/conformance.sh consumes, which is what makes this a fault injection
# rather than a canned string. Codes are fix/gcc-diagnostics-discarded's
# (src/linker.rs:247-261 at aa63982): 3 refused, 4 ill-typed C, 5 no verdict.
#
# 126 and 127 map to 5, NOT to 3. A missing or unexecutable gcc is a TOOLCHAIN
# outcome by this branch's own contract, and calling it a backend rejection
# would be the exact conflation the branch exists to remove, reproduced inside
# the harness that proves it was removed. The suite also refuses to start
# without a working C compiler (see the preflight above), so an absent gcc is
# reported as what it is rather than surfacing as a wrong-looking verdict.
stub_backend_reject='#!/bin/sh
f=$2; stem=`basename "$f" .pd`
mkdir -p build_output
printf "int main(void) { return not_a_declared_identifier; }\n" > "build_output/$stem.c"
echo "Compiling $f..."
echo "Linking with gcc (-O2)..."
err=`gcc -o /dev/null "build_output/$stem.c" 2>&1`; st=$?
if [ "$st" -eq 0 ]; then exit 0; fi
echo "error: gcc compilation failed:" >&2
echo "$err" >&2
if [ "$st" -ge 128 ] || [ "$st" -eq 126 ] || [ "$st" -eq 127 ]; then exit 5; fi
exit 3'

# gcc exited 0 and diagnosed C that pdc generated: an ICE, and a compiler defect
# for a different reason than a refusal. Distinct exit code, distinct sentence.
stub_ill_typed_c='#!/bin/sh
f=$2; stem=`basename "$f" .pd`
mkdir -p build_output
printf "int main(void) { return 0; }\n" > "build_output/$stem.c"
echo "Linking with gcc (-O2)..."
echo "error: gcc compilation failed:" >&2
echo "build_output/$stem.c:1:1: warning: incompatible integer to pointer conversion" >&2
exit 4'

# Codegen succeeded and the C compiler then died by signal instead of judging the
# translation unit. Nothing is established about the C, so this must NOT be
# reported as a backend defect. The kill is real, not a printed claim.
stub_gcc_abnormal='#!/bin/sh
f=$2; stem=`basename "$f" .pd`
mkdir -p build_output
printf "int main(void) { return 0; }\n" > "build_output/$stem.c"
echo "Linking with gcc (-O2)..."
sh -c "kill -9 \$\$" >/dev/null 2>&1; st=$?
echo "error: gcc compilation failed:" >&2
echo "gcc terminated by a signal" >&2
if [ "$st" -ge 128 ] || [ "$st" -eq 126 ] || [ "$st" -eq 127 ]; then exit 5; fi
exit 3'

# TODAY'"'"'S REAL pdc: a translation unit, a failed build, and the flattened
# exit 1 that cannot say which of the two happened (src/main.rs:137-139 emits
# the same string, and the same status, for a rejected C and for a gcc that
# died). The gate must under-claim here. This is the regression pin for the
# accusation being withheld.
stub_no_provenance='#!/bin/sh
f=$2; stem=`basename "$f" .pd`
mkdir -p build_output
printf "int main(void) { return not_a_declared_identifier; }\n" > "build_output/$stem.c"
echo "Linking with gcc (-O2)..."
echo "error: gcc compilation failed:" >&2
echo "build_output/$stem.c:1:25: error: use of undeclared identifier" >&2
exit 1'

# gcc RAN, exited nonzero, and named no translation unit of ours. Structured —
# so the filesystem witness is not needed — but NOT an accusation: an undefined
# symbol from the link stage and a full disk are indistinguishable here, and the
# producer says so by choosing 6 rather than 3.
stub_gcc_unexplained='#!/bin/sh
f=$2; stem=`basename "$f" .pd`
mkdir -p build_output
printf "int main(void) { return 0; }\n" > "build_output/$stem.c"
echo "Linking with gcc (-O2)..."
echo "error: gcc exited 1 without diagnosing build_output/$stem.c." >&2
echo "gcc: fatal error: cannot write output: No space left on device" >&2
exit 6'

# An exit code outside the contract must not be read as a rejection either.
stub_unknown_code='#!/bin/sh
f=$2; stem=`basename "$f" .pd`
mkdir -p build_output
printf "int main(void) { return 0; }\n" > "build_output/$stem.c"
echo "error: gcc compilation failed:" >&2
exit 42'

# One stub, two behaviours keyed on the fixture path: the fixture under one/
# gets a translation unit and a backend failure, the one under two/ is refused
# by the front end with no .c at all. They share build_output/dup.c, which is
# the point.
stub_selective_reject='#!/bin/sh
f=$2; stem=`basename "$f" .pd`
mkdir -p build_output
case "$f" in
  */one/*)
    printf "int main(void) { return not_a_declared_identifier; }\n" > "build_output/$stem.c"
    echo "error: gcc compilation failed:" >&2
    echo "build_output/$stem.c:1:25: error: use of undeclared identifier" >&2
    exit 3 ;;
  *)
    echo "error: refused by the front end" >&2
    exit 1 ;;
esac'


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

# A program the FRONT END ACCEPTS and whose emitted C gcc then REFUSES. Measured
# on this tree (fb12f6f), `pdc compile` exit 1:
#   build_output/<name>.c:273:22: error: brackets are not allowed here; to
#   declare an array, place the brackets after the identifier
#       long long[2] g[2] = {{1, 2}, {3, 4}};
# `type_to_c` composes `T[M][N]` as a type rather than a declarator — the open
# defect CLAUDE.md records as "중첩 배열이 로컬·파라미터 양쪽에서 불가".
#
# HANDOFF, because this control stands on a live defect. When nested arrays start
# working these cases go RED with the fixture PASSING rather than being refused.
# That is not a runner regression: substitute another program that pdc accepts
# and gcc rejects, measure it, and paste the measurement here. If no such program
# can be found any more, say that and delete the positive control — but do not
# weaken the assertion, because then nothing proves the verdict still fires.
backend_reject_program='fn main() {
    let g: [[i64; 2]; 2] = [[1, 2], [3, 4]];
    print_int(g[1][0]);
}'
# A pure FRONT-END refusal whose diagnostic contains the literal `Linking`:
#   error: Undefined variable or function: 'Linking'
# The stage classifier used to decide "did the backend run?" by grepping the
# compiler log for `Linking`, so a fixture's own identifier could answer that
# question on the backend's behalf. Harmless while the answer only chose a label;
# not harmless once one branch accuses the compiler of a defect.
frontend_reject_linking_program='fn main() {
    print_int(Linking);
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
# The discrimination controls below need the negative form. `expect_out` cannot
# express "this verdict did not misfire": a suite that only ever asserts presence
# is equally happy with a runner that shouts BACKEND_REJECT at everything.
expect_not_out() {
  case "$OUT" in *"$1"*) bad "expected output NOT to contain '$1'"; return 1 ;; esac
  return 0
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
# These cases are grouped by class rather than by symptom.
#
# Be accurate about what that buys. These are REPRESENTATIVE BEHAVIOURAL
# REGRESSIONS, not a mechanism that catches any future member of a class: a
# status collapse on a different artifact, or a pattern use on a different
# input, need not encounter these stimuli and would pass. They pin the
# behaviours below; they do not prove the class is empty.
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
manifest "$D" 'tests/a.pd|run|-|expected|-|-' 'tests/locked.pd|skip|compile|No main function found|-|claims to be a non-program'
run_case "$D" tests
chmod 644 "$D/tests/locked.pd"
expect_rc 1 && expect_out "UNREADABLE" && ok

start "class/status: a declared dangling symlink is a harness failure, not a 'skip'"
D=$(new_repo clsdangle2)
fixture "$D" tests/a.pd "$good_program"
ln -sfn nowhere.pd "$D/tests/dangle.pd"
manifest "$D" 'tests/a.pd|run|-|expected|-|-' 'tests/dangle.pd|skip|compile|No main function found|-|declared non-program'
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

# --- CLASS 4: filesystem indirection (grep and pdc both dereference) --------
start "class/symlink: a fixture symlinked OUTSIDE the repo is refused"
D=$(new_repo clsescape)
fixture "$D" tests/a.pd "$good_program"
mkdir -p "$TMPROOT/outside"
printf 'fn main() {\n    print("unversioned");\n}\n' > "$TMPROOT/outside/evil.pd"
ln -sfn "$TMPROOT/outside/evil.pd" "$D/tests/escape.pd"
printf 'ok\n' > "$D/tests/escape.expected"
manifest "$D" 'tests/a.pd|run|-|expected|-|-' 'tests/escape.pd|run|-|expected|-|-'
run_case "$D" tests
expect_rc 1 && expect_out "ESCAPES_REPO" && ok

start "class/symlink: an INTERNAL symlink fixture is still allowed"
rm -f "$D/tests/escape.pd" "$D/tests/escape.expected"
ln -sfn a.pd "$D/tests/alias.pd"
printf 'ok\n' > "$D/tests/alias.expected"
manifest "$D" 'tests/a.pd|run|-|expected|-|-' 'tests/alias.pd|run|-|expected|-|-'
run_case "$D" tests
expect_rc 0 && expect_out "verified=2" && ok

start "class/symlink: a chain that ends outside the repo is refused"
rm -f "$D/tests/alias.pd" "$D/tests/alias.expected"
ln -sfn "$TMPROOT/outside/evil.pd" "$TMPROOT/outside/hop.pd"
ln -sfn "$TMPROOT/outside/hop.pd" "$D/tests/chain.pd"
printf 'ok\n' > "$D/tests/chain.expected"
manifest "$D" 'tests/a.pd|run|-|expected|-|-' 'tests/chain.pd|run|-|expected|-|-'
run_case "$D" tests
expect_rc 1 && expect_out "ESCAPES_REPO" && ok

# --- class=skip is decided by the COMPILER, not by an `fn main` regex -------
# Measured: `fn /* c */ main()`, `fn // c<LF> main()` and plain `fn<LF> main()`
# all compile and run, and all three evaded the old regex — a real program could
# be declared skip and never gated.
start "skip: 'fn /* c */ main()' cannot masquerade as a non-program"
D=$(new_repo skipevade)
fixture "$D" tests/a.pd "$good_program"
fixture "$D" tests/evade.pd 'fn /* c */ main() {
    print("evaded");
}'
manifest "$D" 'tests/a.pd|run|-|expected|-|-' 'tests/evade.pd|skip|compile|No main function found|-|claims not to be a program'
run_case "$D" tests
expect_rc 1 && expect_out "SKIP_IS_A_PROGRAM" && ok

start "skip: 'fn // c<LF> main()' cannot either"
fixture "$D" tests/evade.pd 'fn // c
 main() {
    print("evaded");
}'
run_case "$D" tests
expect_rc 1 && expect_out "SKIP_IS_A_PROGRAM" && ok

start "skip: plain 'fn<LF> main()' cannot either (no comment needed)"
fixture "$D" tests/evade.pd 'fn
 main() {
    print("evaded");
}'
run_case "$D" tests
expect_rc 1 && expect_out "SKIP_IS_A_PROGRAM" && ok

start "skip: a genuine library module is proven skip by the compiler"
D=$(new_repo skiplib)
fixture "$D" tests/a.pd "$good_program"
fixture "$D" tests/lib.pd "$library_module"
manifest "$D" 'tests/a.pd|run|-|expected|-|-' 'tests/lib.pd|skip|compile|No main function found|-|library module'
run_case "$D" tests
expect_rc 0 && expect_out "skip=1" && ok

start "skip: a wrong diagnostic on a skip row fails the gate"
manifest "$D" 'tests/a.pd|run|-|expected|-|-' 'tests/lib.pd|skip|compile|Unsupported type in reference parameter|-|wrong reason'
run_case "$D" tests
expect_rc 1 && expect_out "SKIP_MISMATCH" && ok

start "skip: a row with no fingerprint is a manifest error"
manifest "$D" 'tests/a.pd|run|-|expected|-|-' 'tests/lib.pd|skip|-|-|-|no proof offered'
run_case "$D" tests
expect_rc 2 && expect_out "needs the diagnostic" && ok

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

# ===========================================================================
# BACKEND_REJECT — the one outcome that is not a class.
#
# There is no valid Palladium program for which pdc accepts the source and gcc
# then refuses the C codegen emitted. If the front end said yes, C that will not
# build is a defect in pdc: never a property of the input, and never something a
# fixture may declare. The runner used to CLASSIFY that outcome (stage `link`)
# and then compare it against the manifest like any other verdict, so a backend
# defect could be declared expected — as an xfail, or, worse, as a `reject`,
# which is counted as COVERAGE and owed to nobody.
#
# Three things are proven below, because a verdict that is only declared is not a
# verdict: that it FIRES on a real reproduction, that it does NOT fire on a
# front-end refusal (the distinction is WHO refused), and that the manifest can
# no longer buy an exemption.
#
# What is NOT proven, and must not be claimed: that this finds backend-reject
# defects. The corpus contains only programs someone thought to write down.
# Measured — neither of the two reproductions that motivated this check is in
# tests/conformance-manifest.txt, so the gate would not have caught either. It
# makes the OUTCOME inadmissible for every fixture the corpus runs; corpus
# coverage is a separate, still-open debt.
# ===========================================================================

# --- the verdict FIRES, on manufactured evidence, forever -------------------
start "backend/injected: a stub whose emitted C gcc refuses reports BACKEND_REJECT"
D=$(new_repo backendinjected)
stub_pdc "$D" "$stub_backend_reject"
fixture "$D" tests/any.pd "$good_program"
manifest "$D" 'tests/any.pd|run|-|expected|-|-'
run_case "$D"
expect_rc 1 && expect_out "BACKEND_REJECT" && ok

start "backend/injected: the message names it a compiler defect, not a fixture property"
expect_out "defect in this compiler" && ok

start "backend/injected: and it quotes gcc's line, not pdc's wrapper"
# The whole message is "go fix the backend", so the useful diagnostic is the
# first error line AFTER `error: gcc compilation failed:` — the wrapper is what
# the first `error` match used to be, and it says nothing.
#
# Asserted on the PLANTED IDENTIFIER, not on the C compiler's sentence around
# it. `undeclared identifier` is clang's wording; GNU gcc says
# `'x' undeclared (first use in this function)` for the same error, so the
# earlier assertion would have gone red on Linux while the classification was
# perfectly correct. Both wordings quote the identifier, and nothing else in
# this runner's output can contain that token by accident. It matters more than
# usual here: the repo's Actions are billing-locked, so Linux never runs this
# suite and a portability defect merged this way is never caught afterwards.
expect_out "not_a_declared_identifier" && ok

start "backend/injected: declaring it xfail does NOT excuse it"
manifest "$D" 'tests/any.pd|xfail|compile|undeclared identifier|M1|claims the defect is expected'
run_case "$D"
expect_rc 1 && expect_out "BACKEND_REJECT" && expect_not_out "xfail=1" && ok

start "backend/injected: declaring it a NEGATIVE TEST does not launder it into coverage"
# The worst spelling of the escape hatch: class=reject counts as coverage and is
# owed to no milestone, so a backend defect declared this way would have made the
# corpus look BETTER for containing it.
manifest "$D" 'tests/any.pd|reject|compile|undeclared identifier|-|claims the compiler refuses this'
run_case "$D"
expect_rc 1 && expect_out "BACKEND_REJECT" && expect_not_out "reject=1" && ok

# --- the accusation is WITHHELD when the evidence cannot support it ---------
# `gcc compilation failed` is emitted for every unsuccessful gcc status
# (src/main.rs:137-139), so it cannot separate "gcc refused our C" from "gcc
# died". These pin the under-claim: same never-expectable outcome, same red
# gate, no defect asserted.
start "backend/ambiguous: no structured signal is HARNESS_ERROR, never BACKEND_REJECT"
# This is TODAY'S REAL pdc. Until fix/gcc-diagnostics-discarded lands, every
# fixture that reaches this point takes this path.
D=$(new_repo backendambiguous)
stub_pdc "$D" "$stub_no_provenance"
fixture "$D" tests/any.pd "$good_program"
manifest "$D" 'tests/any.pd|run|-|expected|-|-'
run_case "$D"
expect_rc 1 && expect_out "HARNESS_ERROR" && expect_not_out "BACKEND_REJECT" && ok

start "backend/ambiguous: ...and the message says WHY it will not name a defect"
expect_out "does not say what happened" && ok

start "backend/ambiguous: ...and no manifest column excuses it either"
manifest "$D" 'tests/any.pd|reject|compile|undeclared identifier|-|claims coverage'
run_case "$D"
expect_rc 1 && expect_out "HARNESS_ERROR" && expect_not_out "reject=1" && ok

start "backend/unexplained: exit 6 is HARNESS_ERROR, never BACKEND_REJECT"
# gcc reached a verdict, so this is NOT the toolchain case; pdc could not
# attribute it, so it is NOT a rejection. The gate must refuse without accusing.
D=$(new_repo backendunexplained)
stub_pdc "$D" "$stub_gcc_unexplained"
fixture "$D" tests/any.pd "$good_program"
manifest "$D" 'tests/any.pd|run|-|expected|-|-'
run_case "$D"
expect_rc 1 && expect_out "HARNESS_ERROR" && expect_not_out "BACKEND_REJECT" && ok

start "backend/unexplained: ...and no manifest column excuses it"
manifest "$D" 'tests/any.pd|reject|compile|undeclared identifier|-|claims coverage'
run_case "$D"
expect_rc 1 && expect_out "HARNESS_ERROR" && expect_not_out "reject=1" && ok

start "backend/unexplained: ...and the code alone suffices, with no .c on disk"
# THE FAIL-OPEN THIS CLOSES. Without 6 in the backend_code case, a 6 whose
# translation unit is missing or differently named falls through to the
# front-end arm, where `compile` is a stage a manifest row may declare — and the
# contradiction check that would have caught it greps for `gcc compilation
# failed`, which the exit-6 message deliberately does not print, because a gate
# reading that marker reads a claim nobody supported.
stub_pdc "$D" '#!/bin/sh
echo "Linking with gcc (-O2)..."
echo "error: gcc exited 1 without diagnosing anything." >&2
exit 6'
manifest "$D" 'tests/any.pd|reject|compile|undeclared identifier|-|claims coverage'
run_case "$D"
expect_rc 1 && expect_out "HARNESS_ERROR" && expect_not_out "reject=1" && ok

start "backend/abnormal: a gcc that DIES is HARNESS_ERROR, not a backend defect"
# A real SIGKILL of a real child process, with valid C on disk. Nothing is
# established about the translation unit, so nothing may be claimed about it.
D=$(new_repo backendabnormal)
stub_pdc "$D" "$stub_gcc_abnormal"
fixture "$D" tests/any.pd "$good_program"
manifest "$D" 'tests/any.pd|run|-|expected|-|-'
run_case "$D"
expect_rc 1 && expect_out "HARNESS_ERROR" && expect_not_out "BACKEND_REJECT" && ok

start "backend/abnormal: ...and it is distinguished from a rejected translation unit"
expect_out "never reached a verdict" && ok

# --- the contract itself fails closed ---------------------------------------
start "backend/ill-typed: exit 4 is a defect too, with its own sentence"
# gcc exited 0 and diagnosed C that pdc GENERATED. Not a refusal, still a
# compiler defect, and the message must not claim gcc refused anything.
D=$(new_repo backendilltyped)
stub_pdc "$D" "$stub_ill_typed_c"
fixture "$D" tests/any.pd "$good_program"
manifest "$D" 'tests/any.pd|run|-|expected|-|-'
run_case "$D"
expect_rc 1 && expect_out "BACKEND_REJECT" && expect_out "diagnosed as ill-typed" && ok

start "backend/contract: an exit code outside the contract reads as unresolved"
# Anything not in {3,4,5} says nothing about gcc and may not be upgraded into an
# accusation. A contract that treats an unknown code as a rejection would be a
# guessing classifier again — the exact defect this change removed from the log.
D=$(new_repo backendunknowncode)
stub_pdc "$D" "$stub_unknown_code"
fixture "$D" tests/any.pd "$good_program"
manifest "$D" 'tests/any.pd|run|-|expected|-|-'
run_case "$D"
expect_rc 1 && expect_out "HARNESS_ERROR" && expect_not_out "BACKEND_REJECT" && ok

start "backend/contract: ...and it names the code it could not interpret"
expect_out "pdc exited 42" && ok

# --- the structured code stands alone ---------------------------------------
# The witness used to be a conjunction: a structured code was examined only if
# the translation unit was also on disk. That fails OPEN on the half that is
# missing — pdc exits 3 while codegen names its output differently than this gate
# derives it, and the fixture falls through to stage `compile`, which
# `reject|compile` is allowed to declare. These three drive each structured code
# with NO .c on disk AND no legacy `gcc compilation failed` prose in the log,
# which is exactly the combination the old guard could not see: the previous
# no-TU coverage was exit 1 WITH the wrapper, the one case that already worked.
stub_no_tu_3='#!/bin/sh
echo "Linking with gcc (-O2)..."
echo "error: the C compiler refused the generated translation unit" >&2
echo "somewhere.c:1:25: error: not_a_declared_identifier" >&2
exit 3'
stub_no_tu_4='#!/bin/sh
echo "error: the generated translation unit is ill-typed" >&2
exit 4'
stub_no_tu_5='#!/bin/sh
echo "error: the C compiler could not be started" >&2
exit 5'

start "backend/no-tu: exit 3 alone is conclusive, with no .c and no wrapper text"
D=$(new_repo backendnotu3)
stub_pdc "$D" "$stub_no_tu_3"
fixture "$D" tests/any.pd "$good_program"
manifest "$D" 'tests/any.pd|run|-|expected|-|-'
run_case "$D"
expect_rc 1 && expect_out "BACKEND_REJECT" && ok

start "backend/no-tu: ...and it says the exit code is the witness"
expect_out "no translation unit at" && expect_out "sufficient on its own" && ok

start "backend/no-tu: ...and a reject|compile row still cannot bless it"
# The regression this whole item is about: under the old AND-guard this landed
# in the front-end arm and this row made the gate GREEN.
manifest "$D" 'tests/any.pd|reject|compile|not_a_declared_identifier|-|claims coverage'
run_case "$D"
expect_rc 1 && expect_out "BACKEND_REJECT" && expect_not_out "reject=1" && ok

start "backend/no-tu: ...nor an xfail|compile row"
manifest "$D" 'tests/any.pd|xfail|compile|not_a_declared_identifier|M1|claims a debt'
run_case "$D"
expect_rc 1 && expect_out "BACKEND_REJECT" && expect_not_out "xfail=1" && ok

start "backend/no-tu: exit 4 alone is conclusive too"
D=$(new_repo backendnotu4)
stub_pdc "$D" "$stub_no_tu_4"
fixture "$D" tests/any.pd "$good_program"
manifest "$D" 'tests/any.pd|reject|compile|ill-typed|-|claims coverage'
run_case "$D"
expect_rc 1 && expect_out "BACKEND_REJECT" && expect_not_out "reject=1" && ok

start "backend/no-tu: exit 5 alone is conclusive, and still claims nothing"
D=$(new_repo backendnotu5)
stub_pdc "$D" "$stub_no_tu_5"
fixture "$D" tests/any.pd "$good_program"
manifest "$D" 'tests/any.pd|reject|compile|could not be started|-|claims coverage'
run_case "$D"
expect_rc 1 && expect_out "HARNESS_ERROR" && expect_not_out "BACKEND_REJECT" \
  && expect_not_out "reject=1" && ok

start "backend/no-tu: an UNSTRUCTURED code with no .c is still a front-end refusal"
# The other half of the same boundary: exit 1 is not a witness, so with no .c on
# disk this is an ordinary front-end rejection and MUST stay declarable. A guard
# that answered "backend" here would break every negative test in the corpus.
D=$(new_repo backendnotu1)
stub_pdc "$D" '#!/bin/sh
echo "error: Expected function, struct, enum" >&2
exit 1'
fixture "$D" tests/any.pd "$good_program"
manifest "$D" 'tests/any.pd|reject|compile|Expected function, struct, enum|-|a real negative test'
run_case "$D"
expect_rc 0 && expect_out "reject=1" && expect_not_out "BACKEND_REJECT" && ok

# --- the contradiction check ------------------------------------------------
start "backend/contradiction: gcc ran but no translation unit exists is refused"
# Reaching the front-end arm means "no .c on disk". That name is derived twice,
# independently (this gate's basename vs codegen's file_stem), and if the two
# ever diverge a real backend failure would land in the arm where `xfail
# compile` IS declarable. A stub that fails after gcc without writing the .c
# manufactures exactly that divergence.
D=$(new_repo backendcontradiction)
stub_pdc "$D" '#!/bin/sh
echo "Linking with gcc (-O2)..."
echo "error: gcc compilation failed:" >&2
echo "somewhere.c:1:1: error: broken" >&2
exit 1'
fixture "$D" tests/any.pd "$good_program"
manifest "$D" 'tests/any.pd|run|-|expected|-|-'
run_case "$D"
expect_rc 1 && expect_out "HARNESS_ERROR" && expect_out "cannot both be true" && ok

start "backend/contradiction: ...and it cannot be declared xfail at the compile stage"
manifest "$D" 'tests/any.pd|xfail|compile|broken|M1|claims a front-end refusal'
run_case "$D"
expect_rc 1 && expect_out "HARNESS_ERROR" && expect_not_out "xfail=1" && ok

# --- the manifest can no longer buy an exemption ----------------------------
start "backend/manifest: stage 'link' is a manifest error (the hatch cannot reopen)"
# The red-proof for the validator. On the real corpus this check is green and
# stays green (measured: 82 non-comment rows, stage column 58 `-` + 24 `compile`,
# zero `link`), so a control that plants the row is the only way to see it work.
D=$(new_repo backendlinkstage)
fixture "$D" tests/any.pd "$good_program"
manifest "$D" 'tests/any.pd|xfail|link|gcc compilation failed|M1|declares the defect expected'
run_case "$D"
expect_rc 2 && expect_out "declares stage 'link'" && ok

start "backend/manifest: ...on class=reject too"
manifest "$D" 'tests/any.pd|reject|link|gcc compilation failed|-|declares the defect expected'
run_case "$D"
expect_rc 2 && expect_out "declares stage 'link'" && ok

start "backend/manifest: ...and on class=skip"
manifest "$D" 'tests/any.pd|skip|link|gcc compilation failed|-|declares the defect expected'
run_case "$D"
expect_rc 2 && expect_out "declares stage 'link'" && ok

start "backend/manifest: 'compile' and 'run' are still accepted stages"
D=$(new_repo backendstage)
fixture "$D" tests/broken.pd "$bad_program"
fixture "$D" tests/rt.pd "$runtime_fail_program"
manifest "$D" 'tests/broken.pd|xfail|compile|Expected function, struct, enum|M1|parse failure' \
              'tests/rt.pd|xfail|run|exit=3|M1|exits 3 by design'
run_case "$D"
expect_rc 0 && expect_out "xfail=2" && ok

# --- negative control: nothing else moved ----------------------------------
start "backend/negative: an ordinary passing fixture is untouched"
D=$(new_repo backendnegative)
fixture "$D" tests/a.pd "$good_program"
manifest "$D" 'tests/a.pd|run|-|expected|-|-'
run_case "$D"
expect_rc 0 && expect_out "verified=1" && expect_not_out "BACKEND_REJECT" \
  && expect_not_out "HARNESS_ERROR" && ok

# --- discrimination: WHO refused -------------------------------------------
# A check that cannot tell a front-end refusal from a backend one is the failure
# mode this whole section exists to close: it would call every negative test in
# the corpus a compiler defect.
start "backend/discrimination: a front-end refusal is COMPILE_FAIL, not a backend verdict"
D=$(new_repo backenddiscriminate)
fixture "$D" tests/refused.pd "$bad_program"
manifest "$D" 'tests/refused.pd|run|-|expected|-|-'
run_case "$D"
expect_rc 1 && expect_out "COMPILE_FAIL" && expect_not_out "BACKEND_REJECT" \
  && expect_not_out "HARNESS_ERROR" && ok

start "backend/discrimination: ...and may still be declared xfail (green)"
manifest "$D" 'tests/refused.pd|xfail|compile|Expected function, struct, enum|M1|known parse failure'
run_case "$D"
expect_rc 0 && expect_out "xfail=1" && ok

start "backend/discrimination: ...and reject stays real coverage (green)"
manifest "$D" 'tests/refused.pd|reject|compile|Expected function, struct, enum|-|the compiler must refuse this'
run_case "$D"
expect_rc 0 && expect_out "reject=1" && ok

start "backend/discrimination: a fixture cannot put 'Linking' in the log to fake it"
# `fn main() { print_int(Linking); }` — a front-end refusal reading "Undefined
# variable or function: 'Linking'". The old classifier grepped the log for that
# literal, so the fixture answered "did the backend run?" itself. Acceptance is
# now read off the filesystem (was the translation unit emitted?), which no
# fixture text can reach.
D=$(new_repo backendforge)
fixture "$D" tests/forge.pd "$frontend_reject_linking_program"
manifest "$D" 'tests/forge.pd|reject|compile|Undefined variable or function|-|the compiler must refuse an undefined name'
run_case "$D"
expect_rc 0 && expect_out "reject=1" && expect_not_out "BACKEND_REJECT" && ok

start "backend/discrimination: and it is a compile-stage failure when undeclared"
manifest "$D" 'tests/forge.pd|run|-|expected|-|-'
run_case "$D"
expect_rc 1 && expect_out "COMPILE_FAIL" && expect_not_out "BACKEND_REJECT" && ok

start "backend/stale: a same-basename fixture's C cannot make a refusal look like a defect"
# The emitted C is named from the fixture BASENAME, so tests/one/dup.pd and
# tests/two/dup.pd share build_output/dup.c. Without removing it first, the
# first fixture's translation unit would still be on disk when the second is
# refused by the front end, and the second would be accused of a backend defect.
# `find` output is sorted, so one/ runs before two/.
D=$(new_repo backendstale)
stub_pdc "$D" "$stub_selective_reject"
fixture "$D" tests/one/dup.pd "$good_program"
fixture "$D" tests/two/dup.pd "$good_program"
manifest "$D" 'tests/one/dup.pd|run|-|expected|-|-' \
              'tests/two/dup.pd|reject|compile|refused by the front end|-|front-end refusal'
run_case "$D"
expect_rc 1 && expect_out "BACKEND_REJECT" && expect_out "reject=1" && ok

# --- the live reproduction, kept as evidence but load-bearing on nothing ----
start "backend/live: the nested-array defect still fails the gate today"
# Real pdc, real defect: the front end accepts `[[i64; 2]; 2]` and gcc refuses
# the `long long[2] g[2]` codegen emits. This is EVIDENCE, not the proof — the
# fault-injected controls above own that, so this case may be deleted outright
# the day nested arrays start working. It asserts only the invariant-level
# facts, which is why it survives fix/gcc-diagnostics-discarded landing and
# changing the verdict's NAME: the gate is red, and the fixture is counted as
# neither coverage nor debt.
D=$(new_repo backendlive)
fixture "$D" tests/nested.pd "$backend_reject_program"
manifest "$D" 'tests/nested.pd|reject|compile|brackets are not allowed here|-|claims the compiler refuses this'
run_case "$D"
expect_rc 1 && expect_not_out "reject=1" && expect_not_out "xfail=1" && ok

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
manifest "$D" 'tests/a.pd|skip|compile|No main function found|-|claims not to be a program'
run_case "$D"
expect_rc 1 && expect_out "SKIP_IS_A_PROGRAM" && ok

start "manifest: class=run on a library module with no fn main is rejected"
D=$(new_repo libclass)
fixture "$D" tests/lib.pd "$library_module"
manifest "$D" 'tests/lib.pd|run|-|expected|-|-'
run_case "$D"
# The compiler reports this now, not a regex: "No main function found".
expect_rc 1 && expect_out "COMPILE_FAIL" && ok

start "manifest: declaring it skip is the correct, explicit resolution"
manifest "$D" 'tests/lib.pd|skip|compile|No main function found|-|library module, no fn main by design'
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
