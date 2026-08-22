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
# Receipts now live in a private `mktemp -d` that is REMOVED WHEN THIS SCRIPT EXITS, on
# every path including a failure or an interrupt. There is nothing to replay because
# nothing outlives the run, and the checker additionally refuses a receipts directory
# inside the repository, so one cannot be committed and pointed at. The same change
# removes a race: every invocation used to share build_output/gate-receipts, so two
# concurrent runs would tear each other's RUN_ID, truncate each other's receipts and
# interleave index.tsv — one failing spuriously while the other validated mixed files.
#
# Usage: bash scripts/gate-receipts.sh

set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

OUT=$(mktemp -d "${TMPDIR:-/tmp}/palladium-gate-receipts.XXXXXX") || exit 2
trap 'rm -rf "$OUT"' EXIT INT TERM
GREEN=$'\033[0;32m'; RED=$'\033[0;31m'; NC=$'\033[0m'

# A cited command is A STRING FROM A DOCUMENT, and it used to be checked by PREFIX and
# then handed to `eval`. `make ` and `cargo test` are prefixes, so anything at all could
# follow them — an operator, a substitution, a redirection — and the document controlled
# the shell. That is the same defect the `cmd:` executor exists to prevent, in the file
# that validates it.
#
# Now: every character must come from a set that contains no shell syntax, and the WORDS
# must match an enumerated argv grammar. The intermediate version checked a PREFIX, which
# is not a shape — `make `* accepted `make conformance CC=clang`, `make conformance -j 8`
# and `make conformance --always-make`, each of which changes what the cited gate means
# while still being spelled like the gate the document names.
#
# Adding a gate shape is a deliberate edit to this list, which is the point.
allowed() {
  case "$1" in
    *[!A-Za-z0-9\ ._:=-]*) return 1 ;;   # anything outside this set, metacharacters included
  esac
  local argv
  read -ra argv <<< "$1"
  case "${#argv[@]}:${argv[0]:-}" in
    2:make)
      # exactly one target, no options, no variable assignments
      case "${argv[1]}" in
        [a-z]*[!A-Za-z0-9-]*) return 1 ;;
        [a-z]*) return 0 ;;
      esac
      return 1 ;;
    3:cargo)
      [ "${argv[1]}" = build ] && [ "${argv[2]}" = --release ] && return 0
      return 1 ;;
    5:cargo)
      [ "${argv[1]}" = test ] || return 1
      [ "${argv[2]}" = --release ] || return 1
      [ "${argv[3]}" = --lib ] || return 1
      case "${argv[4]}" in
        *[!A-Za-z0-9_:]*) return 1 ;;
        ?*) return 0 ;;
      esac
      return 1 ;;
  esac
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
