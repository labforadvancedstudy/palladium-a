#!/usr/bin/env bash
# Palladium version gate: every shipped binary must report the version it was built from.
#
# WHAT BROKE, MEASURED. src/cli.rs carried `#[command(version = "0.1.0-alpha")]` — a
# literal, typed once and never touched again. Cargo.toml went 0.1.0 -> 0.2.0 -> 0.3.0 and
# the literal did not move. On the maintainer's machine, 2026-08-22:
#
#     brew list --versions pdc pdc-preview   ->  pdc 0.2.0 · pdc-preview 0.3.0
#     pdc --version                          ->  pdc 0.1.0-alpha
#
# `git show v0.2.0:src/cli.rs` carries the same literal, so v0.2.0 and v0.3.0 each shipped
# a compiler that misreports itself. Nothing failed anywhere, for two releases, because
# nothing ever asked the binary what it thought it was.
#
# WHY THIS IS NOT A GREP FOR `env!`. The cheap check is "does src/cli.rs mention
# CARGO_PKG_VERSION". That check passes on a binary that prints ANYTHING: it reads the
# source, and the source is not what the user runs. A second `version` attribute later in
# the same derive, a `--version` handled by hand before clap sees it, a stale target/ from
# before the bump, a packaging wrapper — each breaks the reported version with the source
# still saying the right words. So this gate does the only thing that answers the question
# it is asking: it EXECUTES each built binary with `--version`, reads what it printed, and
# compares that against the version cargo compiled it from.
#
# THE BINARY LIST IS DERIVED, NOT TYPED. Both the expected version and the set of binaries
# come out of Cargo.toml — the same file that produces CARGO_PKG_VERSION. A fourth
# `[[bin]]` is covered the day it is added, with no second place to remember.
#
# WHAT THIS DOES NOT ASSERT. Only the version token is a pass/fail condition, because a
# wrong version is the defect that shipped. The whole `--version` line of every binary is
# PRINTED, so the name half is in front of whoever reads this gate's output rather than
# behind a warning that can never fail. (Today `pls` prints `alan-von-palladium`, its
# package name, not its binary name. Correct version, and out of this gate's contract.)
#
# Usage: scripts/version-gate.sh [bin-dir]        (default: target/release)
#
# The optional argument exists so scripts/test-version-gate.sh can hand this gate binaries
# that lie, and check that it says so. `make version-gate` passes nothing, so the
# certifying path always measures target/release; the directory measured is printed on
# every run, so a run against anything else is visible in its own output.

set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

BIN_DIR=${1:-target/release}
GREEN=$'\033[0;32m'; RED=$'\033[0;31m'; NC=$'\033[0m'

fail() { echo; echo "${RED}❌ VERSION GATE FAILED: $*${NC}"; exit 1; }

[ -f Cargo.toml ] || fail "Cargo.toml missing — there is no version to be right about"

# awk -F'"' on the manifest, not a TOML library: bash 3.2 is the floor here (see the
# comment in gate-receipts.sh) and every value read is a plain quoted string.
EXPECTED=$(awk -F'"' '
  /^\[package\]/ { p = 1; next }
  /^\[/          { p = 0 }
  p && /^[ \t]*version[ \t]*=/ { print $2; exit }
' Cargo.toml)
[ -n "$EXPECTED" ] || fail "no [package] version in Cargo.toml — cannot tell what is correct"

BINS=()
while IFS= read -r b; do
  [ -n "$b" ] && BINS+=("$b")
done < <(awk -F'"' '
  /^\[\[bin\]\]/ { b = 1; next }
  /^\[/          { b = 0 }
  b && /^[ \t]*name[ \t]*=/ { print $2 }
' Cargo.toml)
[ "${#BINS[@]}" -gt 0 ] || fail "Cargo.toml declares no [[bin]] — nothing to measure"

echo "=============================================="
echo "version gate: ${#BINS[@]} binary(s) declared in Cargo.toml"
echo "  expected (CARGO_PKG_VERSION): $EXPECTED"
echo "  measured in:                  $BIN_DIR"
echo "=============================================="

failures=0
for bin in "${BINS[@]}"; do
  path="$BIN_DIR/$bin"

  if [ ! -x "$path" ]; then
    printf '  %sFAIL%s %-5s %s is not an executable file (cargo build --release)\n' \
      "$RED" "$NC" "$bin" "$path"
    failures=$((failures+1))
    continue
  fi

  # Combined stream: a binary is free to print its version on either, and what it told the
  # user is the whole of what it said.
  out=$("$path" --version 2>&1)
  rc=$?
  if [ "$rc" -ne 0 ]; then
    printf '  %sFAIL%s %-5s `%s --version` exited %s\n' "$RED" "$NC" "$bin" "$bin" "$rc"
    printf '       %-5s it printed: %s\n' "" "$(printf '%s' "$out" | head -3)"
    failures=$((failures+1))
    continue
  fi

  # One line, two fields. Not decoration: the gate has to LOCATE the version in the output
  # before it can compare it, and `<name> <version>` is what clap prints and what all three
  # binaries print today. Anything else — silence, a banner, a version with a suffix glued
  # on — is a shape this gate cannot read a version out of, and guessing at one is how a
  # check starts passing on output nobody has looked at.
  lines=$(printf '%s\n' "$out" | grep -c '[^[:space:]]')
  set -f            # the split below is unquoted on purpose; a `*` in the output is not a glob
  set -- $out
  set +f
  if [ "$lines" -ne 1 ] || [ "$#" -ne 2 ]; then
    printf '  %sFAIL%s %-5s `%s --version` is not `<name> <version>` on one line\n' \
      "$RED" "$NC" "$bin" "$bin"
    printf '       %-5s it printed (%s non-blank line(s), %s field(s)): %s\n' \
      "" "$lines" "$#" "${out:-<nothing>}"
    failures=$((failures+1))
    continue
  fi
  reported=$2

  if [ "$reported" = "$EXPECTED" ]; then
    printf '  %sok%s   %-5s %s\n' "$GREEN" "$NC" "$bin" "$out"
  else
    printf '  %sFAIL%s %-5s reports %s, cargo built it from %s\n' \
      "$RED" "$NC" "$bin" "$reported" "$EXPECTED"
    printf '       %-5s `%s --version` printed: %s\n' "" "$bin" "$out"
    printf '       %-5s derive it — `#[command(version)]` or env!("CARGO_PKG_VERSION") —\n' ""
    printf '       %-5s do not retype the literal, or this returns one release later.\n' ""
    failures=$((failures+1))
  fi
done

echo "=============================================="
if [ "$failures" -gt 0 ]; then
  fail "$failures of ${#BINS[@]} binary(s) disagree with Cargo.toml ($EXPECTED)"
fi
echo "${GREEN}✅ version gate green${NC} — every binary reports $EXPECTED, the version it was built from."
echo "=============================================="
exit 0
