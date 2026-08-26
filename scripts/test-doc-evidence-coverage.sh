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
# no file set and needs no new mutation, so a fix can be entirely absent from both tables
# while this runs green. That was claimed for two rounds and was false; the unit that IS
# authoritative is the control, so the control is what is closed over.
#
# THE RESIDUAL, AND IT IS NOT CLOSED BY ANYTHING HERE. Reviewing the two inventories does
# not close it: neither carries an identity for a fix, so a repair added inside an
# already-declared file leaves both unchanged and both reconciling. Reviewing THE BRANCH
# DIFF against the two tables is the mitigation — for each behavioural change, is there a
# mutation reverting it and a control that dies — and it is a FALLIBLE MANUAL one. It is
# performed by a person who can miss a hunk, and its own test ("a mutation exists and a
# control dies") is the same weak predicate this file has now been caught by twice: once
# as ∃-per-mutation instead of ∀-over-rows, and once as a control dying without dying for
# the reason it names. So the honest statement is that the gap is MITIGATED, not closed,
# and the mitigation shares a failure mode with the mechanism it is mitigating.
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
CONTROLS=scripts/doc-evidence-controls.tsv
GREEN=$'\033[0;32m'; RED=$'\033[0;31m'; YEL=$'\033[0;33m'; NC=$'\033[0m'

# WHEN scripts/doc-evidence-fixes.tsv APPLIES -- ONE SENTENCE, AND THE CODE IS BELOW IT:
#
#   It applies exactly when the tree being measured is the tree it describes, which is to
#   say when HEAD's merge-base with main equals the `# inventory-base:` recorded in it.
#
# Two previous rules were both predicates answering a NEARBY question. "An explicit base
# means branch review" made CI treat every push to main as a branch, so the first push
# after this branch landed would have failed with fatal MISSING. "HEAD is not reachable
# from main" means HEAD is on A branch, while this inventory describes THE branch -- so
# every future feature, release and integration branch would have been reconciled against
# these rows and failed for all of them. Measured before the fix: a fresh branch cut from
# a fresh main reported `reconciliation=branch files=1`.
#
# Tying it to the recorded base also gives the property that makes this safe to merge: the
# inventory STOPS BEING A GATE the moment it stops describing the tree.
#
# APPLICABILITY IS DECIDED FIRST, BEFORE ANY BASE IS LOOKED AT. Where it does not apply
# the reconciliation does not run, and supplying COVERAGE_BASE there is an ERROR rather
# than a way in -- previously the base was validated first, so an unresolvable one exited
# 2 even on main, contradicting this file's own sentence in the commit that wrote it.
INV_BASE=$(sed -n 's/^# inventory-base:[[:space:]]*//p' "$INVENTORY" | head -1)
APPLIES=no; WHY=""
if [ -z "$INV_BASE" ]; then
  WHY="no-inventory-base"
else
  _mb=""
  for _ref in main origin/main; do
    git rev-parse --verify "$_ref" >/dev/null 2>&1 || continue
    _mb=$(git merge-base "$_ref" HEAD 2>/dev/null) && [ -n "$_mb" ] && break
  done
  if [ -z "$_mb" ]; then
    WHY="no-main-ref"
  elif [ "$_mb" = "$(git rev-parse HEAD 2>/dev/null)" ]; then
    WHY="head-is-on-main"
  elif [ "$_mb" = "$INV_BASE" ]; then
    APPLIES=yes
  else
    WHY="different-branch"
  fi
fi

BASE=""; BASE_WHY=""
if [ "$APPLIES" = yes ]; then
  if [ -n "${COVERAGE_BASE:-}" ]; then
    BASE=$(git rev-parse --verify "${COVERAGE_BASE}^{commit}" 2>/dev/null) || BASE=""
    if [ -z "$BASE" ]; then
      echo "error: COVERAGE_BASE='${COVERAGE_BASE}' does not resolve to a commit." >&2
      exit 2
    fi
    if ! git merge-base --is-ancestor "$BASE" HEAD 2>/dev/null; then
      echo "error: COVERAGE_BASE='${COVERAGE_BASE}' is not an ancestor of HEAD, so the" >&2
      echo "       diff between them is not 'what this branch changed'." >&2
      exit 2
    fi
    if [ "$BASE" = "$(git rev-parse HEAD)" ]; then
      echo "error: COVERAGE_BASE='${COVERAGE_BASE}' IS HEAD, so the reconciliation would" >&2
      echo "       be over an empty file set. A base was requested; an empty answer is" >&2
      echo "       not one." >&2
      exit 2
    fi
    BASE_WHY="COVERAGE_BASE"
  else
    BASE=$INV_BASE; BASE_WHY="inventory-base"
  fi
