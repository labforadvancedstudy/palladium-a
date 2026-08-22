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

TMP_KILL=$(mktemp "${TMPDIR:-/tmp}/doc-evidence-killsets.XXXXXX") || exit 2
restore() { git checkout -- $MUTABLE 2>/dev/null; }
trap 'restore; rm -f "$TMP_KILL"' EXIT INT TERM

# --- the file-set reconciliation: branch-time, and honest about when it is not ---------
CONTROLS=scripts/doc-evidence-controls.tsv
[ -f "$INVENTORY" ] || { echo "error: $INVENTORY missing; it IS the file denominator." >&2; exit 2; }
[ -f "$CONTROLS" ]  || { echo "error: $CONTROLS missing; it IS the control denominator." >&2; exit 2; }

# In order: an explicit base; the first parent when HEAD is a merge (post-merge on main,
# where `merge-base main HEAD` is HEAD and the diff would be empty); the merge-base with
# main or origin/main on a feature branch. If none resolves -- a shallow CI checkout has
# no main ref and no HEAD^ -- the file reconciliation is NOT APPLICABLE and says so.
BASE=""; BASE_WHY=""; ON_BRANCH=no
if [ -n "${COVERAGE_BASE:-}" ]; then
  # AN EXPLICIT REQUEST THAT CANNOT BE HONOURED IS AN ERROR, NOT A SKIP. Previously an
  # invalid COVERAGE_BASE fell through to the same green NOT APPLICABLE as "no base
  # exists", so asking for a reconciliation and silently not getting one looked identical
  # to not asking.
  BASE=$(git rev-parse --verify "${COVERAGE_BASE}^{commit}" 2>/dev/null) || BASE=""
  if [ -z "$BASE" ]; then
    echo "error: COVERAGE_BASE='${COVERAGE_BASE}' does not resolve to a commit. A base was" >&2
    echo "       explicitly requested and cannot be honoured; refusing to report a" >&2
    echo "       reconciliation that did not happen." >&2
    exit 2
  fi
  # RESOLVING IS NOT ENOUGH. An unrelated commit resolves fine and would be reported as a
  # meaningful "changed since"; HEAD itself resolves and yields an empty diff, which then
  # degraded to a green NOT APPLICABLE — an explicitly requested reconciliation quietly
  # not happening, which is the same defect as an unresolvable base, one step later.
  if ! git merge-base --is-ancestor "$BASE" HEAD 2>/dev/null; then
    echo "error: COVERAGE_BASE='${COVERAGE_BASE}' is not an ancestor of HEAD, so the diff" >&2
    echo "       between them is not 'what this branch changed'." >&2
    exit 2
  fi
  if [ "$BASE" = "$(git rev-parse HEAD)" ]; then
    echo "error: COVERAGE_BASE='${COVERAGE_BASE}' IS HEAD, so the reconciliation would be" >&2
    echo "       over an empty file set. A base was explicitly requested; an empty answer" >&2
    echo "       is not one." >&2
    exit 2
  fi
  BASE_WHY="COVERAGE_BASE"; ON_BRANCH=yes
else
  # On a FEATURE BRANCH the base is the merge-base with main. Deliberately preferred over
  # HEAD^1 even when HEAD is a merge: a branch that merges main INTO itself has a first
  # parent measuring the incoming upstream delta, not this branch's work.
  for ref in main origin/main; do
    git rev-parse --verify "$ref" >/dev/null 2>&1 || continue
    cand=$(git merge-base "$ref" HEAD 2>/dev/null) || continue
    [ -n "$cand" ] || continue
    if [ "$cand" = "$(git rev-parse HEAD)" ]; then
      # HEAD is ON main: a push, a merge, or a squash. What that push introduced is
      # HEAD^1..HEAD — for a merge commit the first parent is the target branch, so this
      # is the incoming work; for a squash it is the whole branch.
      # HEAD^1 is right for a merge or a squash, and WRONG for an ordinary push of
      # several commits: it reconciles only the last one. CI supplies the push-before SHA
      # (github.event.before) through COVERAGE_BASE, which is handled above; this is the
      # local fallback and its limit is stated rather than hidden.
      BASE=$(git rev-parse --verify 'HEAD^1' 2>/dev/null) || BASE=""
      [ -n "$BASE" ] && BASE_WHY="HEAD is $ref; first parent (a multi-commit push needs COVERAGE_BASE)"
    else
      BASE=$cand; BASE_WHY="merge-base with $ref"; ON_BRANCH=yes
    fi
    [ -n "$BASE" ] && break
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
  # UNDECLARED is fatal everywhere: a file changed NOW with no row is a real gap whoever
  # is looking. MISSING is fatal only on a feature branch, and here is why the two differ.
  # The inventory describes ONE BRANCH's changes. Off that branch — on main, after the
  # merge, for any later push — a declared file the current diff does not touch is not a
  # defect in the inventory, it is the inventory being scoped to something else. Making it
  # fatal there would turn every subsequent push red for a reason nobody could act on,
  # which is how a gate teaches people to disable it.
  missing_note=0
  while IFS=$'\t' read -r path disp _; do
    case "$path" in ''|'#'*) continue ;; esac
    if ! printf '%s\n' "$CHANGED" | grep -qxF -- "$path"; then
      if [ "$ON_BRANCH" = yes ]; then
        printf '  %sMISSING%s       %s -- declared %s, but this branch did not change it\n' \
          "$RED" "$NC" "$path" "$disp"
        recon=$((recon+1))
      else
        missing_note=$((missing_note+1))
      fi
    fi
  done < "$INVENTORY"
  if [ "$missing_note" -gt 0 ]; then
    printf '  %snote%s          %d declared file(s) are outside this diff. Not an error here:\n' \
      "$YEL" "$NC" "$missing_note"
    printf '                the inventory is scoped to a branch and this is not that branch.\n'
  fi
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
         "mk": "Makefile", "ci": ".github/workflows/preview.yml"}
which = {"gate-argv-grammar": "gr", "gate-private-receipts": "gr",
         "ci-step-evidence": "ci", "ci-step-receipts": "ci",
         "ci-step-probe": "ci"}.get(w, "py")
p = pathlib.Path(FILES[which])
t = orig = p.read_text()

if w == "executed":            # run the command at all (the original c199c19 defect)
    t = t.replace("    key = (cmd, want_n == 0)", "    return []\n    key = (cmd, want_n == 0)", 1)
elif w == "cmd-referred":      # pdc/cargo/make are not observations
    t = t.replace("        if os.path.basename(head) in CMD_REFERRED:", "        if False:", 1)
elif w == "cmd-allowlist":     # only the five observation tools may run
    t = t.replace("        if head not in CMD_ALLOWED:", "        if False:", 1)
elif w == "cmd-artifact":      # a build artifact is not reproducible from a checkout
    t = t.replace("            if tok.startswith(CMD_BUILD_ARTIFACT):", "            if False:", 1)
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
gate-substring|scripts/check_doc_evidence.py
gate-kv-compare|scripts/check_doc_evidence.py
gate-argv-grammar|scripts/gate-receipts.sh
gate-private-receipts|scripts/gate-receipts.sh
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
