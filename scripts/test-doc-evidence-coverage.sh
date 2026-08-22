#!/usr/bin/env bash
# Whether the doc-evidence controls would NOTICE if the gate stopped working.
#
# WHAT THIS ESTABLISHES, EXACTLY -- and nothing wider:
#   1. every control case the probe emits has a DECLARED ROLE in
#      scripts/doc-evidence-controls.tsv, reconciled both ways against the labels the probe
#      prints at runtime, so a control cannot be deleted, renamed or silently skipped;
#   2. every `kill` control is actually killed by the mutation it names -- checked BY NAME,
#      so a mutation that reddens three unrelated cases and leaves its own passing is
#      WRONG-CONTROL rather than coverage;
#   3. every mutation still applies (MUTATION-DEAD otherwise) and every file declared
#      `mutated` has at least one.
#
# WHAT IT DOES NOT ESTABLISH. It does not establish that "every fix has a control". A fix
# has no machine-readable identity: a new repair inside an already-inventoried file changes
# no file set and needs no new mutation, so a fix can be entirely absent from the table
# while this runs green. That was claimed for two rounds and was false; the unit that IS
# authoritative is the control, so the control is what is now closed over. Adding a fix
# without a control is caught by review of the two inventories, not by this script.
#
# WHY TWO INVENTORIES.
#   doc-evidence-controls.tsv  closes over the CONTROLS -- runtime-derived, works anywhere.
#   doc-evidence-fixes.tsv     closes over the FILES this branch changed. Its denominator
#                              is a git diff, which is meaningful on a feature branch and
#                              meaningless once merged: on main the merge-base with main is
#                              HEAD, so the diff is empty, and a shallow CI checkout has no
#                              main ref at all. That reconciliation is therefore reported as
#                              NOT APPLICABLE, with the reason, when no base can be
#                              established -- it is a branch-time check, and the two above
#                              are what carry the claim everywhere.
#
# Usage: bash scripts/test-doc-evidence-coverage.sh
#        COVERAGE_BASE=<rev> bash scripts/test-doc-evidence-coverage.sh   # force the base

set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

INVENTORY=scripts/doc-evidence-fixes.tsv
GREEN=$'\033[0;32m'; RED=$'\033[0;31m'; YEL=$'\033[0;33m'; NC=$'\033[0m'

# Every file a mutation may touch. Restored from git after each one, and on exit.
MUTABLE="scripts/check_doc_evidence.py scripts/gate-receipts.sh .github/workflows/preview.yml"

for f in $MUTABLE; do
  if ! git diff --quiet -- "$f" || ! git diff --cached --quiet -- "$f"; then
    echo "error: $f has uncommitted changes. This script rewrites it and restores it" >&2
    echo "       from git, which would destroy them. Commit or stash first." >&2
    exit 2
  fi
done

restore() { git checkout -- $MUTABLE 2>/dev/null; }
trap restore EXIT INT TERM

# --- the file-set reconciliation: branch-time, and honest about when it is not ---------
CONTROLS=scripts/doc-evidence-controls.tsv
[ -f "$INVENTORY" ] || { echo "error: $INVENTORY missing; it IS the file denominator." >&2; exit 2; }
[ -f "$CONTROLS" ]  || { echo "error: $CONTROLS missing; it IS the control denominator." >&2; exit 2; }

# In order: an explicit base; the first parent when HEAD is a merge (post-merge on main,
# where `merge-base main HEAD` is HEAD and the diff would be empty); the merge-base with
# main or origin/main on a feature branch. If none resolves -- a shallow CI checkout has
# no main ref and no HEAD^ -- the file reconciliation is NOT APPLICABLE and says so.
BASE=""
if [ -n "${COVERAGE_BASE:-}" ]; then
  BASE=$(git rev-parse --verify "${COVERAGE_BASE}^{commit}" 2>/dev/null)
elif [ "$(git rev-list --parents -n 1 HEAD 2>/dev/null | wc -w | tr -d ' ')" -ge 3 ]; then
  BASE=$(git rev-parse --verify 'HEAD^1' 2>/dev/null)      # merge commit: what it merged
else
  for ref in main origin/main; do
    if git rev-parse --verify "$ref" >/dev/null 2>&1; then
      BASE=$(git merge-base "$ref" HEAD 2>/dev/null); [ -n "$BASE" ] && break
    fi
  done
