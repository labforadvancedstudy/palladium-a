#!/usr/bin/env bash
# Fault injection for the version gate.
#
# A gate that has only ever been seen green is a gate nobody has tested. This hands
# scripts/version-gate.sh binaries that lie, in each of the ways a binary can lie about its
# own version, and requires it to say so — and hands it honest ones and requires it to
# stay quiet, so "always red" is excluded too.
#
# The saboteurs are stub executables, not a rebuilt compiler. The gate's whole job is to
# execute `<bin> --version` and judge what came back, so a stub that prints
# `pdc 0.1.0-alpha` — the literal that actually shipped in v0.2.0 and v0.3.0 — exercises
# exactly the comparison under test, in a second rather than a release build per case. The
# real compiler is measured by `make version-gate` itself, which this does not replace.
#
# The last two cases are about WIRING, which fault injection cannot see: a perfect gate
# that no target runs is not a gate. `make -n` resolves prerequisites for real, which a
# grep of the prerequisite line would not (the same reasoning as the `gates` wiring block
# in test-doc-evidence.sh).
#
# Usage: scripts/test-version-gate.sh

set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

GATE="scripts/version-gate.sh"
TMP=$(mktemp -d) || exit 2
trap 'rm -rf "$TMP"' EXIT INT TERM
GREEN=$'\033[0;32m'; RED=$'\033[0;31m'; NC=$'\033[0m'
pass=0; fail=0

