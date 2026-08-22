#!/usr/bin/env bash
# THE DEFINITION OF PALLADIUM 1.0, AS A COMMAND. RED until M9, on purpose.
#
# 1.0 is the thesis, not an inventory: bootstrap/pdc.pd rewritten in the
# differentiated dialect, still reaching a byte-identical stage1/stage2 fixed
# point, with a second witness program meeting the same conditions. A vacuous
# conformance fixture can print "unimplemented" and PASS; a compiler cannot
# compile itself vacuously.
#
# The definition lives in docs/contributing/1.0-requirements.tsv — the rows whose
# `disposition` is `thesis`. This command reads and EXECUTES them, and it also
# carries a version-controlled copy of the full contract (kind, evidence,
# fingerprint) which it compares against them.
#
# That duplication is DELIBERATE and it is a reviewed cross-check, not a second
# definition. The two copies catch different defects: the pin catches an edit to
# the manifest, and `_validate_contract()` catches a defect in the pin itself —
# a `reject` row pinned with no required fingerprint would otherwise match a
# manifest that agreed with it, and the fingerprint comparison would be skipped.
# Weakening both together is possible and is meant to be: it takes an edit to two
# files in one commit, which is exactly the thing a reviewer can see.
#
# Conditions 2 and 3 are delegated to scripts/conformance.sh, which already
# compiles, links, runs, diffs stdout against a recorded transcript, checks the
# declared failure stage, matches the declared diagnostic fingerprint, reports
# REJECT_ACCEPTED when a negative test is accepted, and reports MISSING when a
# declared fixture is not on disk. The first version of this gate re-implemented
# none of that and checked the manifest's TEXT instead, so a reject twin the
# compiler happily accepted reported green.
#
# Usage:
#   scripts/thesis-exit.sh              # exit 0 only when 1.0 is real
#   scripts/thesis-exit.sh --self-test  # fault-inject every probe (make test-thesis-runner)
#
# MACHINE CONTRACT. This script's exit code is three-valued:
#   0  THESIS_HOLDS   the thesis is proven
#   1  THESIS_FALSE   it is not — a measurement about Palladium
#   2  NO_VERDICT     the gate could not or would not measure; nothing may be inferred
#
# `make thesis-exit` CANNOT CARRY THAT: Make maps every nonzero recipe status to 2, so a
# status-only consumer sees the same number for "the thesis is false", "no verdict is
# available" and "the build is broken". Consumers that need the distinction must either
# call this script directly, or parse the last line of output, which is
#
#   THESIS_RESULT <code> <name>
#
# and survives the Make layer intact. THE CONTRACT ON A CONSUMER, precisely:
#
#   * AWAIT PROCESS COMPLETION before reading. A partial stream may not contain the line.
#   * Accept exactly ONE occurrence, ANCHORED at the start of a line, as the FINAL line of
#     STDOUT. Do not first-match: a merged stdout+stderr stream can end with Make's own
#     `*** [thesis-exit] Error 2`, and prose above may quote the token.
#   * Read STDOUT only. The line is never written to stderr.

set -uo pipefail
cd "$(dirname "$0")/.."
exec python3 scripts/thesis_exit.py "$@"
