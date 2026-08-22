#!/usr/bin/env bash
# Execute every gate the feature index cites, once, and validate the index against what
# they printed IN THIS RUN.
#
# WHY THIS IS A SEPARATE TARGET
# `gate:` was the last unexecuted evidence class. Eight outcomes, validated only as "a
# Make target by that name exists" — so two of them still carried the conformance output
# format from before it changed shape, under a green gate. It is the same disease the
# `cmd:` work removed, one door down.
#
# It cannot be fixed inside the doc lint. `make check-docs` is itself a step of
# `make gates`, and the gates cited in the index are `make conformance` and
# `make selfhost`, so a lint that ran them would recurse into its own caller. The
# recursion is an artefact of asking one target to both lint documents and certify the
# build, so the two jobs are split:
#
#   make check-docs      static: citations, fences, evidence tags, and every `cmd:` item
#                        actually run. Prints `gate=N, NONE validated` — a counted,
#                        named seam, never a silent skip.
#   make gate-receipts   this file: run each DISTINCT cited gate once, then validate.
#   make gates           includes both, so the target that CERTIFIES has no unvalidated
#                        evidence of any class.
#
# DEDUPLICATION IS THE POINT OF "DISTINCT". Three rows cite `make selfhost`; it runs once.
#
# WHAT A RECEIPT PROVES. Not merely that the gate passed: every checkable token in the
# declared result — a key=value, or a quoted span — must appear literally in what the gate
# printed in this run. That is what stops `-> fixtures=65 ... verified=45` from rotting
# the way `-> total=42 pass=39` did. A result with no checkable token is prose and the
# checker rejects it.
#
# STALENESS IS STRUCTURAL, NOT CORRELATIONAL. The previous design wrote a run id into the
# receipts directory and asked the checker to compare it with a caller-supplied string.
# That is correlation dressed as freshness: the id sat NEXT TO the bytes it was meant to
# authenticate, so anyone could
#     check_doc_evidence.py --gate-receipts build_output/gate-receipts \
#                           --gate-run-id "$(cat build_output/gate-receipts/RUN_ID)"
# and validate a week-old run. Measured — it printed 10/10. The comment claiming only the
# producing invocation knew the id was simply false.
#
# Receipts now live in a private `mktemp -d` removed when this script exits, on every
# path including failure and interrupt. THE GUARANTEE, STATED AT ITS REAL WIDTH: a
# receipts directory is not DISCOVERABLE by, and not REUSABLE by, the certifying path —
# every invocation mints a fresh unpredictable path and passes only that one to the
# checker, so no later run can be pointed at an earlier run's bytes. It is NOT
# "nothing survives": a SIGKILL or a host failure leaves the directory behind, and the
# checker will read an external directory that is explicitly handed to it. What that
# residue cannot do is contaminate a later certifying run. The checker additionally
# refuses a receipts directory inside the repository, so one cannot be committed and
# pointed at. The same change removes a race: every invocation used to share
# build_output/gate-receipts, so two concurrent runs tore each other's files.
#
# Usage: bash scripts/gate-receipts.sh

set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

OUT=$(mktemp -d "${TMPDIR:-/tmp}/palladium-gate-receipts.XXXXXX") || exit 2
trap 'rm -rf "$OUT"' EXIT INT TERM
GREEN=$'\033[0;32m'; RED=$'\033[0;31m'; NC=$'\033[0m'

# A cited command is A STRING FROM A DOCUMENT, and this script EXECUTES it. Two earlier
# designs got that wrong in two different ways:
#
#   1. a prefix check plus `eval`, so the document controlled the shell;
#   2. an argv grammar — `make <one lowercase target>` — which closed the shell and then
#      admitted THE ENTIRE MAKEFILE. `make publish` is a lowercase target. It runs
#      `cargo publish` (Makefile:236-239). So one line in a documentation file made the
#      documentation-evidence gate publish this crate to crates.io, on the `gates` path
#      and in CI. `make install`, `make uninstall` and `make clean` were accepted too.
#
# The lesson is the one the `find` grammar taught: when validating the SHAPE of a thing
# keeps admitting things you did not mean, stop validating the shape and enumerate what
# you actually need. Five commands are cited by the index today, and these are those five,
# by exact string. Every one is read-only with respect to anything outside this checkout:
# they compile, run fixtures, and print.
#
# ADDING ONE IS A DELIBERATE EDIT HERE, and the thing to ask before making it is not "does
# it look like a gate" but "what does this do that cannot be undone".
GATE_COMMANDS='make conformance
make selfhost
make stdlib-gate
cargo build --release
cargo test --release --lib lsp::'