elif [ -n "${COVERAGE_BASE:-}" ]; then
  echo "error: COVERAGE_BASE was supplied, but $INVENTORY does not describe this tree" >&2
  echo "       ($WHY). A base cannot make an inventory applicable; it only chooses where" >&2
  echo "       an applicable one starts." >&2
  exit 2
fi

CHANGED=""
if [ "$APPLIES" = yes ] && [ -n "$BASE" ]; then
  CHANGED=$(git diff --name-only "$BASE"..HEAD)
fi

# A mode for the controls: resolve the scope, say what was decided, and stop. It runs
# BEFORE the restore trap below is installed, because it mutates nothing and an EXIT trap
# that reverts MUTABLE files would otherwise destroy a working tree's uncommitted edits --
# measured, it ate one of this file's own.
if [ "${1:-}" = "--explain-base" ]; then
  if [ "$APPLIES" = yes ] && [ -n "$CHANGED" ]; then
    echo "reconciliation=branch base=$(git rev-parse --short "$BASE") why=$BASE_WHY files=$(printf '%s\n' "$CHANGED" | wc -l | tr -d ' ')"
  elif [ "$APPLIES" = yes ]; then
    echo "reconciliation=not-applicable why=no-diff"
  else
    echo "reconciliation=not-applicable why=$WHY"
  fi
  exit 0
fi

# Every file a mutation may touch. Restored from a startup SNAPSHOT after each one and
# on exit -- not from git; see the note below the dirty-tree guard.
MUTABLE="scripts/check_doc_evidence.py scripts/gate-receipts.sh .github/workflows/preview.yml Makefile scripts/test-doc-evidence-coverage.sh scripts/test-doc-evidence.sh"

for f in $MUTABLE; do
  [ "${1:-}" = --explain-base ] && break   # resolves a scope and exits; mutates nothing
  if ! git diff --quiet -- "$f" || ! git diff --cached --quiet -- "$f"; then
    echo "error: $f has uncommitted changes. This script rewrites it while measuring" >&2
    echo "       and puts back what it found, so they would survive -- but mutating a" >&2
    echo "       file nobody has reviewed measures something nobody has reviewed." >&2
    echo "       Commit or stash first." >&2
    exit 2
  fi
done

TMP_KILL=$(mktemp "${TMPDIR:-/tmp}/doc-evidence-killsets.XXXXXX") || exit 2

# RESTORE FROM A SNAPSHOT, NEVER FROM git.
#
# This used to be `git checkout -- $MUTABLE`, which discards UNCOMMITTED changes to every
# file in that list. That is not a theoretical hazard: a fix to
# .github/workflows/preview.yml was reported as applied three times and was not there any
# of them.
#
# ONE SHARED FILESYSTEM MECHANISM, PLUS A REPEATED VERIFICATION FAILURE. Both were needed
# every time. The mechanism: this script's EXIT trap reverting an uncommitted edit. The
# failure: reporting intended work without reading the committed state back off disk. The
# first is fixed below; the second is a habit, and the only thing that catches it is
# looking at the file instead of at what you meant to write. (That the trap erased each
# specific edit is my reading of the mechanism and the outcome -- the pre- and post-run
# working trees are gone, so it is not something I can show.)
#
# The rule that generalises past the instance: A TOOL THAT REWRITES FILES TO MEASURE THEM
# MUST PUT BACK WHAT IT FOUND, NOT WHAT SOME OTHER SOURCE SAYS SHOULD BE THERE. Restoring
# from git makes the tool's idea of "unchanged" differ from the working tree's, and the
# difference is silently destructive.
#
# WHAT THE SNAPSHOT ESTABLISHES, AND WHAT IT DOES NOT. It establishes that the CONTENT of
# a file that existed and was readable at startup is put back after each mutation and on
# exit, so an uncommitted edit to a MUTABLE file survives a run. It does NOT make this
# script incapable of destroying anything:
#   * absence is not preserved -- a path created during the run is not removed;
#   * a snapshotted file that is DELETED comes back as a plain regular file, so a symlink
#     is not restored as a symlink and its target is not restored at all;
#   * `cp -p` preserves mode and timestamps, not ownership, ACLs or xattrs;
#   * an edit made concurrently by someone else during the run is overwritten, not merged;
#   * SIGKILL and power loss run no trap, so the last mutation stays on disk;
#   * restore errors are suppressed, so a failure to put a file back is silent.
# The dirty-tree guard below therefore stays: it is not the safety belt any more, but it
# keeps the script from measuring a file nobody has reviewed.
SNAP=$(mktemp -d "${TMPDIR:-/tmp}/doc-evidence-snapshot.XXXXXX") || exit 2
snap_of() { printf '%s/%s' "$SNAP" "$(printf '%s' "$1" | tr -c 'A-Za-z0-9' '_')"; }
for _f in $MUTABLE; do
  [ -f "$_f" ] || continue
  cp -p "$_f" "$(snap_of "$_f")" || { echo "error: cannot snapshot $_f" >&2; exit 2; }