fi
CHANGED=""
[ -n "$BASE" ] && CHANGED=$(git diff --name-only "$BASE"..HEAD)

echo "=============================================="
if [ -z "$CHANGED" ]; then
  echo "file-set reconciliation: NOT APPLICABLE"
  if [ -z "$BASE" ]; then
    echo "  no base commit resolves (no main/origin/main ref, HEAD is not a merge, and"
    echo "  COVERAGE_BASE is unset) -- a shallow checkout looks like this."
  else
    echo "  base $(git rev-parse --short "$BASE") is HEAD's own tree, so the diff is empty;"
    echo "  after a merge there is no branch left to diff. This is a branch-time check."
  fi
  echo "  The control closure below is what carries the claim, and it runs anywhere."
else
  echo "file-set reconciliation: $(printf '%s\n' "$CHANGED" | wc -l | tr -d ' ') file(s) changed since $(git rev-parse --short "$BASE"),"
  echo "reconciled against $INVENTORY in both directions"
fi
echo "=============================================="

# The disposition vocabulary is CLOSED. Only the literal `mutated` had any enforcement, so
# `content`, `is-the-detector`, an invented value or a TYPO all bypassed the mutation
# requirement -- measured, misspelling one row as `mutatd` dropped a file out of coverage
# and the run stayed green. tests/conformance-manifest.txt validates its class column for
# exactly this reason.
recon=0
while IFS=$'\t' read -r path disp _; do
  case "$path" in ''|'#'*) continue ;; esac
  case "$disp" in
    mutated|is-the-detector|content) ;;
    *) printf '  %sBAD-DISPOSITION%s %s -- %s is not one of: mutated, is-the-detector, content\n' \
         "$RED" "$NC" "$path" "'$disp'"; recon=$((recon+1)) ;;
  esac
done < "$INVENTORY"

if [ -n "$CHANGED" ]; then
  while IFS= read -r f; do
    [ -z "$f" ] && continue
    if ! awk -F'\t' -v p="$f" '$1==p {found=1} END {exit !found}' "$INVENTORY"; then
      printf '  %sUNDECLARED%s    %s -- changed by this branch, absent from the inventory\n' \
        "$RED" "$NC" "$f"
      recon=$((recon+1))
    fi
  done <<EOF2
$CHANGED
EOF2
  while IFS=$'\t' read -r path disp _; do
    case "$path" in ''|'#'*) continue ;; esac
    if ! printf '%s\n' "$CHANGED" | grep -qxF -- "$path"; then
      printf '  %sMISSING%s       %s -- declared %s, but this branch did not change it\n' \
        "$RED" "$NC" "$path" "$disp"
      recon=$((recon+1))
    fi
  done < "$INVENTORY"
fi

# --- control closure: runtime labels vs declared roles, both directions -----------------
LIVE=$(bash scripts/test-doc-evidence.sh 2>&1 | sed 's/\x1b\[[0-9;]*m//g' \
       | grep -E '^  (ok   |FAIL )' | sed 's/^  ok   //; s/^  FAIL //' | sed 's/[[:space:]]*$//')
if [ -z "$LIVE" ]; then
  echo "error: the probe emitted no control cases at all; nothing to reconcile." >&2
  exit 2
fi
while IFS= read -r c; do
  [ -z "$c" ] && continue
  if ! awk -F'\t' -v p="$c" '$1==p {f=1} END {exit !f}' "$CONTROLS"; then
    printf '  %sUNDECLARED-CONTROL%s %s\n' "$RED" "$NC" "$c"; recon=$((recon+1))
  fi
done <<EOF2
$LIVE
EOF2
while IFS=$'\t' read -r label role _; do
  case "$label" in ''|'#'*) continue ;; esac
  if ! printf '%s\n' "$LIVE" | grep -qxF -- "$label"; then
    printf '  %sMISSING-CONTROL%s    %s -- declared %s, but the probe no longer emits it\n' \
      "$RED" "$NC" "$label" "$role"
    recon=$((recon+1))
  fi
done < "$CONTROLS"

if [ "$recon" -gt 0 ]; then
  echo
  echo "${RED}inventories do not reconcile${NC} -- $recon discrepancy(ies). Coverage measured"
  echo "over an inventory that disagrees with what actually runs is coverage over a guess."
  exit 1
