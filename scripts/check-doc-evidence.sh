#!/usr/bin/env bash
# Gate the documentation's *evidence*, not its prose.
#
# scripts/check-docs.sh proves that documentation snippets compile. It cannot prove
# that a `file:line` citation still points at the code it names, that an absence
# claim was ever measured, or that the set of snippets exempted from compilation is
# the set somebody intended. Those are the three ways this documentation has actually
# rotted, and each gets a check here.
#
#   1. CITATION PINS. Every `path:line` citation in docs/ is resolved, and the cited
#      line is compared against a fingerprint recorded in docs/citation-pins.tsv.
#      A source edit that moves a cited line fails here instead of rotting silently.
#      This is not hypothetical: every codegen/parser/typeck/driver citation in the
#      v0.2 specification had been taken from a superseded revision and pointed at
#      unrelated code by the time it shipped.
#
#   2. NO-COMPILE ALLOWLIST. `scripts/check-docs.sh` reports `skipped(no-compile)=N`
#      but nothing pins N, so the count can drift upward while the gate stays green —
#      the same unbounded drain the fences were introduced to prevent. The exact
#      per-file counts are pinned in docs/no-compile-allowlist.txt.
#
#   3. EVIDENCE TAGS. Every row in docs/reference/features/feature-index.yaml must
#      carry checkable evidence: a source location, a command with its result, a
#      conformance verdict, or a gate outcome. Prose such as "there is no X" is an
#      assertion, not evidence, and is rejected. An absence must be proved by a
#      command whose empty output IS the evidence.
#
# Usage:
#   scripts/check-doc-evidence.sh            # check (exit 1 on any failure)
#   scripts/check-doc-evidence.sh --update   # regenerate the pin and allowlist files
#
# --update is how you record a legitimate move: re-run it, and the diff shows exactly
# which citations changed. Never edit the generated files by hand.

set -uo pipefail
cd "$(dirname "$0")/.."
exec python3 scripts/check_doc_evidence.py "$@"