done
restore() {
  local f
  for f in $MUTABLE; do
    [ -f "$(snap_of "$f")" ] || continue
    cp -p "$(snap_of "$f")" "$f" 2>/dev/null
  done
}
trap 'restore; rm -f "$TMP_KILL"; rm -rf "$SNAP"' EXIT INT TERM

echo "=============================================="
if [ -z "$CHANGED" ]; then
  echo "file-set reconciliation: NOT APPLICABLE"
  if [ "$APPLIES" != yes ]; then
    echo "  $INVENTORY does not describe this tree ($WHY). It records the base it was"
    echo "  built against and applies only where HEAD's merge-base with main is that"
    echo "  base, so it is not a gate on main and not a gate on any other branch. The"
    echo "  control closure below is the gate here, and it runs everywhere."
  elif [ -z "$BASE" ]; then
    echo "  no base commit resolves (no main/origin/main ref, HEAD is not a merge, and"
    echo "  COVERAGE_BASE is unset) -- a shallow checkout looks like this."
  else
    echo "  base $(git rev-parse --short "$BASE") is HEAD's own tree, so the diff is empty;"
    echo "  after a merge there is no branch left to diff. This is a branch-time check."
  fi
  echo "  The control closure below is what carries the claim, and it runs anywhere."
else
  echo "file-set reconciliation: $(printf '%s\n' "$CHANGED" | wc -l | tr -d ' ') file(s) changed since $(git rev-parse --short "$BASE") ($BASE_WHY),"
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
  # Both directions are fatal, because this block only runs where the question is
  # meaningful. An earlier version made MISSING advisory off-branch, which is the shape of
  # a check that cannot fail: it ran everywhere and meant something in one place.
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
  # The role vocabulary is CLOSED, for the reason the disposition column was closed one
  # round ago: `kil` is not `kill`, and an unvalidated column means a typo silently
  # removes a control from every check that follows.
  case "$role" in
    kill|guard) ;;
    *) printf '  %sBAD-ROLE%s      %s -- %s is not one of: kill, guard\n' \
         "$RED" "$NC" "$label" "'$role'"; recon=$((recon+1)) ;;
  esac
  if ! printf '%s\n' "$LIVE" | grep -qxF -- "$label"; then
    printf '  %sMISSING-CONTROL%s    %s -- declared %s, but the probe no longer emits it\n' \
      "$RED" "$NC" "$label" "$role"
    recon=$((recon+1))
  fi
done < "$CONTROLS"

# MEMBERSHIP IS NOT A BIJECTION. Both sides were compared with `grep -qxF`, so two
# controls sharing a label, or two rows declaring one, reconciled cleanly while one of
# each pair went unchecked. The same ∃/∀ slackness as the kill loop below, one table over.
dup_live=$(printf '%s\n' "$LIVE" | sort | uniq -d)
if [ -n "$dup_live" ]; then
  printf '  %sDUPLICATE-CONTROL%s the probe emits these labels more than once:\n' "$RED" "$NC"
  printf '%s\n' "$dup_live" | sed 's/^/       /'
  recon=$((recon+1))
fi
dup_decl=$(awk -F'\t' '$1!~/^#/ && NF>=2 {print $1}' "$CONTROLS" | sort | uniq -d)
if [ -n "$dup_decl" ]; then
  printf '  %sDUPLICATE-ROW%s     %s declares these labels more than once:\n' "$RED" "$NC" "$CONTROLS"
  printf '%s\n' "$dup_decl" | sed 's/^/       /'
  recon=$((recon+1))
fi

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
         "mk": "Makefile", "ci": ".github/workflows/preview.yml",
         "cov": "scripts/test-doc-evidence-coverage.sh",
         "probe": "scripts/test-doc-evidence.sh"}
which = {"gate-argv-grammar": "gr", "gate-private-receipts": "gr",
         "ci-no-base": "ci", "ci-step-evidence": "ci", "ci-step-receipts": "ci", "ci-step-probe": "ci",
         "branch-scope": "cov", "applies-first": "cov", "head-on-main": "cov", "gates-wiring": "mk",
         "ci-gating": "probe", "ci-gating-job": "probe",
         "ci-statement": "probe"}.get(w, "py")
