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
# STALENESS. The receipts directory is deleted and rebuilt on every invocation, and it is
# only ever read by the invocation that wrote it — the checker is called from here, with
# this run's directory. A leftover directory cannot be picked up as evidence, because
# nothing else passes --gate-receipts.
#
# Usage: bash scripts/gate-receipts.sh

set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

OUT=build_output/gate-receipts
GREEN=$'\033[0;32m'; RED=$'\033[0;31m'; NC=$'\033[0m'

# A cited command must be one this file is willing to run. The index can name any Make
# target, and `make` is how a gate is spelled here, but an arbitrary shell string arriving
# from a document and being executed is not something a gate should do quietly.
allowed() {
  case "$1" in
    "make "*|"cargo build"*|"cargo test"*) return 0 ;;
    *) return 1 ;;
  esac
}

rm -rf "$OUT" || exit 2
mkdir -p "$OUT" || exit 2

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
    printf '  %sFAIL%s %-24s refused: a gate: command must be `make <target>` or `cargo build`\n' \
      "$RED" "$NC" "$cmd"
    failures=$((failures+1))
    continue
  fi
  slug="receipt_$(printf '%s' "$cmd" | tr -c 'A-Za-z0-9' '_').out"
  # Combined stream: a gate's verdict banner may go to either, and the receipt is the
  # whole of what it said.
  eval "$cmd" > "$OUT/$slug" 2>&1
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