fi
printf '  %sok%s            %s control case(s) emitted, every one with a declared role\n' \
  "$GREEN" "$NC" "$(printf '%s\n' "$LIVE" | wc -l | tr -d ' ')"
[ -n "$CHANGED" ] && printf '  %sok%s            file inventory reconciles with git in both directions\n' "$GREEN" "$NC"

MUTATED_FILES=$(awk -F'\t' '$2=="mutated" {print $1}' "$INVENTORY")

# --- mutations --------------------------------------------------------------------------
# mutate <name> -> edits one file in place. Exits 9 if it changed nothing.
mutate() {
  python3 - "$1" <<'PYEOF'
import pathlib, sys
w = sys.argv[1]
FILES = {"py": "scripts/check_doc_evidence.py", "gr": "scripts/gate-receipts.sh",
         "mk": "Makefile", "ci": ".github/workflows/preview.yml"}
which = {"gate-argv-grammar": "gr", "gate-private-receipts": "gr",
         "ci-steps": "ci"}.get(w, "py")
p = pathlib.Path(FILES[which])
t = orig = p.read_text()

if w == "executed":            # run the command at all (the original c199c19 defect)
    t = t.replace("    key = (cmd, want_n == 0)", "    return []\n    key = (cmd, want_n == 0)", 1)
elif w == "l1-path":           # first segment must name a path
    t = t.replace('        if n == 0 and not parsed["paths"]:', "        if False:", 1)
elif w == "l1-pattern-opt":    # the pattern may not arrive through an option
    t = t.replace("            if base in GREP_PATTERN_OPTS:", "            if False:", 1)
    t = t.replace("                    if ch in GREP_PATTERN_SHORTS:", "                    if False:", 1)
elif w == "l2-exists":         # a named path must exist
    t = t.replace("    if not p.exists():", "    if False:", 1)
elif w == "l2-symlink":        # a named path may not resolve outside the repo
    t = t.replace("    real = p.resolve()", "    real = p", 1)
elif w == "l3-probe":          # an absence must be shown capable of producing output
    t = t.replace("elif want_n == 0 and (perr := probe_reads_something(segments)):",
                  "elif False and (perr := probe_reads_something(segments)):", 1)
elif w == "l3-find-probe":     # find: keep the traversal bound in the probe
    t = t.replace('        return [head] + parsed["paths"] + expr',
                  '        return [head] + parsed["paths"]', 1)
elif w == "find-allowlist":    # find's expression is enumerated (-exec, -delete, -not)
    t = t.replace("        err = check_find_expression(opts)", "        err = None", 1)
elif w == "find-grammar":      # the permitted expression: <traversal>* <match>?
    t = t.replace("        err = check_find_expression(opts)\n        if err:", "        err = None\n        if err:", 1)
elif w == "exe-path":          # the tool may not be named by path
    t = t.replace('        if "/" in head:', "        if False:", 1)
elif w == "tool-resolution":   # WHICH BINARY runs: resolved on a pinned PATH
    t = t.replace("_TOOLS[name] = shutil.which(name, path=SAFE_PATH)",
                  "_TOOLS[name] = shutil.which(name)", 1)
elif w == "pinned-env":        # WHAT THE TOOL DOES: the child's environment is pinned
    t = t.replace('    env = {"PATH": SAFE_PATH, "LC_ALL": "C"}',
                  '    env = dict(os.environ, LC_ALL="C")', 1)
elif w == "seg-status":        # every segment's status is a verdict
    t = t.replace("            if isinstance(verdict, gate_probe.Malfunction):", "            if False:", 1)
elif w == "gate-checkable":    # a gate: result must carry something checkable
    t = t.replace("                    if not toks:", "                    if False:", 1)
elif w == "gate-exact":        # gate: key=value must be the run's ONLY value for that key
    t = t.replace("        elif seen[k] != {v}:", "        elif v not in seen[k]:", 1)
elif w == "gate-receipts-in-repo":   # a receipts directory may not be repo content
    t = t.replace("    if real == ROOT or ROOT in real.parents:", "    if False:", 1)
elif w == "dynamic-evidence":  # implemented/partial rows need evidence from a run
    t = t.replace('        if impl in ("implemented", "partial"):', "        if False:", 1)
elif w == "conformance-class": # conformance: class must match the manifest
    t = t.replace("                    elif declared[0] != m.group(2):", "                    elif False:", 1)

elif w == "gate-argv-grammar":       # back to a prefix that accepted trailing words
    t = t.replace('  local argv\n  read -ra argv <<< "$1"\n  case "${#argv[@]}:${argv[0]:-}" in',
                  '  case "$1" in "make "*|"cargo "*) return 0 ;; esac\n'
                  '  local argv\n  read -ra argv <<< "$1"\n  case "${#argv[@]}:${argv[0]:-}" in', 1)
elif w == "gate-private-receipts":   # back to a shared, surviving receipts directory
    t = t.replace('OUT=$(mktemp -d "${TMPDIR:-/tmp}/palladium-gate-receipts.XXXXXX") || exit 2\n'
                  'trap \'rm -rf "$OUT"\' EXIT INT TERM',
                  'OUT=build_output/gate-receipts\nrm -rf "$OUT"; mkdir -p "$OUT"', 1)
elif w == "ci-steps":          # the workflow must invoke the evidence gate at all
    t = t.replace("      - name: Documentation evidence\n        run: bash scripts/check-doc-evidence.sh\n", "", 1)
else:
    sys.exit(2)
if t == orig:
    sys.exit(9)
p.write_text(t)
PYEOF
}