p = pathlib.Path(FILES[which])
t = orig = p.read_text()

if w == "executed":            # run the command at all (the original c199c19 defect)
    t = t.replace("    key = (cmd, want_n == 0)", "    return []\n    key = (cmd, want_n == 0)", 1)
elif w == "cmd-referred":      # pdc/cargo/make are not observations
    t = t.replace("        if os.path.basename(head) in CMD_REFERRED:", "        if False:", 1)
elif w == "cmd-allowlist":     # only the five observation tools may run
    t = t.replace("        if head not in CMD_ALLOWED:", "        if False:", 1)
elif w == "cmd-artifact":      # a build artifact is not reproducible from a checkout
    t = t.replace("    if first in CMD_BUILD_ARTIFACT_ROOTS:", "    if False:", 1)
elif w == "cmd-operators":     # a shell operator is not a pipeline
    t = t.replace("        elif tok in CMD_OPERATORS:", "        elif False:", 1)
elif w == "result-compare":    # the claimed exit status and line count are compared
    t = t.replace("    if rc != want_rc or len(lines) != want_n:", "    if False:", 1)
elif w == "quote-check":       # quoted prose must appear in the output
    t = t.replace("    for q in QUOTED.finditer(rest):", "    for q in []:", 1)
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
elif w == "find-grammar":      # the permitted expression: <traversal>* <match>?
    # ONE reversion. `find-allowlist` and `find-grammar` used to be two names for this
    # same replacement, so "29 mutations" counted one twice. The nine controls that name
    # it all exercise branches of this one function, which is the unit being reverted.
    t = t.replace("        err = check_find_expression(opts)", "        err = None", 1)
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
elif w == "conformance-declared":  # a fixture the manifest does not declare at all
    t = t.replace("                    if declared is None:", "                    if False:", 1)
elif w == "cmd-grammar":       # a prose result must not be accepted
    t = t.replace('    if not m:\n        return [f"{name}: `cmd:` must be',
                  '    if not m:\n        return []\n    if False:\n        return [f"{name}: `cmd:` must be', 1)
elif w == "l1-sep-arg":        # an option whose argument would be counted as a path
    t = t.replace('            if base in GREP_OPTS_WITH_ARG and "=" not in tok:', "            if False:", 1)
elif w == "l1-downstream":     # a downstream segment may not name a file
    t = t.replace('        if n > 0 and parsed["paths"]:', "        if False:", 1)
elif w == "pin-semantic":     # a cited range need not carry any content
    # Reverts the NON-SEMANTIC classification: every range becomes "semantic",
    # so a pin that has come to rest on a bare `}` or a blank line is
    # fingerprint-stable, never MOVES, and is therefore never wrong -- which is
    # the laundering the four probe cases exist to catch.
    t = t.replace("    return bool(SEMANTIC.search(text))", "    return True", 1)
elif w == "blank-targets":     # the delimiter-only floor, reverted to the six-string list
    # Not `if False` like most of the others: switching the floor OFF would prove only
    # that some control notices the floor exists at all. What has to be caught is the
    # floor being written as an ENUMERATION again, so this mutation restores the exact
    # frozenset the predicate replaced. The six strings it knew must still pass, and the
    # ones it never knew -- `]`, `];`, `),`, `);`, `},`, a markdown rule -- must go red.
    # If they do not, the new cases are not testing the property they claim to.
    t = t.replace('    return re.search(r"\\w", norm(text)) is None',
                  '    return norm(text) in ("", "}", "{", "};", ")", "*/")', 1)
elif w == "pin-relocate":      # the stored hash is a tripwire only, never an address
    # Reverts the search itself: a pin whose content is intact elsewhere in the file goes
    # back to being an undifferentiated MOVED, which is the state that made authors shape
    # source code around line numbers.
    t = t.replace("    hits: list = []", "    return []\n    hits: list = []", 1)
elif w == "pin-relocate-fail":     # a relocation is REPORTED, and the run still passes
    # The relocation branch appends to two lists: `relocated` prints the move as its own
    # section, `fail` makes it a failure. Removing only the second leaves the printed report
    # byte-identical and exits 0 -- exactly the state this branch's parent fixed, where a
    # gate printed "RELOCATED" beside a green exit and nobody had to read it. `[].append`
    # still builds the message and throws it away, so what is reverted here is the FAILURE
    # and nothing else: the control that dies must die on the exit status.
    t = t.replace("                    relocated.append((key, want[key], hits[0]))\n"
                  "                    ns, ne, _ = hits[0]\n"
                  "                    fail.append(",
                  "                    relocated.append((key, want[key], hits[0]))\n"
                  "                    ns, ne, _ = hits[0]\n"
                  "                    [].append(", 1)
