#!/usr/bin/env bash
# How much of the doc-evidence gate the controls actually cover, MEASURED.
#
# WHY THIS IS A SCRIPT AND NOT A TABLE IN A COMMIT MESSAGE
# "34 of 34 controls pass" says the controls agree with the code. It does not say the
# controls would NOTICE if the code stopped working — a suite of vacuous assertions passes
# just as cleanly. The number that means something is: with fix X reverted, how many
# controls go red. That number was reported by hand for two rounds and could not be
# reproduced by anyone reading the branch, which is the same "trust the prose" failure this
# whole gate exists to remove. So it is a command.
#
# Each mutation below reverts ONE fix in scripts/check_doc_evidence.py, runs the controls,
# and records how many failed. A mutation that reverts nothing (because the code moved and
# the patch no longer applies) is reported as MUTATION-DEAD rather than as 0 failures —
# a distinction that matters, because a silently non-applying patch would otherwise read as
# "this fix has no coverage" and send someone to write controls that already exist.
#
# The file is restored from git after every mutation and again on exit, including on
# interrupt. It refuses to run against a dirty checker, so a mutation can never be
# committed by accident.
#
# Usage: bash scripts/test-doc-evidence-coverage.sh

set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

TARGET=scripts/check_doc_evidence.py
GREEN=$'\033[0;32m'; RED=$'\033[0;31m'; YEL=$'\033[0;33m'; NC=$'\033[0m'

if ! git diff --quiet -- "$TARGET" || ! git diff --cached --quiet -- "$TARGET"; then
  echo "error: $TARGET has uncommitted changes. This script rewrites it and restores it" >&2
  echo "       from git, which would destroy them. Commit or stash first." >&2
  exit 2
fi

restore() { git checkout -- "$TARGET" 2>/dev/null; }
trap restore EXIT INT TERM

# mutate <name> — edits $TARGET in place. Prints nothing; exits 9 if it changed nothing.
mutate() {
  python3 - "$1" <<'PYEOF'
import pathlib, sys
p = pathlib.Path("scripts/check_doc_evidence.py")
t = orig = p.read_text()
w = sys.argv[1]
if w == "executed":            # run the command at all (the original c199c19 defect)
    t = t.replace("    key = (cmd, want_n == 0)", "    return []\n    key = (cmd, want_n == 0)", 1)
elif w == "l1-path":           # first segment must name a path
    t = t.replace('        if n == 0 and not parsed["paths"]:', "        if False:", 1)
elif w == "l1-pattern-opt":    # pattern may not arrive through an option
    t = t.replace("            if base in GREP_PATTERN_OPTS:", "            if False:", 1)
    t = t.replace("                    if ch in GREP_PATTERN_SHORTS:", "                    if False:", 1)
elif w == "l2-exists":         # a named path must exist
    t = t.replace("    if not p.exists():", "    if False:", 1)
elif w == "l2-symlink":        # a named path may not resolve outside the repo
    t = t.replace("    real = p.resolve()", "    real = p", 1)
elif w == "l3-probe":          # an absence must be shown capable of producing output
    t = t.replace("elif want_n == 0 and (perr := probe_reads_something(segments)):",
                  "elif False and (perr := probe_reads_something(segments)):", 1)
elif w == "l3-find-probe":     # ... and for find, keep traversal, neutralise matching
    t = t.replace('        return [head] + parsed["paths"] + expr',
                  '        return [head] + parsed["paths"]', 1)
elif w == "find-allowlist":    # find's expression is enumerated (-exec, -delete, -not)
    t = t.replace("        err = check_find_expression(opts)", "        err = None", 1)
elif w == "exe-path":          # the tool may not be named by path
    t = t.replace('        if "/" in head:', "        if False:", 1)
elif w == "tool-resolution":   # WHICH BINARY runs: resolved on a pinned PATH
    t = t.replace('_TOOLS[name] = shutil.which(name, path=SAFE_PATH)',
                  '_TOOLS[name] = shutil.which(name)', 1)
elif w == "pinned-env":        # WHAT THE TOOL DOES: the child's environment is pinned
    t = t.replace('    env = {"PATH": SAFE_PATH, "LC_ALL": "C"}',
                  '    env = dict(os.environ, LC_ALL="C")', 1)
elif w == "seg-status":        # every segment's status is a verdict
    t = t.replace("            if isinstance(verdict, gate_probe.Malfunction):", "            if False:", 1)
elif w == "gate-checkable":    # a gate: result must carry something checkable
    t = t.replace("                    if not toks:", "                    if False:", 1)
elif w == "gate-exact":        # gate: key=value compared by value, not containment
    t = t.replace("        elif v not in seen[k]:", "        elif False:", 1)
elif w == "gate-freshness":    # receipts must come from the run validating them
    t = t.replace('    if stamp.read_text(encoding="utf-8").strip() != run_id:', "    if False:", 1)
    t = t.replace("    if not stamp.exists():", "    if False:", 1)
elif w == "dynamic-evidence":  # implemented/partial rows need evidence from a run
    t = t.replace('        if impl in ("implemented", "partial"):', "        if False:", 1)
elif w == "conformance-class": # conformance: class must match the manifest
    t = t.replace("                    elif declared[0] != m.group(2):", "                    elif False:", 1)
else:
    sys.exit(2)
if t == orig:
    sys.exit(9)
p.write_text(t)
PYEOF
}