# name | file | THE CONTROL CASE THIS FIX EXISTS FOR.
# The third field is what turns a count into an identity: a mutation that reddens
# three unrelated cases and leaves its own passing is not covered, and counting
# alone reported it as covered. Fragments are verbatim from the case labels and
# contain no backticks, so they survive the shell that reads this table.
MUTATIONS="executed|scripts/check_doc_evidence.py|a false LINE COUNT is rejected
l1-path|scripts/check_doc_evidence.py|a command naming NO path is refused
l1-pattern-opt|scripts/check_doc_evidence.py|an option-supplied pattern
l2-exists|scripts/check_doc_evidence.py|an absence over a MISSING PATH is rejected
l2-symlink|scripts/check_doc_evidence.py|a symlink resolving outside the repository is refused
l3-probe|scripts/check_doc_evidence.py|an absence over an EMPTY scope is refused
l3-find-probe|scripts/check_doc_evidence.py|an absence over an empty scope is refused for FIND too
find-allowlist|scripts/check_doc_evidence.py|find -exec is refused
find-grammar|scripts/check_doc_evidence.py|a find disjunction is refused outright, not reduced
exe-path|scripts/check_doc_evidence.py|an executable named by PATH is refused
tool-resolution|scripts/check_doc_evidence.py|a hijacked PATH does not change WHICH BINARY runs
pinned-env|scripts/check_doc_evidence.py|an inherited GREP_OPTIONS does not change WHAT THE TOOL DOES
seg-status|scripts/check_doc_evidence.py|a real SIGPIPE from a downstream grep -q is caught
gate-checkable|scripts/check_doc_evidence.py|a gate: result with no checkable token is refused
gate-exact|scripts/check_doc_evidence.py|a self-contradicting run endorses NEITHER value
gate-receipts-in-repo|scripts/check_doc_evidence.py|a receipts directory inside the repository is refused
dynamic-evidence|scripts/check_doc_evidence.py|only STATIC evidence is refused
conformance-class|scripts/check_doc_evidence.py|a class the manifest disagrees with is rejected
gate-argv-grammar|scripts/gate-receipts.sh|make conformance CC=clang
gate-private-receipts|scripts/gate-receipts.sh|receipts are invocation-private and removed on exit
ci-steps|.github/workflows/preview.yml|CI runs scripts/check-doc-evidence.sh"

# A failing CASE line, not any line containing the word: a summary line or a quoted
# diagnostic would otherwise inflate the count — the same membership-versus-agreement
# slackness that let `pinned-path` look covered before it was split in two.
count_failing() { bash scripts/test-doc-evidence.sh 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep -c '^  FAIL'; }

echo
baseline=$(count_failing)
if [ "$baseline" -ne 0 ]; then
  echo "${RED}ABORT${NC}: the controls are not green to begin with ($baseline failing), so"
  echo "       every number below would be measuring that instead of the mutation."
  exit 1
