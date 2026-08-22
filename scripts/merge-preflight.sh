#!/usr/bin/env bash
# Run the gates against the MERGE RESULT, not against this worktree.
#
# THE GAP THIS EXISTS FOR, MEASURED RATHER THAN IMAGINED. On 2026-08-22
# fix/d3b-tail-if was green under `make gates` six consecutive times in its own
# worktree, and its merge into main was RED with 18 citation ranges MOVED. No
# textual conflict reported it: main's docs/contributing/MILESTONES.md cites
# LINE RANGES in src/codegen/mod.rs, src/typeck/mod.rs, the Makefile and
# scripts/test-xfail.py, and this branch had inserted lines above every one of
# them. Git merged both sides cleanly because they touch different regions of
# different files. The invariant they jointly break is a SEMANTIC conflict, and
# git has no opinion about those.
#
# So the branch gate was never wrong. It certified the branch tree, which is
# exactly what it says it does. What nobody ran was a gate over the thing being
# shipped — the same sentence as this branch's own MF2, one level up: a check
# whose scope is narrower than the claim made about it. `make gates` green is a
# statement about a tree that is not the tree that lands.
#
# WHAT THIS BUYS, AND WHAT IT CANNOT. It answers "would the merge of THIS branch
# into <ref> AS <ref> STANDS RIGHT NOW pass the gates". It therefore:
#
#   * catches main moving under a branch (it moved twice during one session of
#     this branch's round 19, once mid-diagnosis);
#   * catches semantic conflicts git merges silently, which is the whole class;
#   * CANNOT see a sibling branch that has not landed. Two branches in flight can
#     each be green here and still break each other. That is not a bug in this
#     script, it is a property of measuring against a tip, and the fix for it is
#     merge ORDER, not more measurement.
#
# Because of that, the ref AND its sha are printed on every run, pass or fail. A
# preflight that does not say what it measured against is a receipt for an
# unnamed tree, and this branch spent a round on exactly that mistake.
#
# Usage: scripts/merge-preflight.sh [ref]        (default: main)

set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

REF="${1:-main}"
GREEN=$'\033[0;32m'; RED=$'\033[0;31m'; YELLOW=$'\033[0;33m'; NC=$'\033[0m'

if ! git rev-parse --verify --quiet "$REF^{commit}" >/dev/null; then
  printf '%sMALFUNCTION%s no such ref: %s — nothing was measured.\n' "$RED" "$NC" "$REF"
  exit 2
fi
TARGET=$(git rev-parse "$REF")
HEAD_SHA=$(git rev-parse HEAD)
BRANCH=$(git rev-parse --abbrev-ref HEAD)

# THE TREE MUST BE CLEAN, and this is a refusal rather than a stash. The merge
# below is aborted on the way out; running it over uncommitted work would abort
# that work with it. Refusing is the only safe reading of a dirty tree.
if [ -n "$(git status --porcelain)" ]; then
  printf '%sREFUSED%s the worktree is dirty. This runs a real merge and aborts it on\n' "$RED" "$NC"
  printf '        the way out, which would take uncommitted work with it. Commit or\n'
  printf '        stash first. NOTHING WAS MEASURED.\n'
  exit 2
fi

echo "=============================================="
echo "merge preflight"
echo "  branch : $BRANCH ($HEAD_SHA)"
echo "  into   : $REF ($TARGET)"
echo "=============================================="

if git merge-base --is-ancestor "$TARGET" HEAD; then
  printf '%s%s is already an ancestor of HEAD%s — the merge is a fast-forward, so the\n' \
    "$YELLOW" "$REF" "$NC"
  printf 'merge result IS this worktree and `make gates` already measured it.\n'
  exit 0
fi

cleanup() {
  git merge --abort >/dev/null 2>&1
  git reset --hard "$HEAD_SHA" >/dev/null 2>&1
}
trap cleanup EXIT INT TERM

if ! git merge --no-commit --no-ff "$TARGET" >/dev/null 2>&1; then
  conflicts=$(git diff --name-only --diff-filter=U)
  if [ -n "$conflicts" ]; then
    printf '%sCONFLICT%s the merge does not apply cleanly. Resolve it on this branch\n' "$RED" "$NC"
    printf '         (`git merge %s`), then run this again. Conflicted:\n' "$REF"
    printf '%s\n' "$conflicts" | sed 's/^/           /'
    printf '         NOTHING WAS MEASURED — a tree that does not exist has no verdict.\n'
    exit 1
  fi
fi

echo
echo "${YELLOW}merged cleanly — running the gates against the MERGE RESULT${NC}"
echo

make gates
rc=$?

echo
echo "=============================================="
if [ "$rc" -eq 0 ]; then
  printf '%s✓ merge preflight GREEN%s — %s merged into %s (%s) passes `make gates`.\n' \
    "$GREEN" "$NC" "$BRANCH" "$REF" "$TARGET"
  printf '  Valid for %s AS OF %s. If %s moves, this verdict expires.\n' "$REF" "$TARGET" "$REF"
else
  printf '%s✗ merge preflight RED%s — %s is green on its own and its merge into %s (%s)\n' \
    "$RED" "$NC" "$BRANCH" "$REF" "$TARGET"
  printf '  is not. `make gates` exited %d above. Do NOT hand this back as ready.\n' "$rc"
fi
echo "=============================================="
exit $rc