elif w == "pin-relocate-unique":   # any match will do, take the first
    # Repeated boilerplate is real, so a relocation that does not require UNIQUENESS
    # repoints the citation at whichever copy happens to come first in the file.
    t = t.replace("                if len(hits) == 1:", "                if hits:", 1)
elif w == "pin-relocate-trim":     # a window is named by its raw span, blank edges and all
    # `norm()` deletes a blank line, so a window ENDING on one and the window one line below
    # it STARTING on one hold the same text and hash to the same value. Named by their raw
    # spans they are TWO hits over ONE piece of content, and the pin is refused as AMBIGUOUS
    # between two ranges with no second copy to choose between -- an ambiguity manufactured
    # by the search rather than found in the file. Reverting the trim to identity also
    # neutralises the dedupe below it, since two windows can only share a span once their
    # blank edges are gone: they are one fix and this is the one reversion.
    t = t.replace("            tstart, tend, twindow = trim_blank_edges(start, end, window)",
                  "            tstart, tend, twindow = start, end, window", 1)
elif w == "pin-relocate-width":    # the range's HEIGHT is not held fixed
    # `norm()` collapses newlines, so a two-line range and the single line holding the same
    # two statements hash identically. Searching other widths lets a CHANGED citation
    # relocate onto a differently-shaped coincidence. Bounded to nlines+1 so the mutant is
    # merely wrong and not also quadratic.
    t = t.replace("    for i in range(len(lines) - nlines + 1):\n"
                  "        yield i + 1, i + nlines, lines[i:i + nlines]",
                  "    for i in range(len(lines)):\n"
                  "        for w in range(1, nlines + 2):\n"
                  "            if i + w <= len(lines):\n"
                  "                yield i + 1, i + w, lines[i:i + w]", 1)
elif w == "pin-relocate-zero":     # found nowhere is treated as nothing to report
    # The fail-closed half. With no match the content is GONE, and a search that let that
    # fall through would turn every deleted citation green -- worse than the tax removed.
    t = t.replace("                else:\n                    # ZERO IS A FAILURE",
                  "                elif False:\n                    # ZERO IS A FAILURE", 1)
elif w == "pin-relocate-update":   # --update may record over an unapplied move
    # The door the green RELOCATED verdict opens: the citing document still names the old
    # lines, so regenerating pins there fingerprints whatever moved INTO them, under a key
    # that looks untouched. That is the docstring's laundering, reached through the new
    # mechanism instead of around it.
    t = t.replace("        if pending:", "        if False:", 1)
elif w == "pin-repin":         # --update may re-snapshot a cited range whose content changed
    # The SAME-KEY half. `pending` declines when the old text is nowhere else, so without
    # this the generator records a different piece of content under an untouched key and
    # prints MOVED beside a zero exit -- a report, and a report is read once.
    t = t.replace("        repinned, renumbered = changed, []",
                  "        repinned, renumbered = [], []", 1)
elif w == "pin-repin-renumber":    # a citation may be renumbered away from its own content
    # The half `pending` cannot see AT ALL, because the pin key contains the line numbers:
    # move the DOCUMENT first and the old key is dropped while a new one is added, so
    # `changed` is empty and nothing compares the two. Eleven citations went through this
    # door in one commit.
    t = t.replace("            renumbered.append((k, old[k], hits[0], sorted(added_now[(p, d)])))",
                  "            pass", 1)
elif w == "pin-repin-flag":        # --allow-repin is inert, so the refusal has no door
    # A gate with no way through is deleted the first time a legitimate re-pin arrives, and
    # a legitimate re-pin is common. The flag is where the reading is ASSERTED; making it
    # inert removes the record, not the strictness.
    t = t.replace('        allow_repin = "--allow-repin" in sys.argv',
                  "        allow_repin = False", 1)
elif w == "pin-repin-benign":      # the guard turns absolute and refuses a CORRECT repair
    # The skip that recognises a renumbering ONTO the lines the content actually moved to.
    # Without it the everyday relocation the docstring prescribes needs --allow-repin, the
    # flag gets typed reflexively, and it stops meaning that anything was read.
    t = t.replace('            if f"{ns}-{ne}" in added_now[(p, d)]:',
                  "            if False:", 1)
elif w == "gate-kv-compare":   # key=value results are not compared at all
    t = t.replace("    for k, v in kv:", "    for k, v in []:", 1)
elif w == "gate-substring":    # key=value by containment instead of by value
    t = t.replace("    for k, v in kv:\n        if k not in seen:",
                  '    for k, v in kv:\n        if f"{k}={v}" in output:\n            continue\n        if k not in seen:', 1)