MUTATIONS="executed l1-path l1-pattern-opt l2-exists l2-symlink l3-probe l3-find-probe
find-allowlist exe-path tool-resolution pinned-env seg-status gate-checkable
gate-exact gate-freshness
dynamic-evidence conformance-class"

echo "=============================================="
echo "control coverage: revert one fix, count the controls that notice"
echo "=============================================="

baseline=$(bash scripts/test-doc-evidence.sh 2>&1 | grep -c 'FAIL')
if [ "$baseline" -ne 0 ]; then
  echo "${RED}ABORT${NC}: the controls are not green to begin with ($baseline failing), so"
  echo "       every number below would be measuring that instead of the mutation."
  exit 1
fi
printf '  %sok%s   unmutated: 0 controls failing\n\n' "$GREEN" "$NC"

dead=0; uncovered=0
for m in $MUTATIONS; do
  mutate "$m"; mrc=$?
  if [ "$mrc" -eq 9 ]; then
    printf '  %sMUTATION-DEAD%s %-18s the patch no longer applies; this row measures NOTHING\n' \
      "$YEL" "$NC" "$m"
    dead=$((dead+1)); restore; continue
  elif [ "$mrc" -ne 0 ]; then
    printf '  %sMUTATION-ERROR%s %-17s mutate() does not know this name\n' "$RED" "$NC" "$m"
    dead=$((dead+1)); restore; continue
  fi
  n=$(bash scripts/test-doc-evidence.sh 2>&1 | grep -c 'FAIL')
  restore
  if [ "$n" -eq 0 ]; then
    printf '  %sUNCOVERED%s     %-18s reverted, and NOT ONE control noticed\n' "$RED" "$NC" "$m"
    uncovered=$((uncovered+1))
  else
    printf '  %sok%s            %-18s %2d control(s) go red\n' "$GREEN" "$NC" "$m" "$n"
  fi
done

echo
echo "=============================================="
if [ "$uncovered" -eq 0 ] && [ "$dead" -eq 0 ]; then
  echo "${GREEN}coverage green${NC} -- every fix has at least one control that fails when it is"
  echo "reverted, and every mutation still applies."
  echo "=============================================="
  exit 0
fi
echo "${RED}coverage FAILED${NC} -- $uncovered fix(es) with no control, $dead dead mutation(s)"
echo "=============================================="
exit 1