fi
printf '  %sok%s            unmutated: 0 controls failing\n\n' "$GREEN" "$NC"

dead=0; uncovered=0; ran=0; COVERED_FILES=""
while IFS= read -r entry; do
  [ -z "$entry" ] && continue
  m=${entry%%|*}; rest=${entry#*|}; mf=${rest%%|*}; want=${rest#*|}
  mutate "$m"; mrc=$?
  if [ "$mrc" -eq 9 ]; then
    printf '  %sMUTATION-DEAD%s %-22s the patch no longer applies; this row measures NOTHING\n' \
      "$YEL" "$NC" "$m"
    dead=$((dead+1)); restore; continue
  elif [ "$mrc" -ne 0 ]; then
    printf '  %sMUTATION-ERROR%s %-21s mutate() does not know this name\n' "$RED" "$NC" "$m"
    dead=$((dead+1)); restore; continue
  fi
  ran=$((ran+1)); COVERED_FILES="$COVERED_FILES $mf"
  failing=$(bash scripts/test-doc-evidence.sh 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep '^  FAIL')
  if [ -z "$failing" ]; then n=0; else n=$(printf '%s\n' "$failing" | wc -l | tr -d ' '); fi
  restore
  if [ "$n" -eq 0 ]; then
    printf '  %sUNCOVERED%s     %-22s reverted, and NOT ONE control noticed\n' "$RED" "$NC" "$m"
    uncovered=$((uncovered+1))
  elif ! printf '%s\n' "$failing" | grep -qF -- "$want"; then
    # Something went red, but not the control written for THIS fix. Counting alone would
    # have reported coverage; naming the expected case is what tells an intended kill
    # from collateral damage — the same membership-versus-agreement exactness as the
    # key=value comparison, and what let `pinned-path` look covered before it was split.
    printf '  %sWRONG-CONTROL%s %-22s %2d red, none matching: %s\n' \
      "$RED" "$NC" "$m" "$n" "$want"
    uncovered=$((uncovered+1))
  else
    printf '  %sok%s            %-22s %2d red, incl: %s\n' "$GREEN" "$NC" "$m" "$n" "$want"
  fi
done <<EOF
$MUTATIONS
EOF

# Every mutation named as a `kill` role must actually be in the table above; otherwise a
# control declares itself covered by something that never runs.
while IFS=$'\t' read -r label role detail; do
  case "$label" in ''|'#'*) continue ;; esac
  [ "$role" = kill ] || continue
  if ! printf '%s\n' "$MUTATIONS" | grep -q "^$detail|"; then
    printf '  %sPHANTOM-KILL%s  %-22s no mutation by that name exists (declared by: %s)\n' \
      "$RED" "$NC" "$detail" "$label"
    uncovered=$((uncovered+1))
  fi
done < "$CONTROLS"

# A file declared `mutated` that nothing mutates is the omission this reconciliation
# exists to surface, and it is exactly what happened to four files last round.
for f in $MUTATED_FILES; do
  case " $COVERED_FILES " in
    *" $f "*) ;;
    *) printf '  %sNO-MUTATION%s   %-22s declared mutated, but nothing reverts anything in it\n' \
         "$RED" "$NC" "$f"; uncovered=$((uncovered+1)) ;;
  esac
done

echo
echo "=============================================="
echo "MUTATED (fixes reverted, controls required to notice):"
printf '%s\n' $MUTATED_FILES | sed 's/^/  /'
echo "NOT MUTATED, and why -- every one declared in $INVENTORY:"
awk -F'\t' '$1!~/^#/ && NF>=2 && $2!="mutated" {printf "  %-46s %s\n", $1, $2}' "$INVENTORY"
if [ "$uncovered" -eq 0 ] && [ "$dead" -eq 0 ]; then
  echo "${GREEN}coverage green${NC} -- $ran mutation(s) across $(printf '%s\n' $MUTATED_FILES | wc -l | tr -d ' ') file(s), every one noticed by"
  echo "at least one control, and the inventory reconciles with git."
  echo "=============================================="
  exit 0
fi
echo "${RED}coverage FAILED${NC} -- $uncovered uncovered, $dead dead mutation(s)"
echo "=============================================="
exit 1