elif w == "gate-argv-grammar":       # the whole function, back to prefix-only
    import re as _re
    t = _re.sub(r"^allowed\(\) \{.*?^\}\n",
                'allowed() {\n  case "$1" in\n    "make "*|"cargo build"*|"cargo test"*) return 0 ;;\n'
                '  esac\n  return 1\n}\n', t, count=1, flags=_re.S | _re.M)
elif w == "gate-private-receipts":   # back to a shared, surviving receipts directory
    t = t.replace('OUT=$(mktemp -d "${TMPDIR:-/tmp}/palladium-gate-receipts.XXXXXX") || exit 2\n'
                  'trap \'rm -rf "$OUT"\' EXIT INT TERM',
                  'OUT=build_output/gate-receipts\nrm -rf "$OUT"; mkdir -p "$OUT"', 1)
elif w == "branch-scope":      # applicability is tied to the inventory's recorded base
    t = t.replace('  elif [ "$_mb" = "$INV_BASE" ]; then\n    APPLIES=yes',
                  '  elif true; then\n    APPLIES=yes', 1)
elif w == "head-on-main":      # HEAD on main is never a branch under review
    t = t.replace('  elif [ "$_mb" = "$(git rev-parse HEAD 2>/dev/null)" ]; then\n    WHY="head-is-on-main"',
                  '  elif false; then\n    WHY="head-is-on-main"', 1)
elif w == "applies-first":     # applicability is decided BEFORE any base is validated
    t = t.replace('if [ "$APPLIES" = yes ]; then\n  if [ -n "${COVERAGE_BASE:-}" ]; then',
                  'if true; then\n  if [ -n "${COVERAGE_BASE:-}" ]; then', 1)
elif w == "grep-deref":        # -R follows symlinks out of the checkout
    t = t.replace("            if base in GREP_DEREF_RECURSIVE:", "            if False:", 1)
    t = t.replace('                    if ch == "R":', "                    if False:", 1)
elif w == "artifact-ancestor": # a recursive root containing build output reads it
    t = t.replace("    if any((real / d).exists() for d in CMD_BUILD_ARTIFACT_ROOTS):",
                  "    if False:", 1)
elif w == "ci-statement":      # && / || are not unconditional command starts
    t = t.replace('re.split(r"[\\n;]", "\\n".join(out))',
                  're.split(r"[\\n;]|&&|\\|\\|", "\\n".join(out))', 1)
elif w == "gates-wiring":      # gate-receipts must be a prerequisite of `gates`
    t = t.replace("check-docs gate-receipts test-doc-evidence", "check-docs test-doc-evidence", 1)
elif w == "ci-gating-job":     # a JOB that cannot fail the run does not gate
    t = t.replace('    if (j.get("continue-on-error") or "false").lower() != "false":\n        continue',
                  '    if False:\n        continue', 1)
elif w == "ci-gating":         # a STEP that cannot fail the job does not gate
    t = t.replace('    if (s["continue-on-error"] or "false").lower() != "false":\n        continue',
                  '    if False:\n        continue', 1)
elif w == "ci-no-base":        # the workflow must not supply a base on main
    t = t.replace("      - name: Evidence gate probe coverage\n        run: bash scripts/test-doc-evidence-coverage.sh",
                  "      - name: Evidence gate probe coverage\n        run: |\n"
                  "          export COVERAGE_BASE='${{ github.event.before }}'\n"
                  "          bash scripts/test-doc-evidence-coverage.sh", 1)
elif w.startswith("ci-step-"):
    # ONE MUTATION PER STEP. A single `ci-steps` mutation removed only the first step
    # while THREE controls declared `kill ci-steps`, so two of them were never required
    # to fail by anything. Splitting is the honest unit: each step is a separate fix.
    step = {"ci-step-evidence": "bash scripts/check-doc-evidence.sh",
            "ci-step-receipts": "bash scripts/gate-receipts.sh",
            "ci-step-probe":    "bash scripts/test-doc-evidence.sh"}[w]
    import re as _re
    t = _re.sub(r"      - name: [^\n]*\n(        #[^\n]*\n)*        run: " + _re.escape(step) + r"\n",
                "", t, count=1)
else:
    sys.exit(2)
if t == orig:
    sys.exit(9)
p.write_text(t)
PYEOF
}

