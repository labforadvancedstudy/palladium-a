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
# Exit: 0 = the thesis holds · 1 = it does not · 2 = the gate could not measure.

set -uo pipefail
cd "$(dirname "$0")/.."
exec python3 scripts/thesis_exit.py "$@"
