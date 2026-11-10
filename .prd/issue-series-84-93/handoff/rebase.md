# Rebase onto the new main — every unit (read with coder-rules.md)

main moved: #85 (903ba02, PD0079), #93+#91+#79 (2ca5eb5, PD0080/PD0081) and now #84 landed: **main = f2d1dd3** (origin/main).
#84 added NO diagnostic code; it changed src/codegen/mod.rs (self-reading `let` lowered through a temporary named after the whole
unit by `name_shadow_temporaries`; write_output refuses a leftover placeholder), tests/05_tuple_reference_params fixture lines, the
spec A-shadowing paragraph, many citation line numbers and docs/citation-pins.tsv. Conformance counts on main f2d1dd3: fixtures 231,
verified 92, reject 125. Your codes still start at **PD0082** (the next lander after you renumbers).

Your round's work is now COMMITTED LOCALLY in your worktree by the orchestrator (do not push; nothing is pushed until landing).
For this step only you MAY run git in your own worktree: `git fetch origin` and `git rebase origin/main` (= f2d1dd3), resolving
conflicts and `git rebase --continue` (no other commits, no amend of anyone else's work, never push, never force anything).

Resolution rules:
- Diagnostic codes: your new code(s) take the NEXT FREE number(s) on the main you rebase onto (on f2d1dd3: PD0082 upward). Rename
  everywhere (codes.rs incl. the ALL-count literal, registry tsv, pin map, check-diagnostic-codes.sh counts, manifest `code=` pins,
  tests, docs). Run `bash scripts/check-diagnostic-codes.sh`.
- docs/citation-pins.tsv: take main's side (`git checkout --theirs`/`--ours` as appropriate for a rebase — the pinned file must be
  main's), then fix your claims and re-pin with `bash scripts/check-doc-evidence.sh --update --allow-repin` LAST, then a plain run.
- Counts in prose / feature-index `gate:` strings / MILESTONES: re-derive from the rebased tree (your fixtures + main's), never
  add numbers by hand from two sides. `python3 scripts/requirements.py --check-ledger`, `make gate-receipts`.
- Conflicts in source: keep both changes; re-run your unit's tests and the lib tests.
Then build, run your focused tests, and the FULL gates DETACHED via the lock wrapper (.../iss/gates-locked.sh <work>/gates-rebased.log).
Report (≤300 words): the new base, the codes you took, conflicts and how resolved, the gates' last line. Create <work>/done.
After landing of another unit the orchestrator may ask you to repeat this (main moves one unit at a time).