# name | file. What each mutation must KILL is not written here: it is written in
# scripts/doc-evidence-controls.tsv, once per control, and every row of that table is
# checked. Keeping the expectation in one place is the fix for the defect below.
MUTATIONS="executed|scripts/check_doc_evidence.py
cmd-referred|scripts/check_doc_evidence.py
cmd-allowlist|scripts/check_doc_evidence.py
cmd-artifact|scripts/check_doc_evidence.py
branch-scope|scripts/test-doc-evidence-coverage.sh
applies-first|scripts/test-doc-evidence-coverage.sh
head-on-main|scripts/test-doc-evidence-coverage.sh
grep-deref|scripts/check_doc_evidence.py
artifact-ancestor|scripts/check_doc_evidence.py
ci-statement|scripts/test-doc-evidence.sh
gates-wiring|Makefile
ci-gating|scripts/test-doc-evidence.sh
ci-gating-job|scripts/test-doc-evidence.sh
cmd-operators|scripts/check_doc_evidence.py
result-compare|scripts/check_doc_evidence.py
quote-check|scripts/check_doc_evidence.py
l1-path|scripts/check_doc_evidence.py
l1-pattern-opt|scripts/check_doc_evidence.py
l2-exists|scripts/check_doc_evidence.py
l2-symlink|scripts/check_doc_evidence.py
l3-probe|scripts/check_doc_evidence.py
l3-find-probe|scripts/check_doc_evidence.py
find-grammar|scripts/check_doc_evidence.py
exe-path|scripts/check_doc_evidence.py
tool-resolution|scripts/check_doc_evidence.py
pinned-env|scripts/check_doc_evidence.py
seg-status|scripts/check_doc_evidence.py
gate-checkable|scripts/check_doc_evidence.py
gate-exact|scripts/check_doc_evidence.py
gate-receipts-in-repo|scripts/check_doc_evidence.py
dynamic-evidence|scripts/check_doc_evidence.py
conformance-class|scripts/check_doc_evidence.py
conformance-declared|scripts/check_doc_evidence.py
cmd-grammar|scripts/check_doc_evidence.py
l1-sep-arg|scripts/check_doc_evidence.py
l1-downstream|scripts/check_doc_evidence.py
blank-targets|scripts/check_doc_evidence.py
pin-semantic|scripts/check_doc_evidence.py
pin-relocate|scripts/check_doc_evidence.py
pin-relocate-fail|scripts/check_doc_evidence.py
pin-relocate-trim|scripts/check_doc_evidence.py
pin-relocate-unique|scripts/check_doc_evidence.py
pin-relocate-width|scripts/check_doc_evidence.py
pin-relocate-zero|scripts/check_doc_evidence.py
pin-relocate-update|scripts/check_doc_evidence.py
pin-repin|scripts/check_doc_evidence.py
pin-repin-renumber|scripts/check_doc_evidence.py
pin-repin-flag|scripts/check_doc_evidence.py
pin-repin-benign|scripts/check_doc_evidence.py
gate-substring|scripts/check_doc_evidence.py
gate-kv-compare|scripts/check_doc_evidence.py
gate-argv-grammar|scripts/gate-receipts.sh
gate-private-receipts|scripts/gate-receipts.sh
ci-no-base|.github/workflows/preview.yml
ci-step-evidence|.github/workflows/preview.yml
ci-step-receipts|.github/workflows/preview.yml
ci-step-probe|.github/workflows/preview.yml"

# ∀ KILL ROW, NOT ∃ FRAGMENT PER MUTATION.
#
# The proposition this runner exists to establish is
#     for every `kill` control c,  reverting mutation(c) makes c fail.
# What it implemented was
#     for every mutation m,  SOME one named fragment fails.
# Those differ whenever more than one control names the same mutation, and three did:
# `ci-steps` removed a single CI step while three controls declared `kill ci-steps`, so
# two of them were never required to fail by anything. Measured, that is a real hole.
#
# THE PATTERN, because this is the eighth sighting of this class in this branch and it is
# worth writing down rather than fixing again: WHENEVER YOU WRITE "EVERY X", CHECK WHETHER
# THE LOOP IS OVER X OR OVER SOMETHING X IS GROUPED UNDER. Here X was the control and the
# loop was over the mutation. Earlier on this branch it was `cmd:` items grouped under
# their shape, gate tokens grouped under containment, controls grouped under a count, and
# fixes grouped under files. Same shape every time.
#
# So: each mutation is applied once, the FULL set of failing labels is recorded, and then
# every `kill` row is checked against the set belonging to the mutation it names.
failing_labels() {
  bash scripts/test-doc-evidence.sh 2>&1 | sed 's/\x1b\[[0-9;]*m//g' \
    | grep '^  FAIL ' | sed 's/^  FAIL //' | sed 's/[[:space:]]*$//'
}

echo
base_fail=$(failing_labels)
if [ -n "$base_fail" ]; then
  echo "${RED}ABORT${NC}: the controls are not green to begin with, so every number below"
  echo "       would be measuring that instead of the mutation:"
  printf '%s\n' "$base_fail" | sed 's/^/         /'
  exit 1