# The version the stubs must agree with, read the way the gate reads it. Read once here so
# a case cannot be written against a version that has been bumped since.
VERSION=$(awk -F'"' '
  /^\[package\]/ { p = 1; next }
  /^\[/          { p = 0 }
  p && /^[ \t]*version[ \t]*=/ { print $2; exit }
' Cargo.toml)
[ -n "$VERSION" ] || { echo "cannot read [package] version from Cargo.toml" >&2; exit 2; }

check() {  # check <name> <expected_exit> <actual_exit> [detail]
  if [ "$2" = "$3" ]; then
    printf '  %sok%s   %s\n' "$GREEN" "$NC" "$1"; pass=$((pass+1))
  else
    printf '  %sFAIL%s %s — expected exit %s, got %s %s\n' "$RED" "$NC" "$1" "$2" "$3" "${4:-}"
    fail=$((fail+1))
  fi
}

says() {  # says <name> <needle>   — the last gate run must have PRINTED this
  if grep -qF -- "$2" "$TMP/out"; then
    printf '  %sok%s   %s\n' "$GREEN" "$NC" "$1"; pass=$((pass+1))
  else
    printf '  %sFAIL%s %s — its output never mentions: %s\n' "$RED" "$NC" "$1" "$2"
    fail=$((fail+1))
  fi
}

# --- the world the gate measures -------------------------------------------
# Cargo.toml declares three binaries today; the gate derives that list from the manifest
# rather than carrying its own copy, so a stub dir must supply all three or the gate is
# right to fail on the missing one. Named literally here on purpose: if a later change
# narrows what the gate covers, this file goes red.
DECLARED="pdc pdm pls"

# mk <dir> <name> <body> — a stub binary
mk() { mkdir -p "$1"; printf '%s\n' "$3" >"$1/$2"; chmod +x "$1/$2"; }

# honest <dir> — every declared binary, each reporting the manifest version
honest() {
  for b in $DECLARED; do
    mk "$1" "$b" "#!/bin/sh
echo \"$b $VERSION\""
  done
}

run() {  # run <bin-dir>  -> gate exit code, output in $TMP/out
  bash "$GATE" "$1" >"$TMP/out" 2>&1
}

echo "=============================================="
echo "version gate fault injection (manifest version: $VERSION)"
echo "=============================================="

echo "== honest binaries: the gate must go GREEN =="
# Without this the whole file is satisfied by a gate that returns 1 unconditionally.
honest "$TMP/honest"
run "$TMP/honest"
check "all three report $VERSION" 0 $?
for b in $DECLARED; do
  says "  and it actually measured $b" "$b"
done

echo "== the defect that shipped: a stale hardcoded literal =="
honest "$TMP/stale"
mk "$TMP/stale" pdc '#!/bin/sh
echo "pdc 0.1.0-alpha"'
run "$TMP/stale"
check "pdc reports 0.1.0-alpha while the manifest says $VERSION" 1 $?
says "  names what the binary reported" "0.1.0-alpha"
says "  names what it was built from" "$VERSION"
says "  names WHICH binary disagreed" "pdc"

echo "== a wrong version that is merely one bump behind =="
# 0.2.0 is a plausible version, not obvious garbage: nothing here may be keyed to the
# string "0.1.0-alpha" specifically.
honest "$TMP/behind"
mk "$TMP/behind" pdm '#!/bin/sh
echo "pdm 0.2.0"'
run "$TMP/behind"
check "pdm reports 0.2.0" 1 $?
says "  names the stale version" "0.2.0"

echo "== a binary that prints the right version in an unreadable shape =="
# The point of the whole gate is that it reads the artefact, so a check that merely looks
# for the version somewhere in the output would pass this AND would pass a banner that
# also contained a second, wrong version. The gate must refuse output it cannot locate a
# version in rather than guess.
honest "$TMP/banner"
mk "$TMP/banner" pdc "#!/bin/sh
echo 'Alan von Palladium Compiler'
echo 'version $VERSION'"
run "$TMP/banner"
check "pdc prints a two-line banner containing $VERSION" 1 $?

echo "== a binary that does not answer --version at all =="
honest "$TMP/silent"
mk "$TMP/silent" pls '#!/bin/sh
exit 0'
run "$TMP/silent"
check "pls prints nothing and exits 0" 1 $?

honest "$TMP/rc"
mk "$TMP/rc" pls "#!/bin/sh
echo \"pls $VERSION\"
exit 3"
run "$TMP/rc"
check "pls prints the right version and exits 3" 1 $?
says "  names the exit code" "exited 3"

echo "== a declared binary that was never built =="
honest "$TMP/missing"
rm -f "$TMP/missing/pdm"
run "$TMP/missing"
check "pdm is declared in Cargo.toml but absent from the build dir" 1 $?

echo "== an empty build directory =="
mkdir -p "$TMP/empty"
run "$TMP/empty"
check "nothing built at all" 1 $?

echo "== wiring: the gate has to be reachable =="
# `make version-gate` must pass NO bin-dir argument, or the certifying path could be
# pointed at a directory of obliging stubs — the same hole gate-receipts.sh closed when it
# stopped letting a caller name the bytes it validated.
recipe=$(make -n version-gate 2>/dev/null | grep -F "scripts/version-gate.sh")
if [ -n "$recipe" ]; then
  printf '  %sok%s   make version-gate runs scripts/version-gate.sh\n' "$GREEN" "$NC"
  pass=$((pass+1))
else
  printf '  %sFAIL%s make version-gate does not run scripts/version-gate.sh\n' "$RED" "$NC"
  fail=$((fail+1))
fi
if [ -n "$recipe" ] && printf '%s\n' "$recipe" | grep -qE 'version-gate\.sh[[:space:]]*$'; then
  printf '  %sok%s   it measures the default build dir (no override argument)\n' "$GREEN" "$NC"
  pass=$((pass+1))
else
  printf '  %sFAIL%s make version-gate passes an argument: %s\n' "$RED" "$NC" "$recipe"
  printf '         (an override there lets the certifying run measure stubs)\n'
  fail=$((fail+1))
fi

gates_dry=$(make -n gates 2>/dev/null)
if printf '%s\n' "$gates_dry" | grep -qF "scripts/version-gate.sh"; then
  printf '  %sok%s   make gates runs scripts/version-gate.sh\n' "$GREEN" "$NC"; pass=$((pass+1))
else
  printf '  %sFAIL%s make gates does NOT run scripts/version-gate.sh\n' "$RED" "$NC"
  printf '         (dropping it from the prerequisites means it never runs, so nothing\n'
  printf '          fails and nothing notices — which is how this defect shipped twice)\n'
  fail=$((fail+1))
fi
if printf '%s\n' "$gates_dry" | grep -qF "scripts/test-version-gate.sh"; then
  printf '  %sok%s   make gates runs scripts/test-version-gate.sh\n' "$GREEN" "$NC"; pass=$((pass+1))
else
  printf '  %sFAIL%s make gates does NOT run scripts/test-version-gate.sh\n' "$RED" "$NC"
  fail=$((fail+1))
fi

echo "=============================================="
echo "$pass passed, $fail failed"
echo "=============================================="
[ "$fail" -eq 0 ] || exit 1
echo "${GREEN}✅ the version gate goes red on every way a binary can misreport itself${NC}"
exit 0