allowed() {
  local c
  while IFS= read -r c; do
    [ "$1" = "$c" ] && return 0
  done <<EOF
$GATE_COMMANDS
EOF
  return 1
}

# No shell: split the validated string on whitespace and exec the words. `read -ra` is
# bash 3.2-safe; the character-set check above is what makes the split safe, since no
# token can contain a quote, a glob or an operator.
run_cited() {   # run_cited <command-string> <output-file>
  local argv
  read -ra argv <<< "$1"
  "${argv[@]}" > "$2" 2>&1
}


# `mapfile` is bash 4; macOS ships bash 3.2 and every other script here reads with a
# while loop for exactly that reason. A gate that only runs on the maintainer's shell is
# not a gate.
COMMANDS=()
while IFS= read -r line; do
  [ -n "$line" ] && COMMANDS+=("$line")
done < <(python3 scripts/check_doc_evidence.py --list-gate-commands)
if [ "${#COMMANDS[@]}" -eq 0 ]; then
  echo "error: the index cites no gate: evidence at all. Either it lost its gate rows," >&2
  echo "       or --list-gate-commands is broken; both are failures, not a clean run." >&2
  exit 2
fi

echo "=============================================="
echo "gate receipts: ${#COMMANDS[@]} distinct command(s) cited by the feature index"
echo "=============================================="

: > "$OUT/index.tsv"
failures=0
n=0
for cmd in "${COMMANDS[@]}"; do
  n=$((n+1))
  if ! allowed "$cmd"; then
    printf '  %sFAIL%s %-24s refused: a gate: command must be `make <target>`, `cargo build...`\n' \
      "$RED" "$NC" "$cmd"
    printf '       %-24s or `cargo test...`, built only from letters, digits and ._:=-\n' ""
    failures=$((failures+1))
    continue
  fi
  # Indexed, not transliterated: `tr -c 'A-Za-z0-9' '_'` is lossy, so two distinct
  # allowed commands could map to one filename and silently overwrite each other's
  # output — one gate's receipt validating another gate's claim.
  slug="receipt_$(printf '%03d' "$n").out"
  # Combined stream: a gate's verdict banner may go to either, and the receipt is the
  # whole of what it said.
  run_cited "$cmd" "$OUT/$slug"
  rc=$?
  printf '%s\t%s\t%s\n' "$cmd" "$rc" "$slug" >> "$OUT/index.tsv"
  if [ "$rc" -eq 0 ]; then
    printf '  %sok%s   %-24s exit 0, %s line(s) recorded\n' \
      "$GREEN" "$NC" "$cmd" "$(wc -l < "$OUT/$slug" | tr -d ' ')"
  else
    printf '  %sFAIL%s %-24s exit %s -- see %s\n' "$RED" "$NC" "$cmd" "$rc" "$OUT/$slug"
    failures=$((failures+1))
  fi
done

if [ "$failures" -gt 0 ]; then
  echo
  echo "${RED}$failures of $n cited gate(s) failed${NC}. A failing gate is not evidence for"
  echo "anything, so the index is not validated against this run."
  exit 1
fi

echo
echo "validating every gate: result against the output above"
python3 scripts/check_doc_evidence.py --index-only --gate-receipts "$OUT"
rc=$?
echo "=============================================="
if [ "$rc" -eq 0 ]; then
  echo "${GREEN}gate receipts green${NC} -- every gate: outcome in the feature index was"
  echo "printed by the gate it names, in this run."
else
  echo "${RED}gate receipts FAILED${NC}"
fi
echo "=============================================="
exit "$rc"