fi
printf '  %sok%s            unmutated: 0 controls failing\n\n' "$GREEN" "$NC"

dead=0; problems=0; ran=0; COVERED_FILES=""
KILLSETS=$TMP_KILL
: > "$KILLSETS"
while IFS= read -r entry; do
  [ -z "$entry" ] && continue
  m=${entry%%|*}; mf=${entry#*|}
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
  f=$(failing_labels)
  restore
  n=0; [ -n "$f" ] && n=$(printf '%s\n' "$f" | wc -l | tr -d ' ')
  printf '%s\n' "$f" | sed "s|^|$m\t|" >> "$KILLSETS"
  if [ "$n" -eq 0 ]; then
    printf '  %sUNCOVERED%s     %-22s reverted, and NOT ONE control noticed\n' "$RED" "$NC" "$m"
    problems=$((problems+1))
  elif ! awk -F'\t' -v m="$m" '$2=="kill" && $3==m {f=1} END{exit !f}' "$CONTROLS"; then
    # No control is CREDITED to it. That is fine for a deliberate aggregate — `executed`
    # removes the whole checker — but it has to be said, because "31 controls went red"
    # otherwise reads as coverage for whatever those 31 controls are about. The reason
    # each of them dies belongs to a narrower mutation, and that is where it is checked.
    printf '  %sok%s            %-22s %2d red (AGGREGATE: no control is credited to it)\n' \
      "$YEL" "$NC" "$m" "$n"
  else
    printf '  %sok%s            %-22s %2d control(s) went red\n' "$GREEN" "$NC" "$m" "$n"
  fi
done <<EOF2
$MUTATIONS
EOF2

# --- the proposition itself: every kill row, checked by name -----------------------------
echo
echo "every `kill` control, against the mutation it names:"
killrows=0
while IFS=$'\t' read -r label role detail; do
  case "$label" in ''|'#'*) continue ;; esac
  [ "$role" = kill ] || continue
  killrows=$((killrows+1))
  if ! printf '%s\n' "$MUTATIONS" | grep -q "^$detail|"; then
    printf '  %sPHANTOM-KILL%s  %-22s no mutation by that name exists (declared by: %s)\n' \
      "$RED" "$NC" "$detail" "$label"
    problems=$((problems+1)); continue
  fi
  if awk -F'\t' -v m="$detail" -v l="$label" '$1==m && $2==l {f=1} END{exit !f}' "$KILLSETS"; then
    printf '  %sok%s   %-22s kills: %s\n' "$GREEN" "$NC" "$detail" "$label"
  else
    printf '  %sNOT-KILLED%s %-22s does NOT kill: %s\n' "$RED" "$NC" "$detail" "$label"
    printf '             (the control declares this mutation, and reverting it left the case passing)\n'
    problems=$((problems+1))
  fi
done < "$CONTROLS"
echo

# A file declared `mutated` that nothing mutates is the omission this reconciliation
# exists to surface, and it is exactly what happened to four files last round.
for f in $MUTATED_FILES; do
  case " $COVERED_FILES " in
    *" $f "*) ;;
    *) printf '  %sNO-MUTATION%s   %-22s declared mutated, but nothing reverts anything in it\n' \
         "$RED" "$NC" "$f"; problems=$((problems+1)) ;;
  esac
done

echo
echo "=============================================="
echo "MUTATED (fixes reverted, controls required to notice):"
printf '%s\n' $MUTATED_FILES | sed 's/^/  /'
echo "NOT MUTATED, and why -- every one declared in $INVENTORY:"
awk -F'\t' '$1!~/^#/ && NF>=2 && $2!="mutated" {printf "  %-46s %s\n", $1, $2}' "$INVENTORY"
if [ "$problems" -eq 0 ] && [ "$dead" -eq 0 ]; then
  echo "${GREEN}coverage green${NC} -- $ran mutation(s) across $(printf '%s\n' $MUTATED_FILES | wc -l | tr -d ' ') file(s);"
  echo "all $killrows kill row(s) killed by the mutation each names; $(printf '%s\n' "$LIVE" | wc -l | tr -d ' ') control(s) reconciled."
  if [ -n "$CHANGED" ]; then
    echo "File inventory reconciled with git ($BASE_WHY)."
  else
    echo "File inventory NOT reconciled: no base commit was available, so that check did"
    echo "not run. It is not claimed here."
  fi
  echo "=============================================="
  exit 0
fi
echo "${RED}coverage FAILED${NC} -- $problems problem(s), $dead dead mutation(s)"
echo "=============================================="
exit 1
