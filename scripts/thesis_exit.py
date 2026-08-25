#!/usr/bin/env python3
"""THE DEFINITION OF PALLADIUM 1.0, AS A COMMAND.

1.0 is the thesis, proven on the one artifact here that structurally cannot lie:
bootstrap/pdc.pd, rewritten in the differentiated dialect, still reaching a byte-identical
stage1/stage2 fixed point, plus a second witness. A conformance fixture can print "not yet
implemented" and PASS. A compiler cannot compile itself vacuously.

THREE THINGS THIS FILE HAS BEEN WRONG ABOUT
-------------------------------------------
1. It checked manifest TEXT and ran nothing, so a reject twin the compiler ACCEPTED
   reported green — inside the condition that exists because, for an inference feature,
   the rejection is the product. Fixed by delegating to scripts/conformance.sh.

2. It could never say yes. `D1-01` cites `make thesis-exit` as its own evidence and was
   recorded False unconditionally, so exit 0 was unreachable. That is the mirror image of
   a gate that can never go RED: it does not measure either. A self-referential row is not
   a member of the set — it IS the aggregate, and it is now answered by the summary.
   The self-test's first case drives an all-green state and asserts exit 0, so this
   cannot silently return.

3. It read output from processes that had not finished. Every subprocess now goes through
   scripts/gate_probe.py: one boundary, `classify()` returning `Concluded` (which has a
   `.text`) or `Malfunction` (which does not), and the timeout inside `run()`.

   Stated precisely, because an earlier phrasing here claimed more than the module
   delivers: this does NOT make reading a dying producer's output impossible. Python has
   no access control and the bytes remain reachable through private attributes. What it
   does is put every subprocess behind one decision point and make the honest path the
   easy one — no verdict-shaped API hands you text that was never established as
   evidence. That is a real property and it is the one being relied on here.

MEASUREMENT FAILURE IS NOT A VERDICT
------------------------------------
    exit 0  the thesis holds
    exit 1  the thesis does not hold — a FINDING about the project
    exit 2  the gate could not measure — nothing may be inferred

A witness the definition names but the repository does not contain is a FINDING
(`DECLARED, ABSENT`): a fact about the project. A witness that exists and cannot be read,
a conformance run that did not conclude, a `pdc` that died or rejected the witness — those
are malfunctions and exit 2. Conflating them is how a failure to measure gets read as a
passing measurement.

THE ROW SET IS CLOSED, AND THE PIN IS A CROSS-CHECK
---------------------------------------------------
The definition lives in docs/contributing/1.0-requirements.tsv, in the rows whose
`disposition` is `thesis`. `EXPECTED_THESIS_CONTRACT` below is a version-controlled copy
of the full contract — id, kind, evidence locator, required fingerprint — compared against
the manifest on every run.

The duplication is the point, and it is a REVIEWED CROSS-CHECK rather than a second
definition. Each copy catches what the other cannot: the pin catches an edit to the
manifest (retyping a `reject` row to `fixture` turns a negative test into a positive one),
and `_validate_contract()` catches a defect in the pin (a `reject` row pinned with `-`
would match a manifest that agreed with it, and `p_verdict` would then skip the
fingerprint comparison entirely). Weakening both in one commit is possible and is meant to
be: it is a diff a reviewer can see, which is the level this is defended at.

Every row produces exactly one result and that is asserted, because "the summary printed
23" while a retyped row was silently skipped is the same defect one level up.

Usage:
    scripts/thesis-exit.sh                exit 0 only when 1.0 is real
    scripts/thesis-exit.sh --self-test    drive this gate with injected repository states
"""

from __future__ import annotations

import hashlib
import importlib.util
import io
import os
import re
import secrets
import subprocess
import sys
import traceback
import tempfile
from contextlib import redirect_stderr, redirect_stdout
from collections.abc import Mapping
from types import MappingProxyType
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


class HarnessError(Exception):
    """The gate could not evaluate something. NOT a verdict about the language."""


class Absent(Exception):
    """The definition names an artifact the repository does not contain.

    A FINDING, not a malfunction. "The witness has not been written yet" is a fact about
    the project; "the witness exists and I could not read it" is a failure to measure.
    """


def _load_gate_probe():
    """The typed process boundary. Guarded, because losing it must not silently downgrade
    this gate to reading text from processes that did not finish."""
    try:
        spec = importlib.util.spec_from_file_location(
            "gate_probe", ROOT / "scripts/gate_probe.py")
        if spec is None or spec.loader is None:
            raise HarnessError("scripts/gate_probe.py is not importable")
        mod = importlib.util.module_from_spec(spec)
        sys.modules["gate_probe"] = mod
        spec.loader.exec_module(mod)
    except HarnessError:
        raise
    except Exception as exc:  # noqa: BLE001 — a broken/absent dependency is a FAILURE TO
        # MEASURE, not a thesis finding. Unwrapped it escaped as Python's exit 1, which is
        # the code reserved for "1.0 is not reached".
        raise HarnessError(f"scripts/gate_probe.py could not be loaded: {exc!r}") from exc
    return mod


GP = None  # the gate_probe module; loaded on first use

# The rows that ARE the definition. Pinned, so the manifest and this command cannot drift
# apart in either direction.
# --- WHY THIS COMMAND SOMETIMES REFUSES TO ANSWER -----------------------------------
#
# Four rounds running, a safeguard for this gate's own weakness was checked by asking
# whether an artifact existed. The ladder is visible in the history: a NAME exists -> a
# TEST exists -> a test PASSES -> a TARGET exits 0. Each round the check climbed a level
# and each level asked the same question, because ANY check on a not-yet-existing
# artifact degenerates to "something by that name did not fail". An empty `#[test]`
# satisfied the third level; `@true` satisfied the fourth. A fifth would find a fifth.
#
# So the two safeguards are no longer SCORED. They are PRECONDITIONS ON THIS COMMAND'S
# ABILITY TO COMPUTE A VERDICT AT ALL, and — this is the part that cannot be faked — they
# are decided by introspecting THIS FILE'S OWN WIRING, not by looking for an artifact:
#
#   * GI-11 is satisfied when TH-03/04/05 are DERIVED FROM A CALL GRAPH. Not when a test
#     named after a call graph passes. The gate has to actually dispatch somewhere else,
#     and `_wiring_matches_declaration()` fails if the constant below says it does while
#     the dispatch table still says otherwise.
#   * GI-12 is satisfied when a reject row is adjudicated by a diagnostic CODE. Not when
#     a target exits 0.
#
# While either is outstanding, `make thesis-exit` EXITS 2 — "could not measure" — and
# offers no verdict. That is stronger than a RED row: a RED row says "1.0 is not reached
# yet", which is a measurement, and this command is not entitled to make one while the
# things it would measure with are disclosed as unsound. It still prints every row's
# state, because losing the dashboard would be a real cost and is not necessary to stop
# the false certificate.
#
# Flipping either constant without doing the work is caught by the self-test, which
# compares the declaration against the dispatch table.

LIVENESS_MODEL = "lexical"        # -> "call-graph" when GI-11 lands
ATTRIBUTION_MODEL = "substring"   # -> "code" when GI-12 lands

# The dispatch each model implies. `_wiring_matches_declaration()` checks reality.
LIVENESS_PROBES_LEXICAL = ("p_has_ref_param", "p_total_on_fn", "p_effect_is_transitive")

PRECONDITIONS = (
    ("GI-11", "LIVENESS_MODEL", "lexical", "call-graph",
     "TH-03/04/05 are decided by a lexical model disclosed as unsound for liveness: it "
     "answers P1 (the construct exists) and P2 (nothing names it), never whether the "
     "construct is on a path the program runs"),
    ("GI-12", "ATTRIBUTION_MODEL", "substring", "code",
     "rejection attribution is decided by `grep -qF` over the whole ANSI-stripped log, "
     "disclosed as unsound for attribution: measured, a fixture failing on a stray `@@@` "
     "satisfies a pinned fingerprint that its source line merely contains"),
)


# THE PRECONDITION SET IS PINNED, LIKE EVERY OTHER DEFINITIONAL SET HERE. It was the one
# that was not, and it decides whether this command may compute a verdict at all.
#
# Measured, by deleting GI-12's tuple: `incomplete_definition()` stopped refusing for it
# while ATTRIBUTION_MODEL was still `substring`, and GI-12 reappeared as an ordinary SCORED
# row printing `RED GI-12 … owed by M2`. That is exactly the state the preconditions design
# exists to prevent — "they were ordinary 1.0 rows, so `make thesis-exit` could have gone
# green while rejection attribution was still satisfiable by incidental text" — reopened by
# a one-line deletion. Today GI-11 still blocks, so the exit code does not change and the
# demotion is invisible in it; it becomes fatal the day GI-11 lands.
#
# The self-test does catch it (four cases, via the `BLOCKED=` half of the failure
# signature added in round 20 — before that they asserted a bare `2` and would have
# passed). A pin is stronger: it makes the failure a REFUSAL rather than a verdict, which
# is the property every other definitional set on this branch already has.
EXPECTED_PRECONDITION_IDS = ("GI-11", "GI-12")
EXPECTED_PRECONDITION_CONSTS = {"GI-11": ("LIVENESS_MODEL", "lexical", "call-graph"),
                                "GI-12": ("ATTRIBUTION_MODEL", "substring", "code")}


def validate_preconditions() -> None:
    """The safeguard set must be the reviewed one. Drift is a FAILURE TO MEASURE."""
    ids = tuple(rid for rid, *_ in PRECONDITIONS)
    if ids != EXPECTED_PRECONDITION_IDS:
        raise HarnessError(
            f"the precondition set changed: {ids} against the pinned "
            f"{EXPECTED_PRECONDITION_IDS}. These are the safeguards that stop this command "
            "computing a verdict with tools it has disclosed as unsound; removing one does "
            "not make it satisfied, it makes it SCORED — an ordinary red row, which is the "
            "arrangement the preconditions replaced. Re-pin deliberately or restore it.")
    for rid, const, unsound, sound, _why in PRECONDITIONS:
        want = EXPECTED_PRECONDITION_CONSTS[rid]
        if (const, unsound, sound) != want:
            raise HarnessError(
                f"precondition {rid} now watches {(const, unsound, sound)}, pinned {want}. "
                "Repointing a safeguard at a different constant retires it silently.")


LIVENESS_CORPUS = ROOT / "tests/liveness-differential.tsv"

# THE CORPUS IS CLOSED, in the sense EXPECTED_THESIS_CONTRACT is closed. It carries the
# VERDICT half of GI-11's precondition — the structural half is
# tests/callgraph-differential.tsv — and production validated only that it was non-empty, so a
# reduced or weakened corpus cleared GI-11 in the release command, which made the least
# closed thing in this file the one holding the most. Same mechanism as the thesis rows:
# the id set both ways, and a full digest over (id, answer, subject, source) — the four
# fields that ARE the contract. `why` and `source-of-truth` are prose for a reviewer and
# are deliberately outside the digest.
EXPECTED_LIVENESS_IDS = frozenset({
    "after-conditional", "dead-caller", "direct", "diverging-if", "false-branch",
    "in-condition", "in-initializer", "in-return-expr", "inside-else", "loop-body",
    "mm-direct-spaced", "mm-diverging-if-renamed", "mm-false-branch-reordered",
    "mm-inside-else-renamed", "mm-via-callee-renamed", "mm-while-true-reordered",
    "self-recursive", "unreferenced", "via-callee", "while-true",
})
EXPECTED_LIVENESS_SHA = \
    "b52d9589773b5e009f4a2738101304c156dd8da6522ecf5ea325d827ddf727cc"

# The STRUCTURAL half of GI-11, pinned the same way. See tests/callgraph-differential.tsv
# for why an `observable` could not carry it: an empty `#[test]` reports `1 passed`.
CALLGRAPH_CORPUS = ROOT / "tests/callgraph-differential.tsv"
EXPECTED_CALLGRAPH_IDS = frozenset({
    "completion-diverges", "completion-returns", "entry-roots", "indirect-declared",
    "indirect-multi-site", "indirect-repeated-site", "order-independent",
    "provenance-binding", "scoped-identity",
})
EXPECTED_CALLGRAPH_SHA = \
    "1326d3ec2ab884276f6e85506af4e4035c2e1f16b904d3d31cc438f83c380ff1"

# EVERY SCORE THIS FILE QUOTES IS DERIVED FROM HERE. Four sentences went on quoting the
# previous corpus size after the corpus grew — including the sentence describing the repair for
# quoting figures nobody measured, four lines from the check itself. A figure written by
# hand is a claim with no owner; `CALLGRAPH_ROWS` is the owner, `NO_SCORE_LITERALS` below
# is the check, and prose that wants to talk about a measurement says "full marks" or
# "zero" rather than a number that can rot.
CALLGRAPH_ROWS = len(EXPECTED_CALLGRAPH_IDS)

# WHICH FILES CARRY A FIGURE ABOUT THIS GATE — DERIVED, NOT LISTED. Round 15's check read
# the gate's residue string; round 16 added the roadmap; round 17 added the script and the
# manifest by hand, and a hand list cannot see a fifth file. Measured when that was pointed
# out: the hand list of four was missing three — the Makefile, scripts/thesis-exit.sh and
# the liveness corpus all cite this gate. So the set is now computed from the tree.
SCORE_SCAN_ROOTS = ("Makefile", "docs", "scripts", "tests")
SCORE_SCAN_SKIP = (".git", "target", "build_output", "node_modules", "__pycache__")
GATE_CITATIONS = ("thesis-exit", "thesis_exit", "callgraph-differential",
                  "liveness-differential")

# Expressions that CONTAIN a score-shaped token and are not scores. Whole expressions, not
# bare tokens: exempting a requirement id's fragment globally would have excused that same
# fragment as a real score anywhere in any
# file, which is a hole the size of the exemption. These are stripped before the scan.
NON_SCORE_EXPRESSIONS = ("TH-03/04/05", "N7-13/15/17")

SCOREBOARD_BEGIN = "<!-- ADVERSARY-SCOREBOARD:BEGIN"
SCOREBOARD_END = "<!-- ADVERSARY-SCOREBOARD:END -->"


def score_bearing_files() -> list[str]:
    """Every tracked text file that cites this gate or one of its corpora.

    DERIVED so a new one cannot be invisible; PINNED by the self-test so a new one cannot
    be silent either. What it does not do: notice a file that quotes a figure without ever
    naming the gate. That file is outside this check, and saying so is cheaper than
    implying a completeness it has not got.
    """
    out = []
    for rel in SCORE_SCAN_ROOTS:
        base = ROOT / rel
        paths = [base] if base.is_file() else sorted(
            q for q in base.rglob("*")
            if q.is_file() and not any(part in SCORE_SCAN_SKIP for part in q.parts))
        for q in paths:
            try:
                text = q.read_text(encoding="utf-8")
            except (UnicodeDecodeError, OSError):
                continue
            if any(c in text for c in GATE_CITATIONS):
                out.append(str(q.relative_to(ROOT)))
    return sorted(out)


# PROSE FIGURES THAT ARE NOT ADVERSARY SCORES, pinned so a NEW one has to be declared.
# The backstop saw `9/9` and was blind to a PERCENTAGE and to an `N of M` — the two forms
# my own round-19 miscount was written in, which is to say it could not see the exact error
# it was built after. (Written as forms, not as the figures: spelling them here would plant
# two real undeclared figures in a file this check reads.) These are the legitimate prose figures in the scanned files today: two
# quotations of claims this repository retracted, and two gate counts that are not scores.
PINNED_PROSE_FIGURES = frozenset({
    "85%", "100%",          # quoted, in the passage listing retracted progress claims
    "1 of 23", "0 of 21",   # gate counts: evaluated thesis rows, stdlib files
    # Added 2026-08-23 with the M2 exit criterion. Both are gate counts and neither is an
    # adversary score, which is the distinction this list exists to make a reviewer state:
    "47 of 47",             # test-xfail: owed rows failing for their DECLARED diagnostic
                            # Falls whenever a row is PAID: feat/m2-expressions paid five
                            # (test_else_if_chain, test_loop_keyword,
                            # test_bitwise_operators, test_compound_assignment_operators,
                            # test_as_cast), and feat/m2-patterns paid three more
                            # (test_match_on_integer_literal with N6-02,
                            # test_use_after_move_is_rejected_without_a_type_annotation,
                            # and the linker's -Werror=return-type row with N6-11).
                            # NO RETIRED FIGURE IS WRITTEN OUT HERE: this file is scanned
                            # for prose figures by its own self-test, and a superseded
                            # count quoted in a comment reads as an undeclared one —
                            # measured, by that check, on the edit that first said so.
    "51 of 82",             # the m2-exit aggregation lattice, under its own inversion control
})


def prose_figures(text: str) -> set[str]:
    """`N%` and `N of M` that are not pinned as non-scores.

    NARROW, AND NAMED FOR IT: it sees two prose forms. "roughly half", "most of them" and
    every other English spelling of a measurement pass, and no list closes that.
    """
    found = {m.group(0) for m in re.finditer(r"\b\d+\s*%", text)}
    found |= {m.group(0) for m in re.finditer(r"\b\d+ of \d+\b", text)}
    return {f for f in found if f.replace(" %", "%") not in PINNED_PROSE_FIGURES}


def score_shaped_tokens(text: str) -> set[str]:
    """Every `n/m` in `text` once the known non-score EXPRESSIONS are removed."""
    for expr in NON_SCORE_EXPRESSIONS:
        text = text.replace(expr, " ")
    return set(re.findall(r"\b\d+/\d+\b", text))


def scoreboard_block(measured: dict) -> str:
    """The label -> score table, rendered from what was MEASURED.

    ATTRIBUTION, WHICH MEMBERSHIP WAS NOT. A figure in prose passed the previous check by
    being equal to some measured score; measured when that was pointed out, the roadmap
    carried a hand-written one that was never produced by the adversary beside it. A
    generated block cannot be hand-written: each figure sits in the row of the adversary
    that produced it, and the whole block is byte-compared against this render.
    """
    rows = "\n".join(f"| {label} | {ok}/{tot} |"
                      for label, (ok, tot) in sorted(measured.items()))
    return (f"{SCOREBOARD_BEGIN} — generated by `make test-thesis-runner`; "
            f"regenerate with `scripts/thesis_exit.py --update-scoreboard`. Do not edit. -->\n"
            f"\n| adversary | score |\n|---|---|\n{rows}\n\n{SCOREBOARD_END}")

# Clauses of GI-11's contract that NO corpus of program outputs can pin, because they are
# properties of the artifact rather than of any program's graph. Named here and printed by
# the gate, so the boundary is disclosed rather than implied away.
# ONE clause, not two. "Fault injection" was on this list and should never have been: it
# is mechanically enforceable, it is what makes the structural corpus unfakeable, and by
# calling it human review the gate removed the only thing standing between a hardcoded
# provider and a pass. It is enforced in callgraph_differential() now.
# ONE boundary, named once. Provenance left this list — it is an obligation on every
# answer now, not a property query. What is left is the thing no finite corpus can reach.
#
# THE SENTENCE THIS USED TO END WITH WAS FALSE BY CONSTRUCTION. It said "these rows
# establish that a WRONG implementation fails" while the same paragraph recorded that a
# normalise -> look up -> re-suffix provider scores FULL MARKS — and that provider is not a
# call-graph implementation at all, so the universal is refuted by the measurement standing
# next to it. Narrowed to what was actually measured: SPECIFIC strategies fail, named and
# each with an executable adversary in the self-test. Every figure quoted below is produced
# by one of those adversaries in this run, and the self-test fails if a figure appears here
# that nothing measured.
GI11_HUMAN_REVIEW_RESIDUE = (
    f"that the provider is a REAL implementation rather than a sufficiently well-informed "
    f"lookup. What these rows reject is specific and measured: an exact-source table of "
    f"all {CALLGRAPH_ROWS} programs AND all {CALLGRAPH_ROWS} mutations scores "
    f"{CALLGRAPH_ROWS}/{CALLGRAPH_ROWS} with the seed pinned but 0/{CALLGRAPH_ROWS} under "
    f"a fresh one; a constant graph, a silent graph, a graph right on the original and "
    f"wrong on every mutation, a graph right on the original and silent on every mutation, "
    f"a correct graph returned without provenance, and a correct graph carrying another "
    f"unit's provenance all score 0/{CALLGRAPH_ROWS}; a graph wrong on exactly ONE "
    f"mutation scores {CALLGRAPH_ROWS - 1}/{CALLGRAPH_ROWS} — while an adversary that "
    f"normalises identifiers, looks up and re-applies the suffix scores "
    f"{CALLGRAPH_ROWS}/{CALLGRAPH_ROWS}, hashing whatever it is handed to satisfy the "
    f"provenance obligation as it goes. A finite, public corpus cannot defeat a reader, "
    f"and the snapshot does not change that: it establishes that the GATE read every "
    f"property from one container one invocation returned, never that the provider "
    f"assembled that container atomically, and never that the graph was DERIVED from the "
    f"bytes. So these rows reject those wrong-answer and exact-table strategies; that the "
    f"provider is an authentic implementation, and that it generalises beyond these "
    f"{CALLGRAPH_ROWS} programs, remain REVIEW OBLIGATIONS and are not established here",
)


def callgraph_provider(source: str):
    """ONE provenance-bearing graph SNAPSHOT for a unit, or None if no graph is wired.

    THE INTERFACE IS `(provenance, {property: value})` — one object per unit, from which
    the gate PROJECTS `edges`, `roots`, `completion` and `indirect` itself.

    WHAT THAT FIXES, and it is the second narrowing of this claim. Round 15 asked
    `provider(unit, prop)` and required each answer to arrive with the digest. Reviewers
    refuted the sentence that shipped with it — "the coupling is what makes a stale graph
    fail" — using the self-test's own `_bound`, which hands any value-only provider a
    correct digest for free: stale VALUES are caught by the ordinary expectation checks,
    never by the digest. Worse, querying properties separately allowed `edges`,
    `completion` and `indirect` to come from INCONSISTENT snapshots wearing the same
    digest, since nothing required them to come from the same object.

    WHAT THIS ESTABLISHES, in one sentence, written from the code outward and narrowed for
    the third time: the GATE makes exactly one provider invocation per distinct unit and
    reads every property from a COPY, taken at return, of the single container that
    invocation returned, and that container arrived labelled with the digest of the bytes
    the gate handed over.

    That is a claim about the RUNNER, and the two things it does not reach are named here
    with the counterexample that demonstrates each, both of which live in the self-test and
    both of which PASS:

      * IT IS NOT ATOMIC ASSEMBLY. Nothing here can know the provider built the container
        from one observation. `_bound` builds it by calling its value source once per
        property, so its parts may be computed from four different states, and it scores
        full marks. The second narrowing's sentence said the answers "cannot be assembled
        from different snapshots"; they can, and this one does.
      * IT IS NOT DERIVATION. A provider that hashes its input while returning a remembered
        graph passes — `_bound` again, on purpose.

    What the copy DOES buy, and it is small and real: post-return mutation of the returned
    dictionary is not observable by the gate, so a provider that hands back a live reference
    into a changing world cannot make two projections disagree. `_mutates_after_return` in
    the self-test demonstrates it, and the copy is one `dict()` call.

    Derivation is the boundary no in-corpus check crosses — it is the residue this gate
    prints — and single-invocation projection plus a checked label is what is left.

    Today nothing is wired, so every row fails — which is correct and is the property an
    `observable` lacked: there is no artifact whose mere existence satisfies this.
    """
    if LIVENESS_MODEL != "call-graph":
        return None
    raise HarnessError("LIVENESS_MODEL is `call-graph` but no callgraph_provider is wired")


def liveness_oracle(src: str, subject: str) -> str:
    """The CURRENTLY WIRED model's answer: live | dead | refused.

    One entry point, so the differential corpus interrogates whatever is wired rather than
    a particular implementation. When GI-11 lands, this dispatches to the call-graph
    reader and the corpus decides whether that reader actually answers correctly.
    """
    if LIVENESS_MODEL == "lexical":
        if unmodellable(src):
            return "refused"
        return "dead" if provably_dead(function_bodies(src), subject) else "live"
    raise HarnessError(f"no liveness oracle wired for LIVENESS_MODEL={LIVENESS_MODEL!r}")


def liveness_differential() -> tuple[list[tuple[str, str, str]], int]:
    """-> (failures as (id, expected, got), total rows).

    THE ESCAPE FROM THE FIFTH RUNG. Every earlier precondition asked whether an artifact
    existed, passed, or looked right — and each was satisfiable by an artifact that did
    nothing, up to and including a renamed wrapper around the probe being replaced. This
    asks for ANSWERS on programs whose correct answers are fixed by review. A wrapper
    cannot pass it: what is compared is the verdict, not the spelling.
    """
    if not LIVENESS_CORPUS.is_file():
        raise HarnessError(f"{LIVENESS_CORPUS} is missing — the differential corpus IS "
                           "GI-11's acceptance test, so its absence is not a pass")
    parsed = []
    failures, total = [], 0
    for n, line in enumerate(LIVENESS_CORPUS.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip() or line.startswith("#"):
            continue
        f = line.split("\t")
        if len(f) != 6:
            raise HarnessError(f"{LIVENESS_CORPUS.name}:{n}: {len(f)} columns, want 6")
        rid, answer, subject, source, _why, _prov = f
        if answer not in ("live", "dead"):
            raise HarnessError(f"{LIVENESS_CORPUS.name}:{n}: answer {answer!r} is not live/dead")
        parsed.append((rid, answer, subject, source))
        total += 1
        got = liveness_oracle(strip_literals(source.replace("\\n", "\n")), subject)
        if got != answer:
            failures.append((rid, answer, got))
    if total == 0:
        raise HarnessError("the differential corpus is empty; an empty corpus passes "
                           "everything, which is the defect it exists to prevent")
    ids = {r[0] for r in parsed}
    if ids != EXPECTED_LIVENESS_IDS:
        raise HarnessError(
            "the liveness corpus changed, and it carries the VERDICT half of GI-11. "
            f"added={sorted(ids - EXPECTED_LIVENESS_IDS) or 'none'} "
            f"removed={sorted(EXPECTED_LIVENESS_IDS - ids) or 'none'}. Re-pin "
            "EXPECTED_LIVENESS_IDS and EXPECTED_LIVENESS_SHA in the same commit.")
    digest = hashlib.sha256(
        "\n".join("\t".join(r) for r in sorted(parsed)).encode()).hexdigest()
    if digest != EXPECTED_LIVENESS_SHA:
        raise HarnessError(
            f"a liveness case's answer, subject or source changed (digest {digest}, "
            f"pinned {EXPECTED_LIVENESS_SHA}). Those answers are the contract: editing one "
            "to make a model pass changes what liveness MEANS. Re-pin deliberately.")
    return failures, total


CALLGRAPH_PROVIDER_OVERRIDE = None   # self-test injection point

# Identifiers the metamorphic family renames at run time. Not `main` (a declared entry
# root, and renaming it changes the property under test) and not builtins.
FAMILY_RENAMEABLE = ("helper", "extra", "ok", "stop", "target", "other", "run", "twice",
                     "Buf", "Log", "len", "f", "g")


def family_instance(source: str, expect: str, expect_mut: str, mutation: str, seed: str):
    """One member of a metamorphic family: the same program, alpha-renamed at RUN TIME.

    WHAT IT IS FOR, narrowed to what it does: IT IS A CHEAP EXACT-SOURCE-TABLE DETECTOR,
    and nothing more. Measured: an adversary who reads this corpus and tabulates the
    correct answer for every program AND every mutation scores full marks against a fixed
    corpus; the same table scores ZERO once the source is renamed with a per-run suffix,
    because the exact source is in no table written in advance.

    ITS CEILING IS ALREADY REACHED, by identifier normalisation: an adversary that strips
    the suffix, looks up, and re-applies it scores full marks again — measured, in the
    self-test, as an executable provider rather than as prose. So this raises the cost from "read the
    corpus" to "implement renaming-invariant lookup" and stops there. It is worth its ten
    lines at that price and is not a step toward proving the analysis exists; the residual
    boundary is named in the gate's output.
    """
    subs = {n: f"{n}_{seed}" for n in FAMILY_RENAMEABLE}

    def rename(text: str) -> str:
        for old, new in sorted(subs.items(), key=lambda kv: -len(kv[0])):
            text = re.sub(rf"(?<![A-Za-z_0-9]){re.escape(old)}(?![A-Za-z_0-9])", new, text)
        return text

    return rename(source), rename(expect), rename(expect_mut), rename(mutation)


def _ask_provider(source: str):
    if CALLGRAPH_PROVIDER_OVERRIDE is not None:
        return CALLGRAPH_PROVIDER_OVERRIDE(source)
    return callgraph_provider(source)


INDIRECT_RESOLVED = "resolved:"
INDIRECT_EMPTY = "(none)"

# A SITE IS A POSITION, NOT A NAME. `<caller>#<n>`: an identifier, optionally `::`-scoped,
# then the 1-based index of the indirect call site within that caller in SOURCE ORDER.
#
# The previous key was `<caller>><callee-expression>`, and a `<callee-expression>` is
# arbitrary source text living inside a string whose delimiters are `,` `=` `;` `|` with no
# escaping — so `a,b()` and `x=y()` could not round-trip and the encoding was not injective.
# Escaping would have worked; a position is better, because it is what a call site IS, and
# because this alphabet CONTAINS NO DELIMITER, which makes injectivity a property of the
# grammar rather than a promise about the inputs.
INDIRECT_SITE_RE = re.compile(
    r"[A-Za-z_][A-Za-z_0-9]*(?:::[A-Za-z_][A-Za-z_0-9]*)*#[1-9][0-9]*\Z")


def indirect_site_wellformed(site: str) -> bool:
    """Is `site` a key this grammar can round-trip? Checked against the corpus's own keys.

    An ill-formed key is NOT a harness failure: it is a wrong answer, and it dies in the
    score like any other. What this exists for is the corpus — a row that pinned a key
    containing a delimiter would be unsatisfiable by construction, and no expectation should
    be impossible to answer.
    """
    return bool(INDIRECT_SITE_RE.fullmatch(site))


def indirect_entries(value: str) -> list[tuple[str, str]]:
    """A site-keyed `indirect` answer, parsed. -> [(site, state)]; `(none)` -> [].

    THE ENCODING WAS SCALAR AND THE CONTRACT IS PER SITE. `resolved:a,b` could not
    distinguish TWO TARGETS FOR ONE SITE from ONE TARGET FOR EACH OF TWO SITES, so "every
    indirect call site" was enforceable only on a program with exactly one — which is the
    only program the corpus had. The answer is now a sorted, comma-separated list of
    `<site>=<state>`, where a site is `<caller>#<n>` (see INDIRECT_SITE_RE) and a state is
    `unresolved` or `resolved:` with `;`-separated targets.

    An entry with no `=` is returned with an empty site, so it is a WRONG ANSWER and dies
    in the score rather than here: the grammar refuses one thing only, and the one thing is
    named in `indirect_grammar_or_raise`.
    """
    s = str(value).strip()
    if s == INDIRECT_EMPTY or not s:
        return []
    out = []
    for entry in s.split(","):
        site, sep, state = entry.strip().partition("=")
        out.append((site.strip(), state.strip()) if sep else ("", entry.strip()))
    return out


def indirect_grammar_or_raise(rid: str, value: str) -> None:
    """THE CONTRACT CLAUSE THAT WAS PINNED AND NOT IMPLEMENTED, now applied PER SITE.

    GI-11's text carries two clauses that read as one — "indirect targets resolved or the
    edge declared unresolved" and "unresolved target = HARNESS FAILURE distinct from
    omission" — and the corpus implemented only the first, so `unresolved` passed and
    nothing anywhere was a harness failure. The pin now states which situation each clause
    names, and this is the second one:

        Every indirect call site is answered either as a RESOLUTION NAMING ONE OR MORE
        TARGETS or as UNRESOLVED — both discharge the obligation, and omitting the call
        site silently is a graph FAILURE — while an answer that claims resolution and names
        NO target is a HARNESS FAILURE distinct from omission, because the gate cannot tell
        a resolution from a declination and refuses to score the row rather than read it as
        either.

    So this raises ONLY on an entry whose state is `resolved:` with an empty target list.
    `unresolved` is a legal state, a wrong or missing entry is a scored row failure, and a
    duplicated site key is a wrong answer too — the grammar refuses ONE situation, the one
    the gate cannot classify, and everything else is left to the score.
    """
    for site, state in indirect_entries(value):
        if not state.startswith(INDIRECT_RESOLVED):
            continue
        if [t for t in state[len(INDIRECT_RESOLVED):].split(";") if t.strip()]:
            continue
        raise HarnessError(
            f"{CALLGRAPH_CORPUS.name}: {rid}: the provider answered {str(value).strip()!r} "
            f"— the call site {site or '(unnamed)'!r} claims to have been RESOLVED and "
            "names no target. The contract permits resolving a site or declaring it "
            "`unresolved`; an answer that is neither cannot be scored as either, so this "
            "is a HARNESS FAILURE. It is distinct from OMISSION: a call site the graph "
            "drops silently is a scored graph failure, a finding about the graph, and is "
            "not this.")


def callgraph_differential() -> tuple[list[tuple[str, str, str, str]], int]:
    """-> (failures as (id, stage, expected, got), total). Closed, and FAULT-INJECTED.

    Every row is checked twice. Comparing one answer to one expected string was the seventh
    rung: a source-keyed stub returning the expected strings scored full marks on every row
    the corpus then had. Requiring the answer to CHANGE when the fact is removed is what a
    lookup table cannot do.

    THE STAGE IS PART OF THE RESULT, because "the score is below total" was the whole
    assertion the mutation branch had, and one working row kept an adversary green while
    six rows failed for reasons nobody named. `stage` is `row` (the corpus itself is
    unusable for this row), `original` (the answer for the submitted unit) or `mutation`
    (the answer for the unit with the fact removed), and the self-test asserts the exact
    per-row set for every injected provider.

    EVERY PROPERTY IS PROJECTED FROM ONE SNAPSHOT PER UNIT. See `callgraph_provider` for
    what that establishes and — equally important — what it does not.
    """
    if not CALLGRAPH_CORPUS.is_file():
        raise HarnessError(f"{CALLGRAPH_CORPUS} is missing — it IS the structural half of "
                           "GI-11's acceptance, so its absence is not a pass")
    parsed, failures = [], []
    for n, line in enumerate(CALLGRAPH_CORPUS.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip() or line.startswith("#"):
            continue
        f = line.split("\t")
        if len(f) != 8:
            raise HarnessError(f"{CALLGRAPH_CORPUS.name}:{n}: {len(f)} columns, want 8")
        rid, prop, source, expect, mutation, expect_mut, _why, _prov = f
        # `provenance` is NOT in this list any more. It was a property a provider could be
        # asked for on its own, which proved it could hash its input and tied that digest
        # to nothing; it is now returned WITH every answer and checked on every query.
        if prop not in ("edges", "roots", "completion", "indirect", "identical-to"):
            raise HarnessError(f"{CALLGRAPH_CORPUS.name}:{n}: unknown property {prop!r}")
        if "=>" not in mutation:
            raise HarnessError(f"{CALLGRAPH_CORPUS.name}:{n}: mutation {mutation!r} is not "
                               "`find=>replace`; without one the row cannot be fault-injected "
                               "and a hardcoded provider passes it")
        parsed.append((rid, prop, source, expect, mutation, expect_mut))
    ids = {r[0] for r in parsed}
    if ids != EXPECTED_CALLGRAPH_IDS:
        raise HarnessError(
            "the call-graph corpus changed. "
            f"added={sorted(ids - EXPECTED_CALLGRAPH_IDS) or 'none'} "
            f"removed={sorted(EXPECTED_CALLGRAPH_IDS - ids) or 'none'}. Re-pin "
            "EXPECTED_CALLGRAPH_IDS and EXPECTED_CALLGRAPH_SHA in the same commit.")
    digest = hashlib.sha256(
        "\n".join("\t".join(r) for r in sorted(parsed)).encode()).hexdigest()
    if digest != EXPECTED_CALLGRAPH_SHA:
        raise HarnessError(
            f"a call-graph expectation changed (digest {digest}, pinned "
            f"{EXPECTED_CALLGRAPH_SHA}). Those values are the contract; re-pin deliberately.")

    by_id = {r[0]: r for r in parsed}

    def decoded(src: str) -> str:
        # The provider gets a PROGRAM. An earlier version handed it the TSV encoding, so a
        # real parser would have received a literal backslash-n.
        return src.replace("\\n", "\n")

    snapshots: dict[str, tuple] = {}

    def snapshot(unit: str):
        """The provider's ONE graph object for a unit. -> (graph, problem). CACHED.

        THE CACHE IS THE MECHANISM, not an optimisation. Asking once per unit is what makes
        "the gate read every property of a unit from one container" true of the GATE rather
        than hoped for of the provider: there is no second call that could return a
        different container wearing the same digest, and the container is copied on receipt
        so it cannot change underneath the projections. The self-test counts the calls and
        pins that there is exactly one per distinct unit. It says nothing about how the
        provider built what it returned — see `callgraph_provider` for the two
        counterexamples that pass.
        """
        if unit in snapshots:
            return snapshots[unit]
        got = _ask_provider(unit)
        if got is None:
            out = (None, "no call graph is wired")
        elif not (isinstance(got, tuple) and len(got) == 2):
            out = (None, f"a bare {type(got).__name__} — the interface is "
                         f"(provenance, graph), so nothing binds these answers to a unit")
        else:
            prov, graph = got
            want = hashlib.sha256(unit.encode()).hexdigest()
            if str(prov) != want:
                out = (None,
                       f"provenance {str(prov)[:12]}…, but the unit submitted hashes to "
                       f"{want[:12]}… — the graph is not bound to the program asked about")
            elif not isinstance(graph, Mapping):
                out = (None, f"a {type(graph).__name__} where the snapshot should be a "
                             f"property map — the properties cannot be shown to come from "
                             f"one object")
            elif [k for k, v in graph.items() if not isinstance(v, str)]:
                # THE VALUES ARE STRINGS, AND THAT IS ENFORCED RATHER THAN HOPED. `dict()`
                # is a SHALLOW copy: a list, dict or custom object as a value stays SHARED
                # with the provider and can change between two projections of the "same
                # snapshot", which is precisely what the copy was introduced to prevent.
                # Constraining the values to an immutable type makes the shallow copy a
                # genuine snapshot instead of one that is only true of the top level.
                bad = sorted(k for k, v in graph.items() if not isinstance(v, str))
                out = (None,
                       f"the snapshot's value(s) for {', '.join(bad)} are not strings — "
                       f"a mutable value stays shared with the provider, so a copy of the "
                       f"map is not a copy of the graph")
            else:
                # A COPY, TAKEN AT RETURN, and a real one now that the values are immutable.
                # The cache held the provider's own mapping, so a provider handing back a
                # live view into a changing world could make two projections of "one
                # snapshot" disagree. This does not make the provider's ASSEMBLY atomic —
                # nothing here can — it makes the gate's READS atomic with respect to what
                # it was given. `Mapping`, not `dict`, so a read-only proxy — the shape a
                # careful provider would hand back — is accepted rather than refused.
                out = (dict(graph), None)
        snapshots[unit] = out
        return out

    def query(unit: str, prop: str):
        """-> (value, problem). PROJECTED from the unit's one snapshot, not asked for."""
        graph, problem = snapshot(unit)
        if problem:
            return None, problem
        value = graph.get(prop)
        if value is None:
            return None, (f"silence — this unit's snapshot answers no `{prop}`; "
                          "a table with no entry for it is not a graph")
        return value, None

    def matches(got, expect: str) -> bool:
        # `a|b` means the contract permits either. Without this a provider that RESOLVES an
        # indirect target failed a row written for one that declares it unresolved — the
        # better implementation losing to the weaker.
        return str(got) in {alt.strip() for alt in expect.split("|")}

    def norm(v):
        # SETS, not strings. Direct comparison of provider values is only sound if
        # canonical sorted serialisation is a provider-interface requirement, and
        # nothing states that — so the claim "EDGE SETS are compared" is made true
        # here instead of assumed.
        return frozenset(x.strip() for x in str(v).split(",") if x.strip())

    seed = os.environ.get("THESIS_FAMILY_SEED") or f"m{secrets.token_hex(3)}"
    for rid, prop, source, expect, mutation, expect_mut in parsed:
        # Draw a fresh family member per run. `identical-to` rows must be renamed with the
        # SAME seed as the row they compare against, which holds because the seed is
        # per-run, not per-row.
        source, expect, expect_mut, mutation = family_instance(
            source, expect, expect_mut, mutation, seed)
        find, _, repl = mutation.partition("=>")
        unit = decoded(source)
        mutated = unit.replace(find, repl)
        if mutated == unit:
            failures.append((rid, "row", expect,
                             f"the row's mutation {find!r} matched nothing — "
                             "it cannot fault-inject anything"))
            continue
        if prop == "identical-to":
            # `<row id>@<edge set>`: equality between the two units AND the pinned value.
            # Comparing the units only to each other let one constant answer satisfy the
            # original stage, because two identical wrong answers are identical.
            ref, _sep, want_edges = expect.partition("@")
            other = by_id.get(ref.rsplit("_", 1)[0] if "_" in ref else ref)
            if other is None:
                failures.append((rid, "row", expect, "names no row in this corpus"))
                continue
            other_src = decoded(family_instance(other[2], "", "", "", seed)[0])
            a, pa = query(unit, "edges")
            b, pb = query(other_src, "edges")
            if pa or pb:
                failures.append((rid, "original", f"edges == {ref}'s edges", pa or pb))
                continue
            if norm(a) != norm(b):
                failures.append((rid, "original",
                                 f"edges == {ref}'s edges ({norm(b)})", str(norm(a))))
                continue
            if want_edges and norm(a) != norm(want_edges):
                failures.append((rid, "original",
                                 f"both units' edges == {want_edges}", str(norm(a))))
                continue
            after, pm = query(mutated, "edges")
            if pm:
                failures.append((rid, "mutation",
                                 f"edges == {expect_mut} once `{find}` is removed", pm))
            elif norm(after) != norm(expect_mut):
                failures.append((rid, "mutation",
                                 f"edges == {expect_mut} once `{find}` is removed",
                                 str(norm(after))))
            continue
        got, problem = query(unit, prop)
        if problem:
            failures.append((rid, "original", expect, problem))
            continue
        if prop == "indirect":
            indirect_grammar_or_raise(rid, got)
        if not matches(got, expect):
            failures.append((rid, "original", expect, str(got)))
            continue
        # THE PINNED OUTCOME, not merely a different one. `!= got` let a constant fallback
        # pass, because a constant is a change; and silence is not an answer at all. Both
        # measured against a hardcoded provider before this was tightened.
        after, problem = query(mutated, prop)
        if problem:
            failures.append((rid, "mutation", f"{expect_mut} once `{find}` is removed",
                             problem))
            continue
        if prop == "indirect":
            indirect_grammar_or_raise(rid, after)
        if not matches(after, expect_mut):
            failures.append((rid, "mutation", f"{expect_mut} once `{find}` is removed",
                             str(after)))
    return failures, len(parsed)


def _load_references(tree, skip: tuple[str, ...] = ()) -> set[str]:
    """Every name USED as a value in `tree`, ignoring the bodies of functions in `skip`.

    A `Name` in Load context is a reference the interpreter will follow. A string
    constant that spells the same name is not, a `def` of it is not, and a comment is
    not in the tree at all — which is the whole difference between this and asking
    whether the file contains some characters.

    `skip` EXISTS BECAUSE THE QUESTION IS ABOUT THE GATE, NOT ABOUT THE TESTS. "TH-03/04/05
    dispatch to the lexical probes" is a claim about the evaluation path. Measured: adding
    one self-test case that calls `p_effect_is_transitive` directly was enough to keep the
    probe "wired" with its real dispatch deleted — the control went green while the thing
    it controls was gone. A check whose sensitivity depends on what the test file happens
    to mention is the same defect as one satisfied by a comment, one layer along.
    """
    import ast as _ast
    skipped = {id(n) for n in _ast.walk(tree)
               if isinstance(n, (_ast.FunctionDef, _ast.AsyncFunctionDef))
               and n.name in skip}
    out = set()

    def walk(node):
        if id(node) in skipped:
            return
        if isinstance(node, _ast.Name) and isinstance(node.ctx, _ast.Load):
            out.add(node.id)
        for child in _ast.iter_child_nodes(node):
            walk(child)
    walk(tree)
    return out


# The self-test is not the dispatch path, and a reference from it is not wiring.
WIRING_SCOPE_SKIP = ("self_test",)

# Where a thesis row's verdict is actually decided. The positive trace starts here.
THESIS_DISPATCH_ROOTS = ("evaluate", "liveness_oracle")


def _dispatch_reaches(tree, roots: tuple[str, ...], target: str) -> bool:
    """Is `target` reachable from any of `roots` through this module's call graph?

    Names loaded inside a function are its out-edges — the same relation
    `_load_references` uses, applied per function and closed transitively. Coarse on
    purpose: it over-approximates reachability, so it can only ever say "a path exists"
    too readily, never too rarely, which is the safe direction for an EXISTENCE check.
    """
    import ast as _ast
    edges, order = {}, []
    for node in _ast.walk(tree):
        if isinstance(node, (_ast.FunctionDef, _ast.AsyncFunctionDef)):
            edges[node.name] = {n.id for n in _ast.walk(node)
                                if isinstance(n, _ast.Name) and isinstance(n.ctx, _ast.Load)}
            order.append(node.name)
    seen, stack = set(), [r for r in roots if r in edges]
    while stack:
        cur = stack.pop()
        if cur in seen:
            continue
        seen.add(cur)
        if target in edges.get(cur, ()):
            return True
        stack.extend(n for n in edges.get(cur, ()) if n in edges and n not in seen)
    return False


def _has_fingerprint_comparison(tree) -> bool:
    """Is `want_fp.strip() != decl.strip()` present AS AN EXPRESSION?

    THE DEFECT THIS EXISTS TO FIX WAS ON THE LINE THAT ASKED THE QUESTION. It read
    `"want_fp.strip() != decl.strip()" in source`, and that line contains the string it
    searches for, so the answer was True whether or not the comparison existed anywhere.
    The check that decides whether this command may compute a verdict at all satisfied
    itself, in the file it was searching, by being written down in it.

    An `ast.Compare` cannot be forged by a string literal: a literal is a `Constant`.
    """
    import ast as _ast

    def stripped_name(node):
        """`X.strip()` -> "X", anything else -> None."""
        if (isinstance(node, _ast.Call)
                and isinstance(node.func, _ast.Attribute)
                and node.func.attr == "strip"
                and isinstance(node.func.value, _ast.Name)):
            return node.func.value.id
        return None

    for n in _ast.walk(tree):
        if not (isinstance(n, _ast.Compare) and len(n.ops) == 1
                and isinstance(n.ops[0], _ast.NotEq)):
            continue
        # AS A SET, BECAUSE `a != b` IS `b != a`. Matching `want_fp` on the LEFT meant a
        # no-op refactor to `decl.strip() != want_fp.strip()` read as "the substring
        # adjudicator is gone" — semantically identical code, and GI-12's guard reported
        # the mechanism retired while `conformance.sh` still matched fingerprints with
        # `grep -qF`. An operand order is not a fact about what the code does.
        if {stripped_name(n.left), stripped_name(n.comparators[0])} == {"want_fp", "decl"}:
            return True
    return False


CONFORMANCE_SH = ROOT / "scripts/conformance.sh"


def substring_attribution_live(gate_source: str, conformance_source: str) -> list[str]:
    """Is rejection attribution STILL decided by fixed-string matching? -> the evidence.

    THE PREVIOUS TEST WAS KEYED ON TWO IDENTIFIERS IN THIS FILE, and review defeated it
    with two semantics-preserving edits, neither a constant:

        _l, _r = want_fp, decl          # no Compare over {want_fp, decl} survives
        if _l.strip() != _r.strip():    # still a string comparison

    plus `ATTRIBUTION_MODEL = "code"`. GI-12 outstanding went to 0 with `grep -qF` fully
    live. My own standard, turned around: an operand ORDER is not a fact about what the
    code does — and a variable NAME is not one either.

    So the decision reads the artifact GI-12's text actually names. `conformance.sh` is what
    adjudicates a rejection, with `grep -qF` over the ANSI-stripped log; while that is how a
    fingerprint is matched, attribution is by substring no matter what this file calls its
    locals. Both signals are consulted and EITHER keeps GI-12 outstanding, because a
    precondition should need every route closed, not any one.

    WHAT THIS STILL CANNOT SEE, said rather than implied: it is a lexical test over a shell
    script. Fixed-string matching reintroduced by some other means — `case` globbing, a
    Python helper, `grep -F` spelled differently — would pass it. It is harder to defeat
    than two identifiers in one file because it reads the file that decides; it is not a
    proof that substring attribution is gone.
    """
    live = []
    if _has_fingerprint_comparison(_ast_module().parse(gate_source)):
        live.append("this gate still compares a corpus-declared fingerprint as a string")
    if "grep -qF" in conformance_source:
        live.append("scripts/conformance.sh still matches a diagnostic by FIXED STRING "
                    "(`grep -qF`), which is what GI-12's text names as the unsound "
                    "adjudicator")
    return live


def wiring_matches_declaration(source: str) -> list[str]:
    """Does the code do what LIVENESS_MODEL / ATTRIBUTION_MODEL say it does?

    THIS IS WHAT STOPS THE CONSTANT FROM BEING THE NEW EMPTY TEST. Declaring
    `LIVENESS_MODEL = "call-graph"` while TH-03/04/05 still dispatch to the lexical
    probes is exactly the defect this whole redesign is about, one level up, so the
    declaration is checked against the dispatch table rather than trusted.

    IT IS READ STRUCTURALLY, NOT AS TEXT, because both halves of it were satisfiable by
    text this file contains for other reasons. The probe names occur eight times — in
    `LIVENESS_PROBES_LEXICAL`, in comments, in their own `def`s and in self-test labels —
    so `"p_has_ref_param" in source` stayed True with every dispatch deleted; and the
    fingerprint half was answered by the line that asked it. Now: a probe counts as wired
    when its name is LOADED as a value somewhere, and the comparison counts as present
    when it is a `Compare` node.
    """
    import ast as _ast
    problems = []
    try:
        tree = _ast.parse(source)
    except SyntaxError as exc:
        raise HarnessError(
            f"wiring_matches_declaration was handed source that does not parse ({exc}); "
            "this check reads structure, and unparseable source is a failure to MEASURE "
            "rather than a wiring finding") from exc
    # Parse the DECLARATION out of the source being checked, so the self-test can hand in
    # a modified copy. Reading the live globals made the check blind to exactly the edit
    # it exists to catch.
    def declared(const, default):
        m = re.search(rf'^{const} = "([a-z-]+)"', source, re.M)
        return m.group(1) if m else default
    liveness = declared("LIVENESS_MODEL", LIVENESS_MODEL)
    attribution = declared("ATTRIBUTION_MODEL", ATTRIBUTION_MODEL)
    referenced = _load_references(tree, skip=WIRING_SCOPE_SKIP)
    lexical_wired = all(p in referenced for p in LIVENESS_PROBES_LEXICAL)
    provider_reached = _dispatch_reaches(tree, THESIS_DISPATCH_ROOTS, "_ask_provider")
    if liveness == "call-graph" and lexical_wired:
        problems.append(
            "LIVENESS_MODEL says `call-graph` but TH-03/04/05 still dispatch to "
            + ", ".join(LIVENESS_PROBES_LEXICAL)
            + ". GI-11 requires REPLACING the probes, not passing a test named after one.")
    if liveness == "call-graph" and not provider_reached:
        # AN ABSENCE WHERE AN EXISTENCE IS REQUIRED FAILS OPEN. Proving the three old NAMES
        # are gone does not prove the new thing is wired: renamed lexical equivalents clear
        # the absence check and nothing consumes the graph that passed the differential.
        # This is the positive half — a structural path from the production dispatch to the
        # provider boundary. It is a reachability claim about THIS module's call graph, not
        # about what the provider computes.
        problems.append(
            "LIVENESS_MODEL says `call-graph` but no call path from the thesis dispatch ("
            + ", ".join(THESIS_DISPATCH_ROOTS) + ") reaches `_ask_provider`. The old names "
            "being absent is not the graph being consumed; an absence check standing where "
            "an existence check is required fails OPEN.")
    if liveness == "lexical" and not lexical_wired:
        problems.append("LIVENESS_MODEL says `lexical` but the lexical probes are not wired")
    substring_wired = _has_fingerprint_comparison(tree)
    if attribution == "code" and substring_wired:
        problems.append(
            "ATTRIBUTION_MODEL says `code` but reject rows are still adjudicated by the "
            "corpus fingerprint declaration, which conformance.sh matches as a substring.")
    if attribution == "substring" and not substring_wired:
        problems.append("ATTRIBUTION_MODEL says `substring` but that comparison is gone")
    return problems


def or_true_assertions(source: str) -> list[str]:
    """`case(...)` predicates containing `… or True` — exactly that, and nothing else.

    NAMED FOR WHAT IT DOES. It was called `constant_assertions` and claimed to reject
    compile-time-constant arguments; it detects an `or` node with a literal `True` operand
    and misses `1`, `not False`, `x is x`, `all([])` and every fixed string. A checker whose
    name is wider than its check is the defect this file has spent thirteen rounds on, so
    the name is narrowed rather than the check widened — the broader set is a real check
    someone can add, and pretending it exists would be worse than not having it.

    Reads the file with `ast`, so it sees the EXPRESSION, not its value: `X or True`
    evaluates to True and is invisible to `case()` by the time it arrives.
    """
    import ast as _ast
    bad = []
    tree = _ast.parse(source)
    for node in _ast.walk(tree):
        if not (isinstance(node, _ast.Call) and getattr(node.func, "id", "") == "case"):
            continue
        if len(node.args) < 2:
            continue
        got = node.args[1]
        for sub in _ast.walk(got):
            if isinstance(sub, _ast.BoolOp) and isinstance(sub.op, _ast.Or):
                if any(isinstance(v, _ast.Constant) and v.value is True for v in sub.values):
                    label = node.args[0]
                    text = label.value if isinstance(label, _ast.Constant) else "<computed>"
                    bad.append(f"line {node.lineno}: {str(text)[:60]!r} — `… or True`")
    return bad


def ctx_for_observable() -> "Context":
    """A real Context for the precondition check — never the self-test's injected one."""
    return Context()


def incomplete_definition(gate_source: str | None = None) -> list[tuple[str, str]]:
    """(requirement id, why no verdict is available). Empty means: a verdict is.

    GI-11 needs BOTH, and neither substitutes for the other:

      * the DIFFERENTIAL CORPUS proves the model's VERDICTS — that it answers correctly on
        programs whose answers are fixed by review;
      * the CALL-GRAPH DIFFERENTIAL proves the model's STRUCTURE — scoped call-site
        identities, declared entry roots, a source-order-independent fixed point,
        per-edge completion, indirect targets resolved-or-declared, every answer bound to
        the digest of the unit it is about — each fault-injected, so AN EXACT-SOURCE TABLE
        OF ORIGINALS, WITH A CONSTANT OR SILENT FALLBACK, FAILS. Not "a lookup table
        fails", which this branch had already refuted in
        tests/callgraph-differential.tsv:98 by measurement: an adversary that normalises
        identifiers, looks up and re-applies the suffix scores FULL MARKS and satisfies the
        provenance obligation on the way past. Two files in one branch asserting opposite
        things about one mechanism, and `check-retracted-claims` could not see it — a
        banned list is a LEXICAL defence, and the retraction was spelled differently. That
        is the standing limit of that mechanism, not an oversight in this instance.
        The liveness corpus touches none of that; an `observable` could not carry it
        either, because an empty `#[test]` reports `1 passed`.

    Making the liveness corpus the WHOLE precondition — which is what round 10 did — let
    GI-11 clear on scalar verdicts while the structure it contracted for was unbuilt.
    """
    validate_preconditions()
    src = (gate_source if gate_source is not None
           else (ROOT / "scripts/thesis_exit.py").read_text())
    why = {rid: w for rid, _c, _u, _s, w in PRECONDITIONS}
    out = []
    # GI-11's CONSTANT HALF, no longer `continue`d past. A precondition whose only test was
    # its corpora could be declared met by a corpus that passes; the model in use is part
    # of the stop condition, and a label can only ADD a refusal here — never lift one,
    # because the corpora below must pass as well.
    if LIVENESS_MODEL != "call-graph":
        out.append(("GI-11", why["GI-11"]))
    try:
        cg_fail, cg_total = callgraph_differential()
    except HarnessError as e:
        # A failure to MEASURE, reported as such: this is the branch an ungrammatical
        # indirect answer lands in, and it reads differently from "the graph fails N rows".
        out.append(("GI-11", f"the call-graph differential could not be run: {e}"))
        cg_fail, cg_total = [("(unrun)", "row", "", "")], 0
    if cg_fail:
        shown = ", ".join(f"{rid} ({stage}): want {want}, got {got}"
                          for rid, stage, want, got in cg_fail[:2])
        out.append(("GI-11",
                    f"the wired graph fails {len(cg_fail)} of {cg_total} STRUCTURAL cases in "
                    f"tests/callgraph-differential.tsv — {shown}"
                    f"{' …' if len(cg_fail) > 2 else ''}. This pins what the graph RETURNS "
                    f"(scoped identities, roots, order-independence, per-edge completion, "
                    f"indirect resolved-or-declared, every answer carrying the digest of "
                    f"the unit it is about), and defeats an exact-source table of "
                    f"ORIGINALS with a constant or silent fallback — NOT lookup in "
                    f"general, which the corpus header records as measured and unbeaten; "
                    f"an `observable` could not, because an empty #[test] reports "
                    f"`1 passed`"))
    try:
        failures, total = liveness_differential()
    except HarnessError as e:
        out.append(("GI-11", f"the liveness differential could not be run: {e}"))
        failures, total = [("(unrun)", "", "")], 0
    if failures:
        shown = ", ".join(f"{rid}: want {want}, got {got}" for rid, want, got in failures[:3])
        out.append(("GI-11",
                    f"the wired liveness model fails {len(failures)} of {total} cases in "
                    f"tests/liveness-differential.tsv — {shown}"
                    f"{' …' if len(failures) > 3 else ''}. Those answers are fixed by "
                    f"review, so a model that disagrees with them is wrong, whatever it "
                    f"is called"))
    # GI-12 IS DECIDED BY THE MECHANISM, NOT BY A LABEL.
    #
    # It used to be `globals()[const] != sound`, where `sound` is the fourth field of a
    # tuple in this file and `validate_preconditions()` compared it against a pin in the
    # same file — PIN AGAINST PIN. Dual-edit both to "substring" and GI-12 read as MET
    # while the substring adjudicator was still the thing deciding every reject row. "Met"
    # meant TWO LABELS AGREE, which is not a fact about the gate.
    #
    # The physical stop condition instead: GI-12 is unmet for as long as the substring
    # fingerprint comparator is still present as an EXPRESSION in this gate's source. No
    # constant participates, so no edit to a constant can retire it; the declaration is
    # still checked against the same physical fact by `wiring_matches_declaration`, which
    # is what makes a mismatched label a harness error rather than a silent lift.
    conf = CONFORMANCE_SH.read_text() if CONFORMANCE_SH.is_file() else ""
    live = substring_attribution_live(src, conf)
    if live:
        out.append(("GI-12", why["GI-12"] + " — still live: " + "; ".join(live)))
    return out


def _ast_module():
    import ast as _ast
    return _ast


AGGREGATE_ROW = "D1-01"          # cites this command as its evidence: it is the summary

# THE COMPLETE THESIS CONTRACT, not just its ids. Pinning ids alone left three ways to
# change the definition of 1.0 without tripping anything: retype a row to another
# DISPATCHED kind (reject -> fixture turns a negative test into a positive one), point it
# at a different fixture, or blank its required fingerprint back to `-` — which made
# `p_verdict` skip the fingerprint comparison entirely and reopened the exact hole the
# ninth column was added to close. id -> (kind, evidence, fingerprint).
# MF5: the requirement TEXT is pinned for the rows whose text IS the contract. Pinning
# only (kind, evidence, fingerprint) left GI-11's detailed acceptance criteria — the thing
# that makes it more than a name — weakenable with no harness error.
# MF4: the EXACT normalized acceptance text, by digest. Pinning selected substrings let
# the indirect-target clause go unpinned entirely, and let a pinned phrase survive inside
# NEGATED prose ("does not require scoped call-site identities" contains the phrase). A
# full SHA-256 over the whole normalized field has neither hole: any edit at all fails, and the
# fix is to re-pin deliberately in the same commit.
# The rows whose TEXT is the contract, pinned as a SET so the map cannot lose one quietly.
PINNED_ACCEPTANCE_IDS = frozenset({"GI-11", "GI-12"})

PINNED_ACCEPTANCE_SHA = {
    # Re-pinned three times, deliberately, and the reasons are the review record. Round 15
    # settled the indirect contract (two situations, not one) and named the projection the
    # queries range over. Round 16: provenance is ONE SNAPSHOT PER UNIT rather than a tag on
    # each answer — the per-answer form proved only that the provider could hash its input,
    # and let properties come from inconsistent snapshots — and the indirect clause is
    # answered BY SITE KEY, because a scalar answer cannot address one site of two.
    # Round 17: the snapshot clause is stated as what the RUNNER does (one invocation,
    # projection from a copy) because the stronger reading was never the code's to make, and
    # the site key is a POSITION, because a key built from source text cannot round-trip
    # through a delimiter-separated string with no escaping. The pin moves when the contract
    # moves, and only then.
    "GI-11": "8db9bb09306b660921cfbfc96d1d17bae7beebfb0ade3450772f81fe0ade3471",
    "GI-12": "f14b04ad415e5ee436829fef4f7b4c4865f26df29bc45f30dd7140caad7cea3a",
}


def _validate_pin_keys(keys) -> None:
    """The acceptance-pin key SET is itself pinned. Removing a key is not "unpinned by
    design"; it is unpinned in silence, which is the shape this file has spent 23 rounds on."""
    if set(keys) != PINNED_ACCEPTANCE_IDS:
        raise HarnessError(
            f"the acceptance-pin key set changed: {sorted(keys)} against the reviewed "
            f"{sorted(PINNED_ACCEPTANCE_IDS)}. A row whose TEXT is the contract cannot stop "
            "being pinned by deleting a dictionary entry.")


def case_pin_is_real(sha: str | None) -> bool:
    """Is the case-inventory pin an actual digest? Blank USED TO MEAN OFF, silently, while
    the summary line went on printing `the set is pinned`."""
    return bool(re.fullmatch(r"[0-9a-f]{64}", sha or ""))


def acceptance_digest(text: str) -> str:
    return hashlib.sha256(" ".join(text.split()).encode()).hexdigest()

EXPECTED_THESIS_CONTRACT = {
    "D1-01": ("gate", "make thesis-exit", "-"),
    "N7-01": ("reject", "tests/reject/async_fn.pd", "there is no `async` keyword"),
    "N7-02": ("reject", "tests/10_async_await.pd", "there is no await operator"),
    "N7-04": ("fixture", "tests/09_effects_propagate.pd", "-"),
    "N7-08": ("reject", "tests/reject/pure_function_calls_io.pd", "declared pure"),
    "N8-01": ("fixture", "tests/13_total_attribute.pd", "-"),
    "N8-06": ("fixture", "tests/13_structural_recursion.pd", "-"),
    "N8-08": ("reject", "tests/reject/total_unproven.pd", "cannot prove termination"),
    "N9-01": ("fixture", "tests/05_ref_shared.pd", "-"),
    "N9-03": ("fixture", "tests/05_ref_named_region.pd", "-"),
    "N9-04": ("reject", "tests/reject/lifetime_param_list.pd", "lifetime parameter list"),
    "N9-06": ("reject", "tests/reject/ambiguous_region.pd", "ambiguous region"),
    "SH-01": ("gate", "make selfhost", "-"),
    "SH-02": ("gate", "make selfhost-corpus", "-"),
    "SH-03": ("gate", "make selfhost-corpus", "-"),
    "SH-04": ("gate", "make selfhost-corpus", "-"),
    # SH-05 joins the definition because SH-01 — the ONE thesis row that asserts a RESULT
    # rather than an obligation — is true under a condition the table did not state: the
    # self-hosting unit imports nothing, so the unspecified emission order of imported
    # modules never reaches the output. The thesis is that the fixed point survives the
    # REWRITE, and a rewrite in the differentiated dialect will use modules.
    "SH-05": ("gate", "make selfhost-determinism", "-"),
    "TH-01": ("gate", "make thesis-exit", "-"),
    "TH-02": ("gate", "make thesis-exit", "-"),
    "TH-03": ("gate", "make thesis-exit", "-"),
    "TH-04": ("gate", "make thesis-exit", "-"),
    "TH-05": ("gate", "make thesis-exit", "-"),
    "TH-06": ("gate", "make thesis-exit", "-"),
    "WT-02": ("fixture", "tests/witness/json_parser.pd", "-"),
    # The two safeguards for this gate's OWN weaknesses. They were ordinary `1.0` rows,
    # so `make thesis-exit` could have gone green while rejection attribution was still
    # satisfiable by incidental text and while this lexical model was still in use — the
    # careful placement (M2, M3-start) defeated by not being preconditions. They are
    # thesis rows now: 1.0 cannot be declared with either outstanding.
    # A `gate` row pointing at this command, like D1-01: GI-11 is adjudicated by the two
    # differentials, not by a named test. It carried an `observable` locator that `evaluate`
    # skipped, so the Rust test it named need not have existed for the gate to clear. This
    # tuple is where that fact is now decidable, and it is compared to the manifest on
    # every run — the file-name token that used to stand in for it was prose.
    "GI-11": ("gate", "make thesis-exit", "-"),
    # NOT a `reject` row: a reject row is adjudicated by the substring matcher this
    # requirement exists to replace, so it could never have proved itself.
    "GI-12": ("gate", "make check-diagnostic-codes", "-"),
}
EXPECTED_THESIS_IDS = frozenset(EXPECTED_THESIS_CONTRACT)


def _validate_contract(contract=None):
    """The pin must itself be well formed.

    Comparing the manifest against the pin cannot catch a defect IN the pin. If a reject
    row were pinned with `-`, the manifest would match, the comparison would pass, and
    `p_verdict` would skip the fingerprint check — rejection-at-the-wrong-reason, reopened
    from the one direction the comparison cannot see.
    """
    contract = EXPECTED_THESIS_CONTRACT if contract is None else contract
    for rid, (kind, ev, fp) in sorted(contract.items()):
        if kind not in KINDS:
            raise HarnessError(f"pinned contract: {rid} has unknown kind {kind!r}")
        # `fp.strip()`, not `fp`. Measured: `" "` passed this guard, and `p_verdict`
        # compares `want_fp.strip() != decl.strip()`, so a corpus row declaring nothing
        # then satisfied it — the same end state the guard names, reached by a value it
        # did not consider. `-` and `""` were refused; whitespace was not.
        if kind == "reject" and fp.strip() in ("", "-"):
            raise HarnessError(
                f"pinned contract: {rid} is a `reject` row with no required fingerprint. "
                "Any rejection would satisfy it, including one for incidental unsupported "
                "syntax.")
        if kind != "reject" and fp != "-":
            raise HarnessError(f"pinned contract: {rid} is {kind} but carries a fingerprint")
KINDS = {"fixture", "reject", "skip", "observable", "gate", "decision"}
REQUIRED_VERDICT = {"fixture": "PASS_VERIFIED", "reject": "REJECTED", "skip": "SKIP"}

WITNESSES = ("bootstrap/pdc.pd", "tests/witness/json_parser.pd")

# N14's effectful set. `string_*`, `char_*` and `int_to_string` are pure and deliberately
# absent: a caller of those is not evidence of an IO effect.
#
# `file_open_ex`, `file_close_ex`, `file_read_ex` and `file_write_ex` WERE members and
# are gone, 2026-08-23. They were never N14's — this comment said "N14's effectful set"
# while four of its members were names the specification does not define — and they have
# now left src/builtins.rs as well, so nothing can call one. A name here that no program
# can name classifies nothing; leaving it would make this set the place a deleted builtin
# lives on.
IO_BUILTINS = frozenset({
    "print", "print_int", "panic",
    "file_open", "file_read_all", "file_read_line", "file_write", "file_close",
    "file_exists", "file_flush", "file_seek",
    "path_exists", "path_is_file", "path_is_dir",
    "create_dir", "create_dir_all", "remove_file", "remove_dir", "remove_dir_all",
    "read_file_to_string", "write_string_to_file", "arg_count", "arg_at",
})

GREEN, RED, GREY, OFF = "\033[0;32m", "\033[0;31m", "\033[0;90m", "\033[0m"


# ---------------------------------------------------------------------------
# Lexing. Deliberately models THE COMPILER, not the specification.
# ---------------------------------------------------------------------------
CHAR_LITERAL = re.compile(r"'(?:\\.|[^\\'])'")


def strip_literals(text: str) -> str:
    """Blank string and char literals and comments; KEEP lifetime ticks.

    `'` is ambiguous — it opens a char literal AND introduces a lifetime. Treating every
    `'` as a quote consumes from the tick to end of file and the lifetime probe can then
    never fire. A char literal is `'x'` or `'\\x'`; anything else starting `'` is a tick.

    Block comments NEST, because the compiler that reads the sources this gate scores
    nests them: N2-08 landed, and `slash_or_comment` in src/lexer/token.rs counts depth.
    THE GATE MODELS THE COMPILER, so it flipped in the same commit — a gate that did not
    nest would now disagree with the compiler about whether a real `async` is commented
    out, which is the same disagreement in the other direction.

    The compiler this models is the RUST `pdc`, which is what compiles bootstrap/pdc.pd
    and the witness. `bootstrap/pdc.pd`'s own hand-written scanner still stops at the
    first close with no depth counter; that divergence is recorded in
    docs/specification/bootstrap-subset.md rather than papered over here, and it is not
    observable: no PBS-1 source contains a nested comment, so the two scanners agree on
    every input that exists.
    """
    out: list[str] = []
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        if c == '"':
            i += 1
            while i < n and text[i] != '"':
                i += 2 if text[i] == "\\" else 1
            i += 1
            out.append(" ")
        elif c == "'":
            m = CHAR_LITERAL.match(text, i)
            if m:
                i = m.end()
                out.append(" ")
            else:
                out.append(c)
                i += 1
        elif text.startswith("//", i):
            while i < n and text[i] != "\n":
                i += 1
        elif text.startswith("/*", i):
            # Depth-counted, matching the compiler. A regular expression cannot do
            # this, which is why the compiler needed a callback and why this needed a
            # loop. An unterminated comment consumes to end of file rather than
            # raising: the compiler reports it as an error, and this scanner's only
            # question is "is this token live source", for which "no" is the right
            # answer either way.
            depth, j = 1, i + 2
            while j < n and depth:
                if text.startswith("/*", j):
                    depth += 1
                    j += 2
                elif text.startswith("*/", j):
                    depth -= 1
                    j += 2
                else:
                    j += 1
            i = j
            out.append(" ")
        else:
            out.append(c)
            i += 1
    return "".join(out)


def read_source(root: Path, rel: str) -> str:
    p = root / rel
    if not p.exists():
        raise Absent(f"{rel}: not in the repository")
    if not p.is_file():
        raise HarnessError(f"{rel}: exists but is not a file")
    try:
        raw = p.read_text(encoding="utf-8", errors="replace")
    except OSError as exc:
        raise HarnessError(f"{rel}: unreadable ({exc})") from exc
    if not raw.strip():
        raise HarnessError(f"{rel}: empty — refusing to call an empty file clean")
    src = strip_literals(raw)
    if not src.strip():
        raise HarnessError(f"{rel}: nothing survives lexing")
    return src


# ---------------------------------------------------------------------------
# Source probes. Pure functions of text; the gate and its self-test share exactly
# these, so there is no second implementation to diverge.
# ---------------------------------------------------------------------------
ASYNC_TOKEN = re.compile(r"(?:^|[^A-Za-z_0-9])(async|await)(?:[^A-Za-z_0-9]|$)")

# `ref<'a> T` is the ONE place N9 permits a region name, so it is exempt — with an
# identifier boundary, because without one `myref<'a>` was rewritten to `my` and the
# forbidden list vanished. WHITESPACE IS INSIGNIFICANT between tokens: grammar.ebnf:157
# is `generic_params = '<' generic_param …`, and `fn f< 'a>(x: i64)` compiles today, so
# an adjacency-only `<'` misses a real lifetime parameter list.
REF_REGION = re.compile(r"(?<![A-Za-z_0-9])ref\s*<\s*'[A-Za-z_0-9]*\s*>")
LIFETIME_LIST = re.compile(r"<\s*'")

# grammar.ebnf:119-120 is `"fn" identifier [ generic_params ] '('`, so the generic
# parameter list is OPTIONAL AND MUST BE MATCHED. Without it `fn generic<T>(x: T)`
# matched nothing at all: the function was not in `bodies`, so it was neither a
# reachable target nor a caller — invisible in both directions.
FN_HEADER = re.compile(
    r"(?<![A-Za-z_0-9])fn\s+([A-Za-z_][A-Za-z_0-9]*)\s*(<[^(){}]*>)?\s*\(")
REF_PARAM = re.compile(r":\s*ref(?:\s*<[^>]*>)?(?:\s+mut)?\s+[A-Za-z_\[(]")
# `#[total]`, NOT `#![total]`. The crate-level form is a different requirement (N8-02);
# TH-04 and the manifest both ask for a FUNCTION-level attribute, and accepting `#!` let a
# crate carrying only the crate-level form satisfy it.
TOTAL_ON_FN = re.compile(
    r"#\[\s*total\s*(?:\([^)]*\))?\s*\]\s*(?:#\[[^\]]*\]\s*)*(?:pub\s+)?fn\s+([A-Za-z_][A-Za-z_0-9]*)"
)
CALL = re.compile(r"([A-Za-z_][A-Za-z_0-9]*)\s*\(")
EFFECT_LINE = re.compile(r"Function '([A-Za-z_][A-Za-z_0-9]*)' has effects:\s*(.+)")
IO_EFFECT = re.compile(r"\b(?:IO|Io|io)\b")


def balanced_span(src: str, open_at: int, opener="(", closer=")") -> str:
    depth, i, n = 0, open_at, len(src)
    while i < n:
        if src[i] == opener:
            depth += 1
        elif src[i] == closer:
            depth -= 1
            if depth == 0:
                return src[open_at + 1:i]
        i += 1
    return ""


def duplicate_function_names(src: str) -> list[str]:
    """Names defined more than once. `bodies` is keyed by BARE NAME, so two functions
    sharing one — trivially, two `impl` blocks with a `len` each — silently overwrite,
    and every edge into the loser vanishes. Detected, not tolerated (R4)."""
    seen, dupes = set(), []
    for m in FN_HEADER.finditer(src):
        if m.group(1) in seen:
            dupes.append(m.group(1))
        seen.add(m.group(1))
    return sorted(set(dupes))


def function_bodies(src: str) -> dict[str, str]:
    out: dict[str, str] = {}
    for m in FN_HEADER.finditer(src):
        after = src.find(")", m.end() - 1)
        if after < 0:
            continue
        brace = src.find("{", after)
        if brace < 0:
            continue
        out[m.group(1)] = balanced_span(src, brace, "{", "}")
    return out


def callees(body: str, exclude: str = "") -> set[str]:
    """IDENTIFIERS IMMEDIATELY FOLLOWED BY `(`, minus the function's own name.

    NOT "names called", which is what this said. Measured: `return (1 + 2);` yields
    `return` and `if (x) { }` yields `if`. The set is an over-approximation of the call
    names in a body.

    Consequence, audited rather than assumed: the only consumer is
    `p_effect_is_transitive`, which uses each candidate to look up a BODY and ask whether
    that body calls an IO builtin. A spurious candidate has no body, so `direct_io` is
    empty and it cannot manufacture a green edge — the over-approximation is benign THERE.
    It is not licensed anywhere else, and a second consumer would have to re-do this
    audit.

    A self-edge is not reachability: `#[total] fn r(n) { return r(n); }` was "live"
    because its own name appeared in a body — its own.
    """
    return {c for c in CALL.findall(body) if c != exclude}


def p_no_async_token(src: str) -> tuple[bool, str]:
    m = ASYNC_TOKEN.search(src)
    return (False, f"found `{m.group(1)}`") if m else (True, "no async/await token")


def p_no_lifetime_param_list(src: str) -> tuple[bool, str]:
    if LIFETIME_LIST.search(REF_REGION.sub("ref", src)):
        return False, "a lifetime parameter list survives"
    return True, "none"


def p_has_ref_param(src: str) -> tuple[bool, str]:
    """A `ref` / `ref mut` PARAMETER on a function SOME OTHER BODY IN THE UNIT MENTIONS.

    THE SUMMARY LINE SAID "reachable from `main`" FOR SIX ROUNDS AND THE CODE NEVER
    DECIDED THAT. Separating input, constructed rather than argued:

        fn main() { }
        fn dead_caller() -> i64 { return helper(); }
        fn helper(x: ref String) -> i64 { return 1; }

    `helper` is unreachable from `main` — its only caller is itself unreachable — and this
    returns GREEN, because `provably_dead` asks whether any other body mentions the name,
    which `dead_caller`'s does. The wording was the same retracted property round 18 found
    in the roadmap, surviving here in the probe it describes, spelled differently enough to
    slip the banned phrases. Both spellings are banned now.

    What the rule buys is stated where it is true: a parameter that NOTHING in the unit
    names is an ornament, and that is refutable (P2). "On a live path" is GI-11's job.
    """
    require_modellable(src, "TH-03 / `ref` parameter")
    bodies = function_bodies(src)
    dead = []
    for m in FN_HEADER.finditer(src):
        if not REF_PARAM.search(balanced_span(src, m.end() - 1)):
            continue
        if not provably_dead(bodies, m.group(1)):
            return True, (f"fn {m.group(1)} declares one — P1 existence; liveness is NOT "
                          f"asserted (GI-11)")
        dead.append(m.group(1))
    if dead:
        return False, (f"`ref` parameters exist only on provably dead function(s) — nothing "
                       f"in the unit names them (P2): {', '.join(sorted(set(dead)))}")
    return False, "no `fn` declares a `ref` / `ref mut` PARAMETER"


# --- WHAT THESE PROBES PROVE, AND WHAT THEY DO NOT ----------------------------------
#
# THIS IS THE FOURTH DESIGN FOR THIS QUESTION AND THE FIRST THAT DOES NOT GUESS.
# The three before it — over-approximate, fold `if false`, certain-reachability — each
# shipped with a fail-open path, and the last had three at once:
#
#     if true { return 1; }  ornament();     the `if` diverges; ornament looked certain
#     while true { }         ornament();     the loop never ends; ornament looked certain
#     let f = |x| ornament();                an expression-bodied closure has no braces
#                                            to delete, so its call sat at depth 0
#
# Deleting nested bodies removes their CONTENTS, not their CONTROL-FLOW EFFECT. And
# divergence is interprocedural: `a(); b();` does not run `b` if `a` panics. A lexical
# model cannot decide reachability in a language with divergence and closures, and three
# attempts is enough evidence that a fourth heuristic would have a fourth hole.
#
# So these probes no longer claim reachability. They claim two things, and each has a
# failure direction that is PROVED rather than argued:
#
#   P1  EXISTENCE — what a green verdict means. The construct appears in the source.
#       That is a question about text, decided lexically. A green TH-03 means "a `ref`
#       parameter is declared", NOT "a `ref` parameter is on a live path".
#
#   P2  REFUTATION — what a RED verdict may additionally mean. If the construct appears
#       only inside a function whose name occurs nowhere else in the unit, that function
#       is dead: with no syntactic reference and no indirect dispatch (R4 refuses that
#       case) nothing can call it. Sound in the direction it is used. The converse —
#       "referenced, therefore live" — is NOT claimed.
#
#   R4  THESE constructs are a HARNESS ERROR (exit 2), because each could create a call
#       this model cannot see: a closure in ANY form, a function-typed parameter, a
#       `.`-method call, `T::m(…)` through a declared type parameter, and two functions
#       sharing a name. An earlier version left some closure forms undetected and
#       justified it by saying their bodies were nested and therefore excluded — false for
#       exactly the expression-bodied form, which has no braces at all.
#
#       THE LIST IS NOT A COMPLETENESS PROOF, and the sentence above used to read as one.
#       What P2 needs is weaker than "R4 catches every indirect call": it needs that a
#       function called anywhere must have its NAME appear in some other body. Measured,
#       by construction rather than by assertion: `let f = helper; f();` is NOT refused by
#       R4 — and P2 still does not fire, because `helper` is named in `main`. The same held
#       for every construction tried (a binding, an indexed call through a table). What
#       would break P2 is a call whose target is never named in any `fn` body — expansion
#       from a user macro body, a linker-visible export, reflection — and this language has
#       none of the first two today (macros are builtins; `extern` is rejected) while the
#       third does not exist. THAT IS AN ARGUMENT, NOT A PROOF: I could not construct a
#       counterexample, which is not the same as there being none, and it is written down
#       here so the next reader attacks it rather than trusting it.
#
# WHAT IS NO LONGER ASSERTED, AND WHERE THAT OBLIGATION NOW LIVES.
# Liveness. The gate does not certify that a differentiator is used on a path the program
# runs. That is GI-11 — the compiler exports a release-grade call graph and the gate
# consumes it — and GI-11 is now a THESIS row, so `make thesis-exit` cannot reach green
# while this weaker model is in use. The anti-ornament property is not abandoned; it is
# moved to a mechanism that can prove it and made a precondition of 1.0.

# ANY `|` TOKEN, and the claim is now true in both directions. The bounded-pair form was
# wrong twice at once: `a || b` — the ordinary logical-or — matched it and produced a
# false refusal, and a parameter list longer than the bound escaped it and produced a
# false answer. grammar.ebnf:384 puts a closure behind `|`, and `|` has no other use in
# the language today (bitwise-or is unimplemented, A2), so refusing on the token is exact
# now and conservative later: when bitwise-or lands, this refuses programs it need not,
# which is a wrong exit-2 rather than a wrong verdict, and it is GI-11's job to remove it.
CLOSURE_ANY = re.compile(r"\|")
FN_TYPE_PARAM = re.compile(r":\s*fn\s*\(")
METHOD_CALL = re.compile(r"\.\s*[A-Za-z_][A-Za-z_0-9]*\s*\(")

UNMODELLABLE = (
    (CLOSURE_ANY, "a closure in any form — R4"),
    (FN_TYPE_PARAM, "a function-typed parameter — R4"),
    (METHOD_CALL, "a `.`-method call — R4"),
)

GENERIC_PARAMS = re.compile(
    r"(?<![A-Za-z_0-9])fn\s+[A-Za-z_][A-Za-z_0-9]*\s*<([^(){}]*)>\s*\(")


def unmodellable(src: str) -> list[str]:
    """Constructs that could create an invisible call. Non-empty means: do not answer."""
    found = [why for pat, why in UNMODELLABLE if pat.search(src)]
    dupes = duplicate_function_names(src)
    if dupes:
        found.append(f"two functions sharing a name ({', '.join(dupes)}) — R4")
    params = set()
    for m in GENERIC_PARAMS.finditer(src):
        for raw in m.group(1).split(","):
            name = raw.split(":")[0].strip().lstrip("'")
            if name and not raw.strip().startswith("'"):
                params.add(name)
    for tp in sorted(params):
        if re.search(rf"(?<![A-Za-z_0-9]){re.escape(tp)}::\s*[A-Za-z_]", src):
            found.append(f"dispatch through the type parameter `{tp}::…` — R4")
    return found


def require_modellable(src: str, what: str) -> None:
    found = unmodellable(src)
    if found:
        raise HarnessError(
            f"{what}: this gate cannot see the call edges of " + "; ".join(found)
            + ". Refusing to answer — see the P1/P2/R4 contract in scripts/thesis_exit.py "
              "and GI-11, which replaces this model with the compiler's own call graph.")


def provably_dead(bodies: dict[str, str], name: str) -> bool:
    """P2, NARROWED. True only when no other body mentions this name AT ALL.

    The earlier wording claimed "nothing in the unit names this function" while deciding
    it by searching for `name(` — a CALL. A bare reference used as a function value, an
    alias, a callback argument or a macro argument names the function and can invoke it,
    and every one of those was refuted as dead. Function types are 1.0 (requirement
    N4-14), so that is not post-1.0 syntax.

    So the search is now for the NAME, in any position. A refutation must be sound, and
    the conservative direction for a refutation is to refuse to fire: a bare mention is
    treated as possible use, not as absence. What this decides is therefore narrower than
    reachability and narrower than the old wording — it is "no other function body
    mentions this identifier", nothing more. `main` is an entry root and is never dead.

    THE PREMISE THAT WAS UNSTATED, AND IS NOW ENFORCED. "Nothing can call it" holds only
    if THIS UNIT IS THE WHOLE PROGRAM. Constructed:

        pub fn exported(x: ref String) -> i64 { return 1; }

    a library unit whose function is named nowhere else — `provably_dead` returned True,
    and an exported function called from another compilation unit is not dead. Every
    witness this gate reads today is a whole program, so the refutation was sound in
    practice and unsound in principle; a premise that holds by luck is a premise nobody
    is checking. A unit with no entry root is now a HARNESS ERROR: the model cannot say
    what is dead in a fragment, and refusing is the answer it is entitled to.
    """
    if "main" not in bodies:
        raise HarnessError(
            "P2 (the dead-code refutation) assumes the unit under test is the WHOLE "
            "PROGRAM: 'no other body mentions this name' means 'nothing can call it' only "
            "when there is no other body anywhere. This unit declares no `fn main`, so it "
            "may be a fragment whose functions are called from outside it, and the "
            "refutation is refused rather than guessed.")
    if name == "main":
        return False
    for other, body in bodies.items():
        if other != name and re.search(rf"(?<![A-Za-z_0-9]){re.escape(name)}(?![A-Za-z_0-9])",
                                       body):
            return False
    return True

def p_total_on_fn(src: str) -> tuple[bool, str]:
    """A `#[total]` on a function that is not provably dead (P1 existence + P2).

    "Appears in some body" was not reachability: a dead caller satisfied it, and so did
    the function's own recursive call. Both are the ornament class one level in.
    """
    names = [m.group(1) for m in TOTAL_ON_FN.finditer(src)]
    if not names:
        return False, "no `#[total]` attached to a `fn`"
    require_modellable(src, "TH-04 / #[total]")
    bodies = function_bodies(src)
    live = [n for n in names if not provably_dead(bodies, n)]
    if not live:
        return False, (f"`#[total]` only on provably dead function(s) — nothing in the unit "
                       f"names them (P2): {', '.join(names)}")
    return True, (f"#[total] on {', '.join(live)} — P1 existence; liveness is NOT asserted "
                  f"(GI-11)")


def p_effect_is_transitive(report: str, src: str) -> tuple[bool, str]:
    """An IO effect must reach a caller that performs NO IO itself — via a NAMED edge.

    Two weakenings are closed here. Matching any `has effects` line with an IO spelling
    passed on a DIRECT effect (bootstrap/pdc.pd:49-51 calls `file_write`), which is not
    propagation. And returning true for any reported function with no recognised builtin
    call passed for a function that calls NOTHING — a fabricated or over-approximated
    report then proved the property. The edge caller -> callee -> builtin is exhibited.
    """
    require_modellable(src, "TH-05 / effect-propagation reachability")
    bodies = function_bodies(src)
    reported = {}
    for line in report.splitlines():
        m = EFFECT_LINE.search(line)
        if m and IO_EFFECT.search(m.group(2)):
            reported[m.group(1)] = m.group(2).strip()
    if not reported:
        return False, "the compiler reported no function with an IO effect"

    def direct_io(name: str) -> set[str]:
        return {c for c in CALL.findall(bodies.get(name, "")) if c in IO_BUILTINS}

    for caller in sorted(reported):
        if caller not in bodies:
            continue                    # reported but not defined here: no edge to show
        if provably_dead(bodies, caller):
            continue                    # P2: nothing names it, so it cannot be a path
        if direct_io(caller):
            continue                    # a DIRECT effect proves nothing about propagation
        # NOTE ON THE OTHER END OF THE EDGE. Review asked for the same relation applied to
        # the callee. It is VACUOUS and was removed rather than kept as decoration: a
        # callee is discovered BY being named in the caller's body, so `provably_dead` —
        # "nothing in the unit names it" — is false for it by construction. The caller is
        # the only end where the relation can discriminate, and it is applied there.
        for callee in sorted(callees(bodies[caller], exclude=caller)):
            io = direct_io(callee)
            if io:
                return True, (f"`{caller}` performs no IO itself -> calls `{callee}` -> "
                              f"`{sorted(io)[0]}`; reported {reported[caller]}")
    named = ", ".join(sorted(reported))
    # "is unreachable" stood here and the code decided no such thing: `provably_dead` is
    # "no other body in the unit mentions this name". A verdict line is a claim like any
    # other, and this one was making the gate's own weakest inference sound like an
    # analysis.
    return False, (f"no caller with a mentioned name exhibits the edge caller -> "
                   f"callee -> "
                   f"IO builtin; every function reported with an IO effect ({named}) "
                   f"either performs IO directly, is named by nothing else in the unit, "
                   f"is not defined here, or calls nothing that does")


# ---------------------------------------------------------------------------
# Context — every input the gate reads, so the self-test can drive main().
# ---------------------------------------------------------------------------
@dataclass
class Context:
    root: Path = ROOT
    requirements: Path = field(
        default_factory=lambda: ROOT / "docs/contributing/1.0-requirements.tsv")
    conformance_manifest: Path = field(
        default_factory=lambda: ROOT / "tests/conformance-manifest.txt")
    witnesses: tuple[str, ...] = WITNESSES
    verdicts_text: str | None = None               # injected conformance output
    make_results: dict[str, int] | None = None     # injected `make <target>` exit codes
    effect_reports: dict[str, str] | None = None   # injected `pdc compile` output
    observable_results: dict[str, int] | None = None   # injected `cargo test` exit codes
    # THE INPUT THIS CLASS DID NOT COVER WHILE CLAIMING TO COVER EVERY INPUT. `main()` read
    # the gate's own source straight off ROOT for the wiring check, so no injected state
    # could reach it and the drift branch was undrivable — the one path that decides
    # whether the command may compute a verdict at all. None means "the real file", which
    # is what the release path must use.
    gate_source: str | None = None
    # SELF-TEST ONLY. Lets the scoring machinery be exercised as if GI-11 and GI-12 had
    # landed. A case asserts the REAL run never sets it, so this cannot become the fifth
    # existence check by another name.
    assume_definition_complete: bool = False


# Environment variables that change WHICH corpus a delegated run measures, or whether it
# measures at all. `conformance.sh` documents them at :88-94.
CORPUS_ENV_OVERRIDES = ("CONFORMANCE_MANIFEST", "CONFORMANCE_FORBID_OWNER",
                        "CONFORMANCE_BLESS")
# The value each takes when the caller does not choose one. `CONFORMANCE_MANIFEST` has no
# neutral value — a corpus must be named — so it is absent here and stated by the caller.
CORPUS_ENV_NEUTRAL = {"CONFORMANCE_BLESS": "0", "CONFORMANCE_FORBID_OWNER": ""}


def _probe(argv, cwd, reject_codes=(1,), env_overrides=None):
    """One process, one decision. -> Concluded (has `.text`) | Malfunction (does not).

    `reject_codes` IS PER CALL, because the same number means different things to
    different producers. It defaults to `(1,)` for a REJECT probe, where exit 1 is a
    legitimate verdict — the compiler refused a bad program. For `conformance.sh`, exit 1
    means THE CORPUS RUN FAILED, and inheriting the reject default made a failed run
    `Concluded`: its partial verdict lines were then parsed and scored. CONCLUDED IS NOT
    SUCCEEDED, and the boundary's own docstring said so while the caller ignored it.

    THE ENVIRONMENT IS NOT INHERITED BLIND. `CONFORMANCE_MANIFEST=… scripts/thesis-exit.sh`
    made the delegated run measure one corpus while `declared_fingerprint` read the
    canonical manifest — two corpora, one verdict — and `CONFORMANCE_BLESS=1` made the
    delegated run REWRITE the goldens instead of measuring them. The overriding variables
    are stripped, and a caller that needs one states it explicitly.
    """
    global GP
    if GP is None:
        GP = _load_gate_probe()
    # OVERRIDDEN, NOT UNSET, and the difference is a fact about the boundary rather than a
    # choice: `gate_probe.run` MERGES its `env` into `os.environ`, so nothing here can
    # remove a variable — it can only supply a value that wins. `conformance.sh` reads
    # `${CONFORMANCE_BLESS:-0}` and `${CONFORMANCE_FORBID_OWNER:-}`, so these two values
    # are the neutral ones; the manifest has no neutral value and is STATED by the caller
    # that cares, which is the same corpus `declared_fingerprint` reads.
    env = dict(CORPUS_ENV_NEUTRAL)
    env.update(env_overrides or {})
    return GP.classify(GP.run(argv, cwd=str(cwd), env=env), reject_codes=reject_codes)


def conformance_verdicts(ctx: Context) -> dict[str, str]:
    """Verdicts from the harness that actually RUNS things.

    scripts/conformance.sh compiles, links, runs, diffs stdout against a recorded
    transcript, checks the declared failure STAGE, matches the declared DIAGNOSTIC
    FINGERPRINT, reports REJECT_ACCEPTED when a negative test is accepted, and reports
    MISSING for a declared fixture not on disk. None of that is re-implemented here; its
    verdicts are read, and only after it concluded.
    """
    if ctx.verdicts_text is not None:
        text = ctx.verdicts_text
    else:
        # reject_codes=(): for THIS producer a non-zero exit is a failed measurement, not a
        # verdict. The manifest is stated rather than inherited, so the corpus the delegate
        # measures is the corpus `declared_fingerprint` reads.
        res = _probe(["bash", str(ctx.root / "scripts/conformance.sh"), "tests", "examples"],
                     ctx.root, reject_codes=(),
                     env_overrides={"CONFORMANCE_MANIFEST": str(ctx.conformance_manifest)})
        if not hasattr(res, "text"):
            raise HarnessError(f"scripts/conformance.sh did not conclude ({res.how}); "
                               "its output is not evidence")
        text = res.text
    verdicts = {}
    for line in text.splitlines():
        m = re.match(r"^(\S+\.pd)\s+([A-Z_]+)\s*$", line.strip())
        if m:
            verdicts[m.group(1)] = m.group(2)
    if not verdicts:
        raise HarnessError("scripts/conformance.sh produced no verdict lines")
    return verdicts


def declared_fingerprint(ctx: Context, path: str) -> str:
    try:
        text = ctx.conformance_manifest.read_text(encoding="utf-8")
    except OSError as exc:
        raise HarnessError(f"cannot read {ctx.conformance_manifest}: {exc}") from exc
    for line in text.splitlines():
        if line.startswith("#") or not line.strip():
            continue
        f = line.split("\t")
        if len(f) >= 4 and f[0] == path:
            return f[3]
    return ""


def p_verdict(ctx, verdicts, path, kind, want_fp) -> tuple[bool, str]:
    want = REQUIRED_VERDICT[kind]
    got = verdicts.get(path)
    if got is None:
        return False, f"DECLARED, ABSENT — no conformance row ran for {path} (want {want})"
    if got != want:
        return False, f"{path} is {got}, want {want}"
    if kind == "reject" and want_fp and want_fp != "-":
        # A rejection is evidence of the language property only if it is the INTENDED
        # rejection. "Refused because the prohibition is enforced" and "refused for
        # incidental unsupported syntax" are the same verdict at this layer, and a
        # sibling branch can turn a reject fixture green with no compiler change. So the
        # row names the fingerprint and the corpus must declare it; conformance.sh has
        # already matched that declaration against the actual diagnostic.
        # EQUALITY, not substring. Both sides of this comparison are ours — the row's
        # pin and the corpus's declaration — so there is no reason to be loose, and
        # `conformance.sh` is already substring-matching the declaration against the real
        # diagnostic (`grep -qF`, scripts/conformance.sh:204-211,870). Being loose here
        # too would compound two approximations into one unstated one.
        decl = declared_fingerprint(ctx, path)
        if want_fp.strip() != decl.strip():
            return False, (f"{path} is REJECTED, but for the wrong reason: the corpus "
                           f"declares '{decl or '(none)'}' and this row requires '{want_fp}'")
        return True, f"{path} REJECTED at '{decl}'"
    return True, f"{path} {got}"


def p_observable(ctx: Context, locator: str) -> tuple[bool, str]:
    """`path::test_name` — the test must exist, be un-ignored, AND PASS.

    It used to search for a matching function and stop. An EMPTY function with the right
    name satisfied it, so GI-11 could have gone green with no call graph, no contract
    assertion and nothing consumed — the promotion that was supposed to close MF5 would
    have changed scheduling and proved nothing. Presence is now a precondition for
    running it, not a substitute.
    """
    if "::" not in locator:
        raise HarnessError(f"observable locator {locator!r} is not `path::test_name`")
    rel, name = locator.split("::", 1)
    f = ctx.root / rel
    if not f.is_file():
        return False, f"DECLARED, ABSENT — {rel} does not exist"
    text = f.read_text(encoding="utf-8", errors="replace")
    if not re.search(rf"fn\s+{re.escape(name)}\s*\(", text):
        return False, f"DECLARED, ABSENT — {rel} has no `fn {name}`"
    if re.search(rf"#\[ignore[^\]]*\][^#]*fn\s+{re.escape(name)}\s*\(", text):
        return False, f"{rel}::{name} exists but is #[ignore]d"
    if ctx.observable_results is not None:            # self-test injection point
        rc = ctx.observable_results.get(locator)
        if rc is None:
            return False, f"DECLARED, ABSENT — no result for {locator}"
        return rc == 0, f"{locator} exit {rc}"
    target = Path(rel).stem
    res = _probe(["cargo", "test", "--release", "--test", target, "--",
                  "--exact", name, "--nocapture"], ctx.root)
    if not hasattr(res, "text"):
        raise HarnessError(f"`cargo test --test {target} -- --exact {name}` did not "
                           f"conclude ({res.how})")
    if not res.succeeded:
        return False, f"{locator} RAN AND FAILED (cargo exit {res.rc})"
    if "1 passed" not in res.text:
        return False, (f"{locator} — cargo exited 0 but did not report a passing test; "
                       f"a filter that matches nothing also exits 0")
    return True, f"{locator} ran and passed"


def p_make_target(ctx: Context, target: str) -> tuple[bool, str]:
    if ctx.make_results is not None:
        if target not in ctx.make_results:
            return False, f"DECLARED, ABSENT — no `{target}` target exists"
        rc = ctx.make_results[target]
        return rc == 0, f"make {target} exit {rc}"
    try:
        mk = (ctx.root / "Makefile").read_text(encoding="utf-8")
    except OSError as exc:
        raise HarnessError(f"cannot read {ctx.root / 'Makefile'}: {exc}") from exc
    if not re.search(rf"^{re.escape(target)}:", mk, re.M):
        return False, f"DECLARED, ABSENT — no `{target}` target exists"
    res = _probe(["make", "-s", target], ctx.root)
    if not hasattr(res, "text"):
        raise HarnessError(f"`make {target}` did not conclude ({res.how})")
    return res.succeeded, f"make {target} exit {res.rc}"


def effect_report(ctx: Context, witness: str) -> str:
    """Compile a witness and return its effect report — only if pdc CONCLUDED and
    SUCCEEDED. Effect lines scraped from a failed compilation would let partial or
    stale-format diagnostics satisfy the propagation condition."""
    if ctx.effect_reports is not None:
        if witness not in ctx.effect_reports:
            raise HarnessError(f"no injected effect report for {witness}")
        return ctx.effect_reports[witness]
    pdc = ctx.root / "target/release/pdc"
    if not pdc.is_file():
        raise HarnessError("target/release/pdc is not built")
    res = _probe([str(pdc), "compile", witness, "-o", os.devnull], ctx.root)
    if not hasattr(res, "text"):
        raise HarnessError(f"pdc did not conclude while compiling {witness} ({res.how})")
    if not res.succeeded:
        raise HarnessError(f"pdc rejected {witness} (exit {res.rc}); an effect report from "
                           "a failed compilation is not evidence")
    return res.text


# ---------------------------------------------------------------------------
# The manifest is the definition. This command executes it.
# ---------------------------------------------------------------------------
def thesis_rows(ctx: Context) -> list[dict]:
    """Closed, in the sense tests/conformance-manifest.txt is closed."""
    _validate_contract()
    rows, seen = [], set()
    try:
        lines = ctx.requirements.read_text(encoding="utf-8").splitlines()
    except OSError as exc:
        raise HarnessError(f"cannot read {ctx.requirements}: {exc}") from exc
    for n, line in enumerate(lines, 1):
        if not line or line.startswith("#"):
            continue
        f = line.split("\t")
        if len(f) != 9:
            raise HarnessError(f"{ctx.requirements.name}:{n}: {len(f)} columns, want 9")
        if f[0] in seen:
            raise HarnessError(f"{ctx.requirements.name}:{n}: duplicate id {f[0]}")
        seen.add(f[0])
        if f[7] != "thesis":
            continue
        if f[4] not in KINDS:
            raise HarnessError(f"{ctx.requirements.name}:{n}: unknown evidence kind {f[4]!r}")
        rows.append(dict(id=f[0], milestone=f[1], req=f[3], kind=f[4], ev=f[5], fp=f[8]))
    ids = {r["id"] for r in rows}
    if ids != EXPECTED_THESIS_IDS:
        added, gone = sorted(ids - EXPECTED_THESIS_IDS), sorted(EXPECTED_THESIS_IDS - ids)
        raise HarnessError(
            "the thesis row set changed, and that is a change to the DEFINITION OF 1.0. "
            f"added={added or 'none'} removed_or_retyped={gone or 'none'}. Update "
            "EXPECTED_THESIS_CONTRACT in this file in the same commit, deliberately.")
    _validate_pin_keys(set(PINNED_ACCEPTANCE_SHA))
    for r in rows:
        want = EXPECTED_THESIS_CONTRACT[r["id"]]
        got = (r["kind"], r["ev"], r["fp"])
        if got != want:
            raise HarnessError(
                f"{r['id']}: the thesis contract changed, which changes the DEFINITION OF "
                f"1.0. pinned {want}, manifest has {got}. Update EXPECTED_THESIS_CONTRACT "
                "in this file in the same commit, deliberately.")
        # DIRECT SUBSCRIPT, LIKE THE LINE ABOVE. `.get(...)` plus `if want_sha and …` meant
        # DELETING A KEY retired that row's pin in silence: measured, with `GI-11` removed
        # the identical weakening of its acceptance text raised nothing, while the pin's own
        # comment claimed "any edit at all fails". Key removal was the edit that did not.
        # One line up, `EXPECTED_THESIS_CONTRACT[r["id"]]` raises KeyError; one line was
        # loud and the next was silent.
        if r["id"] in PINNED_ACCEPTANCE_IDS and (
                acceptance_digest(r["req"]) != PINNED_ACCEPTANCE_SHA[r["id"]]):
            want_sha = PINNED_ACCEPTANCE_SHA[r["id"]]
            raise HarnessError(
                f"{r['id']}: its acceptance text changed (digest "
                f"{acceptance_digest(r['req'])}, pinned {want_sha}). That text IS the "
                "contract for this row — weakening it changes what 1.0 requires. Re-pin "
                "PINNED_ACCEPTANCE_SHA in the same commit, deliberately.")
        if r["kind"] == "reject" and (not r["fp"] or r["fp"] == "-"):
            raise HarnessError(
                f"{r['id']}: a thesis `reject` row with no required fingerprint. Any "
                "rejection would satisfy it — including one for incidental unsupported "
                "syntax — which is the hole the ninth column exists to close.")
    return rows


def evaluate(ctx: Context, rows: list[dict]) -> list[tuple[str, bool, str, str, str]]:
    """-> (id, ok, owner, detail, group). Exactly one result per non-aggregate row."""
    results = []
    verdicts = conformance_verdicts(ctx)          # HarnessError propagates: exit 2
    probes = {"TH-01": p_no_async_token, "TH-02": p_no_lifetime_param_list,
              "TH-03": p_has_ref_param, "TH-04": p_total_on_fn}
    seen_targets: dict[str, tuple[bool, str]] = {}

    def over_witnesses(fn):
        oks, details = [], []
        for w in ctx.witnesses:
            try:
                ok, d = fn(w)
            except Absent as e:
                ok, d = False, f"DECLARED, ABSENT — {e}"
            oks.append(ok)
            details.append(f"{w}: {d}")
        return all(oks), " · ".join(details)

    precondition_ids = {rid for rid, *_ in PRECONDITIONS}
    for r in rows:
        rid, kind = r["id"], r["kind"]
        if rid == AGGREGATE_ROW:
            continue                              # it IS the summary; see the docstring
        if rid in precondition_ids:
            continue                              # a PRECONDITION, not a scored row
        if rid in probes:
            ok, d = over_witnesses(lambda w, f=probes[rid]: f(read_source(ctx.root, w)))
            results.append((rid, ok, r["milestone"], d, "C1"))
        elif rid == "TH-05":
            def th05(w):
                # ORDER MATTERS. Reading the source first turns an ABSENT witness into a
                # finding; compiling first turned it into `pdc` exit 1 and therefore into
                # a malfunction, so "the witness has not been written yet" exited 2 and
                # reported nothing. The gate's own self-test did not catch this — running
                # it against the real repository did.
                src = read_source(ctx.root, w)
                return p_effect_is_transitive(effect_report(ctx, w), src)

            ok, d = over_witnesses(th05)
            results.append((rid, ok, r["milestone"], d, "C1"))
        elif rid == "TH-06":
            w2 = ctx.witnesses[1]
            sub = [("runs", *p_verdict(ctx, verdicts, w2, "fixture", "-"))]
            for name, fn in (("ref param", p_has_ref_param), ("#[total]", p_total_on_fn),
                             ("no async", p_no_async_token),
                             ("no 'a list", p_no_lifetime_param_list)):
                try:
                    ok, d = fn(read_source(ctx.root, w2))
                except Absent as e:
                    ok, d = False, f"DECLARED, ABSENT — {e}"
                sub.append((name, ok, d))
            bad = "; ".join(f"{n}: {d}" for n, ok, d in sub if not ok)
            results.append((rid, all(s[1] for s in sub), r["milestone"],
                            bad or "witness 2 runs and exercises all three differentiators",
                            "C4"))
        elif kind in REQUIRED_VERDICT:
            ok, d = p_verdict(ctx, verdicts, r["ev"], kind, r["fp"])
            results.append((rid, ok, r["milestone"], d, "C3" if kind == "reject" else "C2"))
        elif kind == "observable":
            # A named Rust test that must exist, not be #[ignore]d, and pass. Absent is a
            # FINDING (DECLARED, ABSENT), not a malfunction — same rule as a witness.
            ok, d = p_observable(ctx, r["ev"])
            results.append((rid, ok, r["milestone"], d, "C1"))
        elif kind == "gate":
            target = r["ev"].replace("make ", "").strip()
            if target not in seen_targets:
                seen_targets[target] = p_make_target(ctx, target)
            ok, d = seen_targets[target]
            results.append((rid, ok, r["milestone"], d, "C1"))
        else:
            # Never a silent skip: an undispatched kind means the manifest and this file
            # have diverged, which is a failure to measure and not a red row.
            raise HarnessError(f"{rid}: evidence kind {kind!r} reaches no dispatch")

    expected = {r["id"] for r in rows} - {AGGREGATE_ROW} - {p[0] for p in PRECONDITIONS}
    produced = [x[0] for x in results]
    if len(produced) != len(set(produced)) or set(produced) != expected:
        raise HarnessError(
            f"dispatch produced {len(produced)} result(s) for {len(expected)} row(s); "
            f"missing={sorted(expected - set(produced))} "
            f"extra={sorted(set(produced) - expected)}")
    return results


GROUPS = [
    ("C1", "Condition 1 — the fixed point, and every witness written in the dialect"),
    ("C2", "Condition 2 — one non-vacuous fixture per differentiator, RUN by scripts/conformance.sh"),
    ("C3", "Condition 3 — the reject twin per differentiator, at its DECLARED fingerprint. "
           "For an inference feature the rejection is the product"),
    ("C4", "Condition 4 — a second witness, so one program's shape is not the language"),
]


def main(ctx: Context | None = None) -> int:
    ctx = ctx or Context()
    gate_src = (ctx.gate_source if ctx.gate_source is not None
                else (ROOT / "scripts/thesis_exit.py").read_text())
    drift = wiring_matches_declaration(gate_src)
    if drift:
        raise HarnessError("the gate's declared models do not match its wiring: "
                           + "; ".join(drift))
    rows = thesis_rows(ctx)
    results = evaluate(ctx, rows)
    by_id = {r["id"]: r for r in rows}

    # ONE evaluation, reused. It ran twice — opening banner and closing report — so a
    # mutable workspace could produce two different answers in one run, and the corpora
    # were walked twice for nothing.
    blocked_early = ([] if ctx.assume_definition_complete
                     else incomplete_definition(gate_src))
    print("=" * 78)
    print("  make thesis-exit — the definition of Palladium 1.0")
    print(f"  {len(rows)} `thesis` rows from {ctx.requirements.name}; "
          f"{AGGREGATE_ROW} is the aggregate and is answered by the summary")
    if blocked_early:
        print("=" * 78)
        print("  NO VERDICT IS AVAILABLE. THE DEFINITION OF 1.0 IS INCOMPLETE.")
        print("  The rows below are STATE, not a score. Do not total them.")
    print("=" * 78)
    for key, title in GROUPS:
        group = sorted(r for r in results if r[4] == key)
        if not group:
            continue
        print(f"\n{title}")
        for rid, ok, owner, detail, _ in group:
            mark = f"{GREEN}ok  {OFF}" if ok else f"{RED}RED {OFF}"
            print(f"  {mark} {rid:<7} {by_id[rid]['req'][:52]:<54}"
                  f"{'' if ok else 'owed by ' + owner}")
            print(f"        {GREY}{detail}{OFF}")

    blocked = blocked_early
    red = [r for r in results if not r[1]]
    if blocked:
        print("\n" + "=" * 78)
        print("  NO VERDICT IS AVAILABLE. THE DEFINITION OF 1.0 IS INCOMPLETE.")
        print("=" * 78)
        for rid, why in blocked:
            print(f"  {RED}{rid}{OFF} outstanding — {why}")
        print()
        print("  These are not scored rows. They are preconditions on this command's")
        print("  ability to compute a verdict, and they are decided by introspecting this")
        print("  gate's own wiring — not by looking for an artifact. Four rounds running,")
        print("  a check on a not-yet-existing artifact degenerated to 'something by that")
        print("  name did not fail': an empty #[test] satisfied one, `@true` satisfied the")
        print("  next. There is nothing here to satisfy by naming it.")
        print()
        print("  WHAT NO CORPUS HERE ESTABLISHES, so it is not implied away:")
        for item in GI11_HUMAN_REVIEW_RESIDUE:
            print(f"    - {item}")
        print("    Discharged by HUMAN REVIEW at the point GI-11 lands. This is ONE")
        print("    boundary, not a list: provenance was on it and is mechanized now — one")
        print("    snapshot per unit, checked against the digest of the unit submitted, on")
        print(f"    the original and on the mutation of all {CALLGRAPH_ROWS} rows.")
        print()
        print("  A green run is not merely unreached — it is UNAVAILABLE, and will stay so")
        print("  until the two models above are replaced. `1.0 is not reached yet` is a")
        print("  measurement, and this command is not entitled to make one with tools it")
        print("  has itself disclosed as unsound.")
        print("=" * 78)
        return 2

    # DERIVED from the wired models, not a fixed paragraph. A fixed one would have gone
    # on saying "liveness is NOT asserted, that obligation is GI-11" after GI-11 landed,
    # misdescribing the very first verdict the gate was ever entitled to give.
    print("\n" + "-" * 78)
    print("  WHAT THIS GREEN MEANS")
    print("  Every differentiator's construct is present in both witnesses, each has a")
    print("  non-vacuous fixture and a reject twin, and the self-hosting compiler still")
    print("  reaches a byte-identical fixed point.")
    print(f"  Liveness: decided by the {LIVENESS_MODEL} model, which passes every case in")
    print("    tests/liveness-differential.tsv — answers fixed by review, not by the model.")
    print(f"  Rejection attribution: decided by {ATTRIBUTION_MODEL} matching.")
    print("=" * 78)
    print(f"  thesis: {len(results) - len(red)} green, {RED}{len(red)} RED{OFF}"
          f"   ({len(results)} evaluated rows + {AGGREGATE_ROW}, the aggregate)")
    if not red:
        print(f"  {AGGREGATE_ROW}: every row is green. Palladium 1.0: the thesis holds.")
        print("=" * 78)
        return 0
    print(f"  {AGGREGATE_ROW}: RED — 1.0 is not reached. Every line above names its owner.")
    print("  Committed red on purpose. Do not make it pass by weakening it: dropping the")
    print("  reject twins would let a no-op inferencer look finished.")
    print("=" * 78)
    return 1


# ---------------------------------------------------------------------------
# --self-test: drive main() with injected repository states.
# ---------------------------------------------------------------------------
HDR = "# id\tmilestone\tsource\trequirement\tkind\tevidence\tstatus\tdisposition\tfingerprint\n"

# Probe groups with no negative control. THIS IS AN EXPLICIT DISCLOSURE, NOT A DERIVED
# CHECK, and the difference matters: nothing here computes which probes lack a control, so
# this list cannot detect a probe that quietly loses one. What it does do is fail if the
# text changes, so the disclosure cannot be emptied, softened or reworded without the
# self-test going red and a human reading the diff. Deriving it for real would mean
# enumerating probes and their controls from the case table, which is worth doing the
# moment this list is longer than one entry.
# The self-test's own case inventory, pinned like everything else it polices. Set to "" to
# print the digest for a deliberate re-pin.
# Each metamorphic variant and the base it must be scored identically to. Pinned as a
# RELATION, because the digest detects an edit to a row but not that a variant has drifted
# into being a different program from its base.
VARIANT_OF_BASE = {
    "diverging-if": "mm-diverging-if-renamed",
    "while-true": "mm-while-true-reordered",
    "false-branch": "mm-false-branch-reordered",
    "direct": "mm-direct-spaced",
    "via-callee": "mm-via-callee-renamed",
    "inside-else": "mm-inside-else-renamed",
}

# RE-PINNED AGAIN on the review rework, for a reason worth naming: A CASE LABEL
# CARRIES A CITATION, so this digest is coupled to line numbers in grammar.ebnf.
# The label `\`fn q< 'a>\` SPACED goes RED — grammar.ebnf:151` became `:157` when
# N2-09's `char_escape` production was added four lines above `generic_params`.
# No control was added, removed or weakened; one label's citation moved with its
# target. Verified by diffing the CASE-NAME SET against the pre-rework tree: 293
# labels on both sides, exactly one differing, and it is that one.
#
# That coupling is a property of the pin, not a defect introduced here — but it
# means any edit to grammar.ebnf re-pins this digest, and a reviewer should
# always ask WHICH label moved rather than accepting the new value.
#
# RE-PINNED TWICE BEFORE THAT, and that value was neither side's — recomputed on the merge.
#
# `fix/m2-builtins-exit` re-pinned it because one case text changed ("the two gate
# counts" -> "the four gate counts") when PINNED_PROSE_FIGURES took the two figures the
# M2 exit criterion introduced; no case was added or removed.
#
# `fix/m2-lexical` re-pinned it because the three `strip_literals` block-comment cases
# were rewritten to assert NESTING, which is what the old ones existed to force: two
# case LABELS changed and one case was ADDED -- `...and a real `async` AFTER the outer
# close is still FOUND` -- because nesting introduces a way for the probe to become one
# that can never fire, which is F12's defect in the other direction and needs its own
# control.
#
# Both are true of the merged tree and neither branch's digest is, which is the whole
# reason this pin exists. Recomputed here via `--print-case-digest`.
# Superseded: 1dd2b683... (base), 2bc2aabd... (lexical), bc01d66e... (builtins-exit).
EXPECTED_CASE_SHA = "60351814c29ebdb9a29d2ba34b3f00f6edc7cef61535adde4115f84a7c0f4897"

EXPECTED_UNCOVERED = frozenset({
    "the real `make` subprocess: a control would need a deliberately broken build. Its "
    "target-absent and nonzero-exit paths ARE covered, by injection.",
})

# The disclosure exactly as reviewed. Compared verbatim, so emptying OR rewording the set
# above fails the self-test instead of quietly printing a different promise.
_UNCOVERED_AS_REVIEWED = (
    "the real `make` subprocess: a control would need a deliberately broken build. Its "
    "target-absent and nonzero-exit paths ARE covered, by injection.",
)

# BANNED-LIST-BEGIN (excluded from the check, or it would flag its own examples)
# --- RETRACTED CLAIMS, BANNED BY NAME ------------------------------------------------
#
# Deleting code is not deleting claims. Round 7 removed `reachable_from_main`,
# `certainly_reached_from_main`, `top_level_calls` and `strip_false_blocks`, and three
# sentences asserting what they had computed survived the deletion — a docstring saying
# "REACHABLE FROM `main`", a verdict line saying "CERTAINLY REACHED FROM MAIN", and a
# self-test heading claiming an edge relation applied to BOTH ends after the callee half
# had been deleted for being vacuous.
#
# Each phrase below was retracted by a specific round. The check is whole-file and by
# name, so re-asserting one fails as itself rather than waiting for a reviewer. Adding a
# phrase here is how a retraction is made durable; removing one is a claim that the thing
# is true again, and it is a diff a reviewer can see.
RETRACTED_CLAIMS = (
    ("REACHABLE FROM", "round 7 — reachability is not computed; P1 is existence"),
    ("CERTAINLY REACHED", "round 7 — certain-reachability had three fail-open paths"),
    ("certainly reached from main", "round 7 — same"),
    ("applies to BOTH ends", "round 7 — the callee half is vacuous and was deleted"),
    ("the safe direction for a gate", "round 6 — over-approximation fails OPEN here"),
    ("no longer expressible", "round 4 — Python has no access control"),
    ("not reachable, rather than merely discouraged", "round 4 — same"),
    ("cannot grep what was never printed", "round 4 — same"),
    # Round 11: a FIXED paragraph about what a future verdict would mean. Repaired in code
    # in round 10 and left in prose, which is why this entry names the wording rather than
    # the file — MILESTONES.md is in CLAIM_SCANNED, so the lint now covers both.
    ("liveness is\nnot asserted, after three lexical", "round 11 — derive it, do not fix it"),
    ("Would NOT mean: that any differentiator is used", "round 11 — same"),
    # Round 12: the corpus was made the WHOLE precondition in round 10 and restored to one
    # of two halves in round 11; this wording survived the restoration in three places.
    ("ENTIRE liveness precondition", "round 12 — the corpus is the VERDICT half only"),
    ("whole liveness precondition", "round 12 — same"),
    # Round 18: RETRACTED PROPERTIES, not retracted mechanisms. Reviewers asked whether
    # DEAD_MECHANISMS should carry these; it should not, and the distinction is the whole
    # reason there are two lists. A dead MECHANISM is a thing that no longer exists — the
    # token names it and the token is gone. A retracted PROPERTY is a thing the code never
    # computed: `provably_dead` decides "no other body mentions this identifier", and the
    # roadmap described that as reachability from `main` and as "actually called" for six
    # rounds. Nothing was deleted, so no mechanism name could catch it; the wording is what
    # was wrong, which is what RETRACTED_CLAIMS is for. They go here.
    ("reachability from `main`", "round 18 — `provably_dead` decides mentions, not reach"),
    ("that is actually called", "round 18 — a mention is not a call; same defect"),
    # Round 19: the same property, in the probes' OWN prose and in a verdict line, spelled
    # differently enough to walk past the two entries above. A retracted property has as
    # many spellings as English has, which is the standing weakness of a phrase list and
    # the reason the entry says what the code decides instead of only what it does not.
    ("PARAMETER on a function reachable from", "round 19 — it decides `mentioned`, not `reachable`"),
    ("is unreachable, is not defined here", "round 19 — the verdict line claimed reachability"),
    # Round 22: the corpus header retracted "a lookup table fails" BY MEASUREMENT in this
    # same branch, and the docstring went on asserting it. The phrase is banned so the
    # unqualified form cannot come back — and the general problem is named rather than
    # implied solved: a banned list is lexical, this retraction was spelled differently,
    # and nothing here detects two files asserting opposite things about one mechanism.
    ("so a lookup table fails", "round 22 — refuted by the corpus header's own measurement"),
)

# Mechanisms that NO LONGER EXIST, as EXACT TOKENS, matched CASE-INSENSITIVELY.
#
# WHAT THIS IS NOT, because the previous comment here claimed it: it does NOT "name the
# MECHANISM, so any sentence that still points at it fails regardless of how it is
# phrased". It is a token search. A paraphrase passes, and the round that retracts
# something still has to add its words here. Two narrowings this round, both measured:
#
#  * CASE-INSENSITIVE. The tokens were compared case-sensitively, so `N10_CALLGRAPH.RS`
#    walked through a check whose stated purpose is to stop the sentence coming back.
#  * `p_observable` IS GONE FROM THIS LIST, and that is not a claim that it is alive
#    again as a mechanism — it is that the token never named a dead one. `p_observable`
#    is a LIVE function in this file, the dispatcher for `observable` rows, so once this
#    file is scanned (it was skipped, which is where these sentences most often live) the
#    token can only produce a false positive on its own definition. The fact it was
#    guarding — that GI-11 is adjudicated by a named test — is pinned where it is
#    decidable instead: EXPECTED_THESIS_CONTRACT["GI-11"] is ("gate", "make thesis-exit",
#    "-"), compared against the manifest on every run.
DEAD_MECHANISMS = (
    ("acceptance observable", "GI-11 stopped being an `observable` in round 12"),
    ("n10_callgraph.rs", "the named test is no longer any row's evidence"),
)


# BANNED-LIST-END


def stale_claims(text: str) -> list[str]:
    """Retracted wording still present. Whole-file, by name.

    WHAT THIS DOES NOT CATCH, stated where the mechanism is described because a check
    whose limits are undocumented is the thing this file keeps being rebuilt over:
    a PARAPHRASE ("the function main can reach it"), a string assembled at runtime, and
    wording in a file not passed to it. It catches the exact retracted phrases, which is
    what stopped three of them surviving a deletion, and nothing more.

    AND THE STANDING QUESTION — is there a shape that keeps the entries current, or is
    this intrinsically a per-round human act? IT IS A PER-ROUND HUMAN ACT, and the record
    says so: the entries have now failed to catch a retraction TWICE, in rounds 11 and 12,
    both times because they named the phrases of an earlier round while the claim came
    back in new words. Deriving them would require knowing which sentences a future
    retraction will invalidate, which is the retraction itself. So the honest statement is
    that this lint prevents the exact wording from returning silently, and that noticing a
    PARAPHRASE is a reviewer's job — which is why the round that retracts something must
    add its wording here in the same commit, and why the success message says "no exact
    banned phrase" rather than "no retracted claim".
    """
    return [f"{phrase!r} ({why})" for phrase, why in RETRACTED_CLAIMS if phrase in text]


# Files the release path scans. The TSV is included because a retracted claim can as
# easily live in a requirement's text as in a docstring.
CLAIM_SCANNED = (
    "scripts/thesis_exit.py",
    "scripts/thesis-exit.sh",
    "docs/contributing/MILESTONES.md",
    "docs/contributing/1.0-requirements.tsv",
    # THE NORMATIVE ANNEX, added after it carried a false universally-quantified absence
    # ("No fixture uses this class: reject=0") for a second time. This file's own F7 says
    # the annex is the authority a release plan reads, so a stale number in it is release
    # governance — and it was the one authority not under any mechanism here.
    "docs/specification/language-spec.md",
)

# Conformance totals appear in prose in several files and rot when the corpus grows: 53 -> 70
# happened during an integration and left two files disagreeing. The counts are MEASURED from
# the manifest, so a prose figure that no longer matches is a finding rather than a surprise.
CONFORMANCE_MANIFEST_PATH = ROOT / "tests/conformance-manifest.txt"


def conformance_corpus_size() -> int:
    """Rows in the manifest — the denominator every `over N fixtures` sentence quotes."""
    if not CONFORMANCE_MANIFEST_PATH.is_file():
        raise HarnessError(f"{CONFORMANCE_MANIFEST_PATH} is missing; the corpus size that "
                           "every prose figure quotes cannot be measured")
    return len([l for l in CONFORMANCE_MANIFEST_PATH.read_text().splitlines()
                if l.strip() and not l.startswith("#")])


def corpus_figures_in(rel: str, text: str, want: int) -> list[str]:
    """`over N fixtures` in one text where N is not `want`. PURE, so it can be PLANTED.

    Its sibling `scan_claims(rel, text)` has this shape for exactly this reason: a checker
    that can only be run against the real tree has no negative control, and gutting it to
    `return []` left the self-test green with the digest unchanged.
    """
    out = []
    for m in re.finditer(r'(.?)over (\d+)\s*\n?fixtures', text):
        # A QUOTED figure is a RECORD OF WHAT SOMETHING SAID, not a live claim. F7 exists
        # to quote stale numbers accurately, so a checker that could not tell a quotation
        # from an assertion would make honest history impossible to write.
        if m.group(1) == '"':
            continue
        if int(m.group(2)) != want:
            out.append(f"{rel}: `over {m.group(2)} fixtures`, measured {want}")
    return out


def stale_corpus_figures() -> list[str]:
    """Every scanned file's live `over N fixtures`, against the MEASURED corpus size."""
    want = conformance_corpus_size()
    out = []
    for rel in CLAIM_SCANNED:
        f = ROOT / rel
        if f.is_file():
            out += corpus_figures_in(rel, f.read_text(), want)
    return out


def dead_mechanism_hits(text: str) -> list[str]:
    """Exact tokens naming a mechanism that no longer exists, matched case-insensitively.

    Narrow by construction, and the narrowness is the point of the name: it finds the
    TOKEN. It cannot tell a claim from a mention, and a paraphrase escapes it — which is
    why the tokens live next to the retracted phrases and are a per-round human act.
    """
    low = text.lower()
    return [f"names the dead mechanism {mech!r} ({why})"
            for mech, why in DEAD_MECHANISMS if mech.lower() in low]


def scan_claims(rel: str, text: str) -> list[str]:
    """Every claim check for ONE file's text. Pure, so the self-test can plant text.

    scripts/thesis_exit.py IS SCANNED FOR DEAD MECHANISMS TOO. It was exempted, and it is
    the file that most often carries these sentences — the wording that survived rounds 11
    and 12 was in this docstring, not in a document. Only the banned-list block itself is
    excluded, because it is where the phrases are deliberately written down.
    """
    b, e = "# BANNED-LIST-" + "BEGIN", "# BANNED-LIST-" + "END"
    if rel == "scripts/thesis_exit.py" and b in text and e in text:
        text = text.split(b)[0] + text.split(e)[1]
    # No sentinels means no exclusion, which fails CLOSED: losing them makes the check
    # flag the banned list itself rather than quietly scanning nothing.
    return [f"{rel}: {hit}" for hit in stale_claims(text) + dead_mechanism_hits(text)]


def check_retracted_claims() -> int:
    """`make check-retracted-claims`. On the release path, not only under --self-test."""
    bad = []
    for rel in CLAIM_SCANNED:
        bad += scan_claims(rel, (ROOT / rel).read_text(encoding="utf-8", errors="replace"))
    bad += stale_corpus_figures()
    if bad:
        print(f"{RED}retracted claims are back{OFF}:")
        for b in bad:
            print(f"  {b}")
        print("Each was retracted by the round named. Re-asserting one is a claim that it "
              "is true again; make that argument in the commit, or remove the wording.")
        return 1
    print(f"{GREEN}ok{OFF} no EXACT BANNED PHRASE and no DEAD-MECHANISM TOKEN in "
          f"{len(CLAIM_SCANNED)} file(s), this script INCLUDED except for the banned-list "
          f"block itself; {len(RETRACTED_CLAIMS)} phrases (case-sensitive) and "
          f"{len(DEAD_MECHANISMS)} tokens (case-insensitive) checked. This does not certify "
          f"the absence of a retracted CLAIM: a paraphrase, a runtime-assembled string, or "
          f"a file not in CLAIM_SCANNED all pass.")
    return 0


# The real acceptance text for the digest-pinned rows, read from the manifest, so the
# synthetic corpus agrees with the pin instead of duplicating it.
_REAL_ACCEPTANCE_CACHE: dict[str, str] | None = None


def real_acceptance() -> dict[str, str]:
    """The pinned rows' acceptance text, read from the manifest — LAZILY.

    THIS RAN AT IMPORT TIME AND THAT DEFEATED THE WHOLE THREE-STATE CONTRACT. Module-level
    file I/O happens BEFORE `_entry()` exists to catch anything, so a missing manifest
    raised `FileNotFoundError` out of the import and Python exited 1 — and in this gate's
    contract exit 1 means THE THESIS DOES NOT HOLD. Measured: hide
    docs/contributing/1.0-requirements.tsv and the command reported Palladium 1.0
    DISPROVEN, with no `THESIS_RESULT` line at all.

    A failure to measure is never a verdict. That is the sentence this file is built on,
    and import order was quietly exempt from it. Deferred here so the read happens inside
    the boundary, where an absent manifest is exit 2 like every other unreadable input.
    """
    global _REAL_ACCEPTANCE_CACHE
    if _REAL_ACCEPTANCE_CACHE is None:
        rows = (ROOT / "docs/contributing/1.0-requirements.tsv").read_text().split("\n")
        _REAL_ACCEPTANCE_CACHE = {
            r.split("\t")[0]: r.split("\t")[3] for r in rows
            if len(r.split("\t")) == 9 and not r.startswith("#")
            and r.split("\t")[0] in PINNED_ACCEPTANCE_SHA}
    return _REAL_ACCEPTANCE_CACHE


def module_level_file_reads(source: str) -> list[str]:
    """Top-level statements SPELLED as a file read. A FAST PRE-CHECK, NOT THE CONTROL.

    THIS DOCSTRING USED TO CALL ITSELF "THE DURABLE HALF" AND THAT WAS THE SAME DEFECT ONE
    LEVEL UP: it names spellings, not the fact. Measured escapes, all of which leave it
    returning `[]` while the requirement is violated —

        _EAGER = real_acceptance()        a module-level call to a reading HELPER
        def f(x=Path("p").read_text()):   a default-argument value
        @decorator(Path("p").read_text()) a decorator expression
        class C: data = open("p").read() a class-body statement

    — plus `subprocess`, `os.listdir`, `Path.iterdir` and a bound-method read. The
    requirement is "no external file is read before `_entry()` can classify the failure",
    and no list of attribute names decides that.

    What decides it is running the command with the file gone and reading the exit code,
    which `case("a MISSING manifest is exit 2 …")` does in a subprocess. Note the existing
    `unreadable_requirements=True` control cannot see this by construction: it drives
    `_entry()`, and the defect is upstream of `_entry()`.
    """
    import ast as _ast
    bad = []
    for node in _ast.parse(source).body:
        if isinstance(node, (_ast.FunctionDef, _ast.AsyncFunctionDef, _ast.ClassDef)):
            continue
        for sub in _ast.walk(node):
            if (isinstance(sub, _ast.Call) and isinstance(sub.func, _ast.Attribute)
                    and sub.func.attr in ("read_text", "read_bytes", "open")):
                bad.append(f"line {node.lineno}: {sub.func.attr}() at module level")
            elif (isinstance(sub, _ast.Call) and isinstance(sub.func, _ast.Name)
                  and sub.func.id == "open"):
                bad.append(f"line {node.lineno}: open() at module level")
    return bad

BASE_ROWS = [
    ("D1-01", "M9", "gate", "make thesis-exit", "-"),
    ("N7-01", "M5", "reject", "tests/reject/async_fn.pd", "there is no `async` keyword"),
    ("N7-02", "M5", "reject", "tests/10_async_await.pd", "there is no await operator"),
    ("N7-04", "M5", "fixture", "tests/09_effects_propagate.pd", "-"),
    ("N7-08", "M5", "reject", "tests/reject/pure_function_calls_io.pd", "declared pure"),
    ("N8-01", "M6", "fixture", "tests/13_total_attribute.pd", "-"),
    ("N8-06", "M6", "fixture", "tests/13_structural_recursion.pd", "-"),
    ("N8-08", "M6", "reject", "tests/reject/total_unproven.pd", "cannot prove termination"),
    ("N9-01", "M7", "fixture", "tests/05_ref_shared.pd", "-"),
    ("N9-03", "M7", "fixture", "tests/05_ref_named_region.pd", "-"),
    ("N9-04", "M7", "reject", "tests/reject/lifetime_param_list.pd", "lifetime parameter list"),
    ("N9-06", "M7", "reject", "tests/reject/ambiguous_region.pd", "ambiguous region"),
    ("SH-01", "-", "gate", "make selfhost", "-"),
    ("SH-02", "M9", "gate", "make selfhost-corpus", "-"),
    ("SH-03", "M9", "gate", "make selfhost-corpus", "-"),
    ("SH-04", "M9", "gate", "make selfhost-corpus", "-"),
    ("SH-05", "M9", "gate", "make selfhost-determinism", "-"),
    ("TH-01", "M9", "gate", "make thesis-exit", "-"),
    ("TH-02", "M9", "gate", "make thesis-exit", "-"),
    ("TH-03", "M9", "gate", "make thesis-exit", "-"),
    ("TH-04", "M9", "gate", "make thesis-exit", "-"),
    ("TH-05", "M9", "gate", "make thesis-exit", "-"),
    ("TH-06", "M9", "gate", "make thesis-exit", "-"),
    ("WT-02", "M9", "fixture", "tests/witness/json_parser.pd", "-"),
    ("GI-11", "M3-start", "gate", "make thesis-exit", "-"),
    ("GI-12", "M2", "gate", "make check-diagnostic-codes", "-"),
]


def _rows(drop=None, retype=None, repoint=None, blank_fp=None, extra=""):
    out = [HDR]
    for rid, ms, kind, ev, fp in BASE_ROWS:
        if rid == drop:
            continue
        req = real_acceptance().get(rid, f"req {rid}")
        if retype and rid == retype[0]:
            kind, ev = retype[1], retype[2]
        if repoint and rid == repoint[0]:
            ev = repoint[1]
        if rid == blank_fp:
            fp = "-"
        out.append(f"{rid}\t{ms}\tsrc\t{req}\t{kind}\t{ev}\towed\tthesis\t{fp}\n")
    return "".join(out) + extra


GOOD_WITNESS = """
fn emit(mut c: C, s: String) { file_write(c.out, s); }
fn header(mut c: C) { emit(c, "x"); }
#[total]
fn depth(n: i64) -> i64 { return n; }
fn drive(x: ref String, mut c: C) -> i64 { header(c); return depth(1); }
fn main() { drive(s, c); }
"""
GOOD_REPORT = "Function 'emit' has effects: [Io]\nFunction 'header' has effects: [Io]\n"
# The synthetic corpus uses the REAL evidence locators, because the contract is pinned:
# a synthetic state that disagreed with it would be rejected before any case ran.
_C = EXPECTED_THESIS_CONTRACT
ALL_VERDICTS = "\n".join(
    f"{ev} {'REJECTED' if kind == 'reject' else 'PASS_VERIFIED'}"
    for kind, ev, _fp in _C.values() if kind in ("reject", "fixture"))
WITNESS2 = _C["WT-02"][1]
GOOD_MAKE = {"selfhost": 0, "selfhost-corpus": 0, "selfhost-determinism": 0,
             "thesis-exit": 0, "check-diagnostic-codes": 0}
GOOD_OBSERVABLE: dict[str, int] = {}   # no thesis row is an `observable` now


def mutate(text: str, old: str, new: str, expect: int = 1) -> str:
    """Replace `old` with `new`, asserting it matched EXACTLY `expect` times.

    THE MECHANICAL ANSWER to a shape this gate has now hit four times: a control whose
    `str.replace` silently becomes a no-op, so the case passes by asserting that an
    unmutated state is unchanged. `_verdict()` closed it for verdict lines; every other
    synthetic edit — witness sources, fingerprint maps — went through raw `str.replace`
    and had the same hole. Now nothing does.
    """
    if old == new:
        # An identity "mutation" is a control that tests nothing, exactly like one that
        # matches nothing. Same defect, and it would read as deliberate in a diff.
        raise HarnessError(f"self-test: mutation {old!r} replaces text with itself")
    n = text.count(old)
    if n != expect:
        raise HarnessError(
            f"self-test: mutation {old!r} matched {n} time(s), expected {expect}. "
            "A control that mutates nothing tests nothing.")
    out = text.replace(old, new)
    if out == text and old != new:
        raise HarnessError(f"self-test: mutation {old!r} changed nothing")
    return out


def mutate_fp(row_id: str, wrong: str) -> dict:
    """GOOD_FP with ONE row's declaration replaced, asserting the old value was there.

    The fingerprint control used to build its dict by overriding an entry directly, with
    no assertion that the key existed or that the value changed — the same
    mutates-nothing hole `mutate()` closes for text, in the one place that was not text.
    Sixth sighting of the class; this is the typed mutator that makes the claim true.
    """
    kind, ev, fp = _C[row_id]
    if kind != "reject" or fp == "-":
        raise HarnessError(f"self-test: {row_id} is not a reject row with a fingerprint")
    # GOOD_FP is derived from _C today, so this cannot currently fire; it is a guard for a
    # future where the two stop being derived from one source. Stated rather than counted
    # as coverage — an unreachable check is not a control.
    if GOOD_FP.get(ev) != fp:
        raise HarnessError(f"self-test: {ev} does not currently declare {fp!r}")
    if wrong == fp:
        raise HarnessError("self-test: the wrong fingerprint equals the right one")
    return {**GOOD_FP, ev: wrong}


def _verdict(row_id: str, verdict: str) -> str:
    """ALL_VERDICTS with one row's verdict replaced — and an ASSERTION that it changed.

    Four controls were disarmed the moment the synthetic fixture paths were derived from
    the pinned contract: they mutated `ALL_VERDICTS` with a hardcoded path, the
    `str.replace` became a no-op, nothing turned red, and the cases passed by asserting
    that an unmutated all-green state is... green. A control that silently stops
    controlling is the defect this whole gate exists to catch, so the mutation is now
    the thing that fails loudly.
    """
    kind, ev, _fp = _C[row_id]
    was = "REJECTED" if kind == "reject" else "PASS_VERIFIED"
    old, new = f"{ev} {was}", f"{ev} {verdict}".strip()
    # Through mutate(), so the "one helper owns every synthetic edit" claim is true and
    # the count is EXACTLY one rather than at-least-one.
    return mutate(ALL_VERDICTS, old, new)
# Every pinned reject fingerprint, so the all-green state really is all-green. One entry
# used to cover one row; the other five rows had `-` and skipped the comparison, which is
# the hole MF2 closed.
# The corpus must declare EXACTLY the pinned fingerprint — that is the contract the gate
# enforces, and conformance.sh then greps that literal against the real diagnostic.
GOOD_FP = {ev: fp for kind, ev, fp in _C.values() if kind == "reject" and fp != "-"}


def _drive(*, rows=None, witness_b=GOOD_WITNESS, verdicts=ALL_VERDICTS, make=None,
           report=GOOD_REPORT, report_b=None, fingerprints=None, drop_witness_b=False,
           omit_report_b=False, real_conformance=None, real_pdc=None,
           unreadable_requirements=False, unreadable_makefile=False,
           real_make=False, drop_observable=False, observables=None,
           definition_incomplete=False, gate_source=None) -> int:
    """Run the WHOLE gate against an injected repository state."""
    with tempfile.TemporaryDirectory() as d:
        tmp = Path(d)
        (tmp / "a.pd").write_text(GOOD_WITNESS)
        # GI-11 is no longer an `observable`, so the synthetic repo has no test file to
        # create. Kept as a no-op knob rather than removed, so the `drop_observable` cases
        # fail loudly if someone reintroduces an observable precondition.
        assert "::" not in _C["GI-11"][1], ("GI-11 is an observable again — see rounds 12 "
                                            "and 13")
        w2 = tmp / WITNESS2                      # the contract's own path for witness 2
        w2.parent.mkdir(parents=True, exist_ok=True)
        if not drop_witness_b:
            w2.write_text(witness_b)
        req = tmp / "req.tsv"
        req.write_text(rows if rows is not None else _rows())
        cm = tmp / "cm.txt"
        cm.write_text("".join(f"{p}\treject\tcompile\t{fp}\t-\t-\n"
                              for p, fp in (fingerprints or GOOD_FP).items()))
        # omit_report_b models "witness 2 has no measurable report". Paired with
        # drop_witness_b it pins the ORDER inside TH-05: read_source must run first, so an
        # ABSENT witness is a finding. If the order regresses, effect_report is reached,
        # finds no injected report, raises HarnessError, and this case sees exit 2.
        reports = None if report is None else {"a.pd": report}
        if not omit_report_b and reports is not None:
            reports[WITNESS2] = report if report_b is None else report_b
        # real_conformance / real_pdc drop the injection and force the REAL subprocess
        # boundary, which is the only way to exercise gate_probe's status handling: with
        # verdicts_text or effect_reports set, no process is ever launched.
        if real_conformance is not None:
            (tmp / "scripts").mkdir(exist_ok=True)
            sh = tmp / "scripts/conformance.sh"
            sh.write_text(real_conformance)
            sh.chmod(0o755)
            verdicts = None
        if real_pdc is not None:
            (tmp / "target/release").mkdir(parents=True, exist_ok=True)
            ex = tmp / "target/release/pdc"
            ex.write_text(real_pdc)
            ex.chmod(0o755)
            reports = None
        ctx = Context(root=tmp, requirements=req, conformance_manifest=cm,
                      witnesses=("a.pd", WITNESS2), verdicts_text=verdicts,
                      make_results=None if real_make else (GOOD_MAKE if make is None else make),
                      effect_reports=reports,
                      observable_results=(None if drop_observable
                                          else (observables or GOOD_OBSERVABLE)),
                      assume_definition_complete=not definition_incomplete,
                      gate_source=gate_source)
        if real_make:
            (tmp / "Makefile").write_text(
                "".join(f"{tgt}:\n\t@true\n" for tgt in GOOD_MAKE))
            make = None                        # force the real `make` subprocess
        if unreadable_requirements:
            req.unlink()
            req.mkdir()                       # a directory where a file is required
        if unreadable_makefile:
            ctx = Context(root=tmp, requirements=req, conformance_manifest=cm,
                          witnesses=("a.pd", WITNESS2), verdicts_text=verdicts,
                          make_results=None,   # force the real Makefile read
                          effect_reports=reports,
                      observable_results=(None if drop_observable
                                          else (observables or GOOD_OBSERVABLE)),
                      assume_definition_complete=not definition_incomplete,
                      gate_source=gate_source)
        buf = io.StringIO()
        # THE REASON A HARNESS ERROR CARRIES IS ON stderr, and only stdout was captured —
        # so a case asserting WHY the gate refused could not see the why at all, and the
        # only assertion available was the exit code. Both streams are captured now, which
        # is what makes `_because(...)` possible.
        try:
            with redirect_stdout(buf), redirect_stderr(buf):
                rc = main(ctx)
        except HarnessError as exc:
            buf.write(f"\nharness error: {exc}\n")
            rc = 2
        _drive.last_output = buf.getvalue()
        _drive.calls += 1
        return rc


_drive.last_output = ""
_drive.calls = 0


def self_test() -> int:
    global GP
    if GP is None:
        GP = _load_gate_probe()
    fails = cases = driven = 0

    seen_names: set[str] = set()

    def case(name, got, want, drives_main=True):
        """Record one case. DUPLICATE LABELS ARE A HARNESS ERROR.

        The summary claimed "124 unique" while `case()` only incremented a counter — an
        asserted property, not a checked one, in the run whose whole job is to check
        properties. Ten duplicates had just been removed by hand; nothing stopped the
        eleventh. Now a repeated label stops the run.
        """
        nonlocal fails, cases, driven
        # DECLARED vs MEASURED. `drives_main` was a flag the caller set, and the summary
        # reported the sum of those flags as if it were an observation — "59 drive main()
        # end to end" was a claim nothing checked. Measured when that was asked: one case
        # was mis-declared. `_drive` counts its own invocations, so the flag is now
        # compared against what happened.
        drove = _drive.calls > case.mark
        case.mark = _drive.calls
        if drove != drives_main:
            raise HarnessError(
                f"self-test: case {name!r} declares drives_main={drives_main} but "
                f"main() was {'' if drove else 'NOT '}driven. The split in the summary is "
                "an observation, not a label; fix the declaration or the case.")
        if name in seen_names:
            raise HarnessError(
                f"self-test: duplicate case label {name!r}. A count that grows by copying "
                "measures nothing; give the new case its own name or delete the copy.")
        seen_names.add(name)
        cases += 1
        driven += 1 if drives_main else 0
        if got == want:
            print(f"  {GREEN}ok  {OFF} {name}")
        else:
            print(f"  {RED}FAIL{OFF} {name} (got {got!r}, want {want!r})")
            fails += 1

    def _why(code):
        """WHICH failure, not that one: `<code> RED=<ids>` / `BLOCKED=<ids>` / `HARNESS=<reason>`.

        `_drive(...) == 1` says "some row went red" and `== 2` says "some harness error
        happened" — and EVERY red row, or every harness error, satisfies it. That is the
        shape that let one upstream wall discharge three declared debts on a sibling
        branch, and it was the assertion of 44 cases here. This reads the run's own output
        and names the failure, so a case that goes red for a new reason stops passing.
        """
        out = re.sub(r"\x1b\[[0-9;]*m", "", _drive.last_output)
        # The synthetic repository lives in a fresh mkdtemp, so its path is different on
        # every run: a signature carrying one would pin the temp directory, not the reason.
        out = re.sub(r"(/[^\s'\"]+)+", lambda m: ("<tmp>" if "/var/" in m.group(0)
                                                   or "/tmp/" in m.group(0)
                                                   else m.group(0)), out)
        m = re.search(r"harness error: (.+)", out)
        if m:
            return f"{code} HARNESS={m.group(1).strip()[:44]}"
        blocked = sorted(set(re.findall(r"^\s*([A-Z]+-\d+) outstanding", out, re.M)))
        if blocked:
            return f"{code} BLOCKED={','.join(blocked)}"
        reds = sorted(set(re.findall(r"^\s*RED\s+([A-Z][A-Z0-9]*-\d+)", out, re.M)))
        return f"{code} RED={','.join(reds)}"

    def _because(code, needle):
        """(exit code, does the output NAME the reason?).

        A DECLARED FAILURE MUST NOT BE SATISFIED BY ANY FAILURE. `_drive(...) == 2` says
        "some harness error happened", and every harness error satisfies it — the shape
        that let one upstream wall discharge three separate declared debts on a sibling
        branch. Where the reason is the point, it is asserted; the count of cases that
        still assert only an exit code is reported in the round's audit rather than
        implied away.
        """
        return (code, needle in _drive.last_output)

    case.mark = _drive.calls
    _me_for_drive = (ROOT / "scripts/thesis_exit.py").read_text()
    # Assembled at run time; written whole each would occur twice and `mutate()` refuses an
    # ambiguous anchor.
    _fp_anchor = "if want_fp" + ".strip() != decl.strip():"
    _disp_anchor = "return p_effect_is_" + "transitive(effect_report(ctx, w), src)"

    def _raises_harness(fn):
        try:
            fn()
        except HarnessError:
            return True
        return False

    print("thesis-exit self-test — the GATE is driven where a gate-level answer is what")
    print("  is in question. MOST cases run main() against an injected repository state and")
    print("  assert its exit code (0 holds, 1 a finding, 2 cannot measure); the rest")
    print("  exercise one helper directly, and the split is reported in the summary.")

    print("\n  the gate must be capable of BOTH answers")
    case("an all-green repository state reaches EXIT 0", _drive(), 0)
    case("one RED row makes it exit 1",
         _why(_drive(verdicts=_verdict("N9-01", "OUTPUT_MISMATCH"))), '1 RED=N9-01')
    case("a conformance run with no parsable verdicts is exit 2, not a verdict",
         _why(_drive(verdicts="nothing parsable here")), '2 HARNESS=scripts/conformance.sh produced no verdict l')

    print("\n  conditions 2 and 3 — verdicts come from the harness that RUNS things")
    case("a reject twin the compiler ACCEPTED goes RED",
         _why(_drive(verdicts=_verdict("N8-08", "REJECT_ACCEPTED"))), '1 RED=N8-08')
    case("a fixture whose stdout differs goes RED",
         _why(_drive(verdicts=_verdict("N8-01", "OUTPUT_MISMATCH"))), '1 RED=N8-01')
    case("a DECLARED, ABSENT fixture goes RED — silence is not a pass",
         _why(_drive(verdicts=_verdict("N9-03", ""))), '1 RED=N9-03')
    # ONE real pinned path is mutated and the other five keep their correct declarations,
    # so the only thing that can turn this red is the fingerprint comparison. The previous
    # version handed in a map for a path that is not in the contract at all: all six real
    # rows then had no declaration, the run went red for THAT, and deleting the comparison
    # outright would not have turned it green.
    case("REJECTED for the WRONG reason goes RED (incidental unsupported syntax)",
         _why(_drive(fingerprints=mutate_fp("N9-06",
                                       "Unsupported type in reference parameter"))), '1 RED=N9-06')
    case("the other declarations are untouched by that mutation",
         len(GOOD_FP), sum(1 for k, _e, f in _C.values() if k == "reject" and f != "-"),
         drives_main=False)
    case("REJECTED at the fingerprint the row demands is green", _drive(), 0)

    print("\n  condition 1 — the witnesses, and the gates beneath them")
    case("a real `async fn` in a witness goes RED",
         _why(_drive(witness_b=GOOD_WITNESS + "async fn g() { }\n")), '1 RED=TH-01,TH-06')
    case("`fn q<'a>` goes RED",
         _why(_drive(witness_b=GOOD_WITNESS + "fn q<'a>(x: i64) -> i64 { return x; }\n")), '1 RED=TH-02,TH-06')
    case("`fn q< 'a>` SPACED goes RED — grammar.ebnf:157, and it compiles today",
         _why(_drive(witness_b=GOOD_WITNESS + "fn q< 'a>(x: i64) -> i64 { return x; }\n")), '1 RED=TH-02,TH-06')
    case("`myref<'a>` goes RED — the ref<'…> exemption needs an identifier boundary",
         _why(_drive(witness_b=GOOD_WITNESS + "fn myref<'a>(x: i64) -> i64 { return x; }\n")), '1 RED=TH-02,TH-06')
    case("`ref<'a> T` is PERMITTED by N9 and stays green",
         _drive(witness_b=mutate(GOOD_WITNESS, "x: ref String", "x: ref<'a> String")), 0)
    case("no `ref` PARAMETER (a struct field only) goes RED",
         _why(_drive(witness_b="struct S { x: ref String }\n" + mutate(
             mutate(GOOD_WITNESS, "fn drive(x: ref String, mut c: C)", "fn drive(mut c: C)"),
             "drive(s, c)", "drive(c)"))), '1 RED=TH-03,TH-06')
    # These two keep the REST of witness 2 green, so the only thing that can turn the run
    # red is the property under test. An earlier draft mutated the witness so heavily that
    # TH-05 failed too, and the cases passed for the wrong reason.
    case("a `ref` parameter only on an UNREACHABLE fn goes RED",
         _why(_drive(witness_b=mutate(
             mutate(GOOD_WITNESS, "fn drive(x: ref String, mut c: C)", "fn drive(mut c: C)"),
             "drive(s, c)", "drive(c)")
             + "fn ornament(x: ref String) -> i64 { return 1; }\n")), '1 RED=TH-03,TH-06')
    case("an effect chain only on UNREACHABLE functions goes RED",
         _why(_drive(witness_b="fn emit(mut c: C, s: String) { file_write(c.out, s); }\n"
                          "fn header(mut c: C) { emit(c, \"x\"); }\n"
                          "#[total]\nfn depth(n: i64) -> i64 { return n; }\n"
                          "fn drive(x: ref String) -> i64 { return depth(1); }\n"
                          "fn main() { drive(s); }\n",
                report_b="Function 'header' has effects: [Io]\n")), '1 RED=TH-05')
    case("a GENERIC function is visible to the model, not silently invisible (R3)",
         _drive(witness_b=mutate(GOOD_WITNESS, "fn drive(x: ref String, mut c: C)",
                                 "fn drive<T>(x: ref String, mut c: C)")), 0)
    case("trait-bound dispatch `T::m(…)` is a HARNESS ERROR, never a guess (R4) — and the "
         "output NAMES that reason, so an unrelated malfunction cannot satisfy this case",
         _because(_drive(witness_b=GOOD_WITNESS
                         + "fn show<T: Display>(x: T) { T::fmt(x); }\n"),
                  "dispatch through the type parameter"), (2, True))
    case("a `.`-method call is a HARNESS ERROR (R4), and says so",
         _because(_drive(witness_b=GOOD_WITNESS + "fn m(s: S) { s.len(); }\n"),
                  "a `.`-method call"), (2, True))
    case("a function-typed parameter is a HARNESS ERROR (R4), and says so",
         _because(_drive(witness_b=GOOD_WITNESS + "fn hof(f: fn(i64) -> i64) { }\n"),
                  "a function-typed parameter"), (2, True))
    case("`#[total]` named only by another function is NOT refuted (P1, not liveness)",
         _drive(witness_b=mutate(GOOD_WITNESS, "return depth(1);", "return 1;")
                          + "fn dead() -> i64 { return depth(2); }\n"), 0)
    case("`#[total]` on a self-recursive-only fn goes RED (a self-edge is not a name)",
         _why(_drive(witness_b=mutate(
             mutate(GOOD_WITNESS, "fn depth(n: i64) -> i64 { return n; }",
                    "fn depth(n: i64) -> i64 { return depth(n); }"),
             "return depth(1);", "return 1;"))), '1 RED=TH-04,TH-06')
    case("a missing witness is a FINDING (exit 1), not a malfunction",
         _why(_drive(drop_witness_b=True)), '1 RED=TH-01,TH-02,TH-03,TH-04,TH-05,TH-06')
    case("TH-05 reads before it compiles, so an ABSENT witness is a finding not a malfunction",
         _why(_drive(drop_witness_b=True, omit_report_b=True)), '1 RED=TH-01,TH-02,TH-03,TH-04,TH-05,TH-06')
    case("a witness that EXISTS but cannot be measured is exit 2, not a red row",
         _why(_drive(omit_report_b=True)), '2 HARNESS=no injected effect report for tests/witness/')
    case("`make selfhost` failing goes RED",
         _why(_drive(make={"selfhost": 1, "selfhost-corpus": 0, "thesis-exit": 0})),
         '1 RED=SH-01,SH-05')
    case("an absent make target goes RED",
         _why(_drive(make={"selfhost": 0, "thesis-exit": 0})),
         '1 RED=SH-02,SH-03,SH-04,SH-05')

    print("\n  P1/P2 — what a green verdict now means, and what it deliberately does not")
    _orn = ("fn ornament(x: ref String) -> i64 { return 1; }\n"
            "fn okpath(mut c: C) { header(c); }\n"
            "#[total]\nfn depth(n: i64) -> i64 { return n; }\n"
            "fn emit(mut c: C, s: String) { file_write(c.out, s); }\n"
            "fn header(mut c: C) { emit(c, \"x\"); }\n")
    # THE DELIBERATE LOSS, pinned so it cannot be forgotten or quietly re-claimed. A
    # decoration referenced only from a dead branch is NOT refuted any more: three
    # reachability heuristics each had a fail-open path, so the gate stopped guessing.
    # P1 is existence; liveness is GI-11's, and GI-11 is a thesis row, so the gate cannot
    # be green while that obligation is open.
    case("a dead-branch decoration is NOT refuted — liveness is not asserted (P1)",
         _drive(witness_b=_orn + "fn main() { if true { okpath(c); } else "
                                 "{ ornament(s); depth(1); } }\n"), 0)
    case("a decoration NOTHING names IS refuted — P2 is sound in that direction",
         _why(_drive(witness_b="fn ornament(x: ref String) -> i64 { return 1; }\n"
                          "#[total]\nfn depth(n: i64) -> i64 { return n; }\n"
                          "fn emit(mut c: C, s: String) { file_write(c.out, s); }\n"
                          "fn header(mut c: C) { emit(c, \"x\"); }\n"
                          "fn main() { header(c); }\n")), '1 RED=TH-03,TH-04,TH-06')
    case("the three control-flow shapes that broke the last model are now moot",
         _drive(witness_b=_orn + "fn main() { if true { return; } okpath(c); "
                                 "ornament(s); depth(1); }\n"), 0)

    print("\n  R4 — every closure form refuses, not only the brace form")
    for form, label in [("|x| ornament(x)", "brace/ident body"), ("|x| (ornament(x))", "paren body"),
                        ("|x| [ornament(x)]", "bracket body"), ("|x| -ornament(x)", "unary body")]:
        case(f"a closure with a {label} is a HARNESS ERROR, naming the closure",
             _because(_drive(witness_b=GOOD_WITNESS
                             + f"fn hof(mut c: C) {{ let f = {form}; }}\n"),
                      "a closure in any form"), (2, True))

    # THE MEASUREMENT BEHIND THE HELPER. An unrelated malfunction produces the SAME exit
    # code and none of the R4 wording, so `== 2` on its own does not distinguish the defect
    # a case is named for from any other defect. This is the sibling branch's `test-xfail`
    # finding, reproduced here rather than assumed not to apply.
    case("exit 2 alone does NOT discriminate: an unrelated malfunction is also exit 2, and "
         "carries none of the R4 reason — which is why the cases above assert the reason",
         _because(_drive(real_conformance="#!/bin/sh\necho 'f/ref.pd PASS_VERIFIED'\n"
                                          "kill -9 $$\n"),
                  "a closure in any form"), (2, False))

    print("\n  TH-05 — P2 applies to the caller (on the callee it is vacuous)")
    case("a caller nothing names cannot supply the exhibited edge (P2 on the caller)",
         _why(_drive(witness_b="fn ghost_io(mut c: C) { file_write(c.out, \"x\"); }\n"
                          "fn orphan(mut c: C) { ghost_io(c); }\n"
                          "#[total]\nfn depth(n: i64) -> i64 { return n; }\n"
                          "fn drive(x: ref String, mut c: C) -> i64 { return depth(1); }\n"
                          "fn main() { drive(s, c); }\n",
                report_b="Function 'orphan' has effects: [Io]\n")), '1 RED=TH-05')

    print("\n  TH-04 — a FUNCTION-level attribute, not the crate-level one")
    case("a witness carrying only `#![total]` does not satisfy TH-04",
         _why(_drive(witness_b=mutate(GOOD_WITNESS, "#[total]", "#![total]"))), '1 RED=TH-04,TH-06')

    print("\n  retracted claims stay retracted, and the disclaimers are pinned to OUTPUT")
    _self = (ROOT / "scripts/thesis_exit.py").read_text()
    # Exclude this table itself, or the check would flag its own banned list.
    _b, _e = "# BANNED-LIST-" + "BEGIN", "# BANNED-LIST-" + "END"
    assert _b in _self and _e in _self, "banned-list sentinels missing"
    _body = _self.split(_b)[0] + _self.split(_e)[1]
    case("no retracted claim survives anywhere in this file",
         stale_claims(_body), [], drives_main=False)
    case("the phrase check CATCHES a re-asserted claim",
         len(stale_claims("a docstring saying " + "REACHABLE" + " FROM main")), 1,
         drives_main=False)
    for doc in ("docs/contributing/MILESTONES.md", "scripts/thesis-exit.sh"):
        case(f"no retracted claim survives in {doc}",
             stale_claims((ROOT / doc).read_text()), [], drives_main=False)
    # RETRACTED PROPERTIES, added this round, with the control that they are caught. The
    # roadmap described `provably_dead` as reachability and as "actually called" for six
    # rounds; the code decides neither. DEAD_MECHANISMS could not have caught it — nothing
    # was deleted — which is the difference between the two lists.
    case("the roadmap's description of the liveness refutation is caught if it claims "
         "reachability again",
         len(stale_claims("we compute reach" + "ability from `main` here")), 1,
         drives_main=False)
    case("...and if it claims the function is called rather than mentioned",
         len(stale_claims("an attribute on a fn that" + " is actually called")), 1,
         drives_main=False)
    case("what the code actually decides: a bare MENTION refuses the refutation, so it is "
         "narrower than reachability and narrower than `called`",
         (provably_dead({"main": "let h = helper;", "helper": "return 1;"}, "helper"),
          provably_dead({"main": "return 1;", "helper": "return 1;"}, "helper")),
         (False, True), drives_main=False)

    # THE DEAD-MECHANISM SCAN (MUST-FIX 6). It matched exact case, it skipped this file —
    # the file these sentences most often live in — and no planted control ever ran, so
    # deleting the scan outright would have changed no result.
    # Assembled at run time, like the retracted-phrase control above it: written whole, the
    # planted sentence would be a real occurrence in a file this check now scans.
    _planted = "GI-11's evidence is the named test tests/n10_" + "callgraph.rs"
    case("a planted dead mechanism is caught IN THIS FILE — it was exempt, and the "
         "wording rounds 11 and 12 missed was in this file",
         [h.split(": ", 1)[1] for h in scan_claims("scripts/thesis_exit.py", _planted)],
         ["names the dead mechanism 'n10_" + "callgraph.rs' (the named test is no longer "
          "any row's evidence)"], drives_main=False)
    case("the scan is CASE-INSENSITIVE — the exact-case version let a capitalisation past",
         len(dead_mechanism_hits("see TESTS/N10_" + "CALLGRAPH.RS for GI-11")), 1,
         drives_main=False)
    case("a PARAPHRASE is not caught, which is why the claim says tokens and not "
         "mechanisms",
         dead_mechanism_hits("the Rust test that used to be GI-11's evidence"), [],
         drives_main=False)
    case("every file on the release path is scanned for dead mechanisms, this one "
         "included",
         sorted(CLAIM_SCANNED),
         ["docs/contributing/1.0-requirements.tsv", "docs/contributing/MILESTONES.md",
          "docs/specification/language-spec.md", "scripts/thesis-exit.sh",
          "scripts/thesis_exit.py"], drives_main=False)
    case("as committed, no file on that path names one",
         sorted(h for rel in CLAIM_SCANNED
                for h in scan_claims(
                    rel, (ROOT / rel).read_text(encoding="utf-8", errors="replace"))),
         [], drives_main=False)
    # The fact the removed `p_observable` entry was standing in for, pinned where it is
    # decidable rather than searched for in prose.
    case("GI-11 is adjudicated by this command, not by a named test — pinned, not grepped",
         _C["GI-11"], ("gate", "make thesis-exit", "-"), drives_main=False)
    # AND THE JUDGEMENT ON `p_observable` ITSELF, mechanized instead of argued — and stated
    # for what it is, which is weaker than "it is needed". The 18 manifest rows are DEMAND,
    # not LIVENESS: nothing dispatches them today, because `make v1-exit` (GI-10's evidence)
    # does not exist. So this is an INVENTORY-BACKED DEBT SIGNAL: the code is unreachable
    # from every gate that currently runs, it is retained because a named, counted set of
    # rows will need exactly this dispatch when the 1.0 gate is written, and if that set
    # ever empties the case below fails and the dispatch becomes a deletion candidate
    # loudly. Retention is a bet on GI-10, recorded here so it can be called in.
    _kinds = [r.split("\t") for r in
              (ROOT / "docs/contributing/1.0-requirements.tsv").read_text().split("\n")]
    _kinds = [f for f in _kinds if len(f) == 9 and not f[0].startswith("#")]
    case("no THESIS row is an `observable`, so `make thesis-exit` cannot reach that dispatch",
         [f[0] for f in _kinds if f[7] == "thesis" and f[4] == "observable"], [],
         drives_main=False)
    # 19 -> 20 on 2026-08-23: N2-10 ("attributes lex: #[name], #[name(args)],
    # #![name(args)]") changed evidence-kind from `fixture` to `observable
    # tests/m2_lexical.rs::every_attribute_shape_is_refused_by_name`. `fixture` was
    # UNSATISFIABLE rather than unmet — it demands a conformance row of class `run`, and
    # N2-11 makes every attribute a compile error, so no program containing one can run —
    # and no single reject fixture can carry a claim about THREE shapes. This is real
    # demand of the same kind as N14-01's: the row is `satisfied` today by a test
    # `make v1-exit` would have to dispatch, and the +1 is the price of the kind change,
    # recorded here rather than absorbed.
    #
    # 20 -> 22 on 2026-08-23, and this one is an ADDITION rather than a kind change:
    # N3-15 ("item order does not decide whether a program compiles, over the by-value
    # containment graph") and N4-22 ("an `enum` whose payload names its own enum is a
    # type") both landed as `observable`, because each is witnessed by a Rust integration
    # test asserting a runtime property stdout cannot show — a permutation over six
    # declaration orders, and a recursive value built, taken apart and summed. Neither
    # could be a `fixture`: a conformance row names ONE file in ONE order, which is
    # exactly the evidence shape N3-15 exists to reject.
    #
    # It went unnoticed for one round because that round ran a curated eight gates and
    # `make test-thesis-runner` was not among them. The count is the only thing that
    # notices an added row here, which is the whole reason it is pinned.
    #
    # 18 -> 19 on 2026-08-23: N14-01 ("the builtin set is exactly the 34 normative names")
    # changed evidence-kind from `gate make stdlib-gate` to
    # `observable src/builtins.rs::test_registry_is_exactly_the_normative_builtin_set`. Its
    # old evidence compared the registry against tests/stdlib/BUILTINS.tsv — a second copy
    # of the compiler's own opinion — and so could not have gone red on the defect the row
    # is about. The count is a DEMAND figure, and this one is real demand: the row is
    # `satisfied` today by a test `make v1-exit` would have to dispatch.
    case("the manifest carries `observable` rows nothing dispatches yet — DEMAND for the "
         "1.0 gate GI-10 owes, not evidence that this code is live; retention is debt "
         "against that row, and an empty set here makes it a deletion",
         len([f for f in _kinds if f[4] == "observable"]), 22, drives_main=False)
    case("...and the gate that would dispatch them does not exist yet, which is what makes "
         "this debt rather than liveness",
         (ROOT / "Makefile").read_text().count("\nv1-exit:"), 0, drives_main=False)
    _drive()
    case.mark = _drive.calls          # a free-standing drive belongs to no case
    out = _drive.last_output
    case("a green run SAYS liveness is not asserted, in its own output",
         "liveness is NOT asserted" in out, True, drives_main=False)
    case("a green run names the obligation that carries it",
         "GI-11" in out, True, drives_main=False)
    case("a green run states what THIS green means, derived from the wired models",
         "WHAT THIS GREEN MEANS" in out and LIVENESS_MODEL in out, True, drives_main=False)
    case("a green run does NOT repeat disclaimers its preconditions have retired",
         "liveness is NOT asserted" in out.split("WHAT THIS GREEN MEANS")[-1], False,
         drives_main=False)

    print("\n  the definition is INCOMPLETE, so no verdict is offered at all")
    case("with GI-11/GI-12 outstanding the gate REFUSES — exit 2, not a RED verdict",
         _why(_drive(definition_incomplete=True)), '2 BLOCKED=GI-11,GI-12')
    case("it refuses even when every scored row would pass",
         _why(_drive(definition_incomplete=True)), '2 BLOCKED=GI-11,GI-12')
    _drive(definition_incomplete=True)
    case.mark = _drive.calls          # likewise
    _out = _drive.last_output
    case("the refusal says the DEFINITION is incomplete, not that 1.0 is unreached",
         "THE DEFINITION OF 1.0 IS INCOMPLETE" in _out, True, drives_main=False)
    case("it still prints the per-row dashboard, so no progress signal is lost",
         "TH-06" in _out and "SH-01" in _out, True, drives_main=False)
    case("the refusal banner precedes the rows, so no reader meets a score first",
         _out.index("NO VERDICT IS AVAILABLE") < _out.index("SH-01"), True,
         drives_main=False)
    case("no aggregate tally is printed under refusal — a total is the quotable certificate",
         "of 22 evaluated rows would pass" in _out, False, drives_main=False)
    case("it names both outstanding preconditions",
         "GI-11" in _out and "GI-12" in _out, True, drives_main=False)
    case("the real run never assumes the definition is complete",
         Context().assume_definition_complete, False, drives_main=False)

    print("\n  a control that cannot fail is a harness error")
    case("no `case(...)` in this file contains `… or True` (not: no constant at all)",
         or_true_assertions((ROOT / "scripts/thesis_exit.py").read_text()), [],
         drives_main=False)
    case("the lint catches a planted `… or True`",
         len(or_true_assertions('case("x", all(y or True for y in z), True)')), 1,
         drives_main=False)

    print("\n  the CALL-GRAPH STRUCTURAL DIFFERENTIAL — GI-11's other half")
    _cg_fail, _cg_total = callgraph_differential()
    case("it pins graph OUTPUTS, which an empty #[test] cannot produce",
         _cg_total >= 5, True, drives_main=False)
    case("every structural row fails with no graph wired",
         len(_cg_fail), _cg_total, drives_main=False)
    case("the row set is exactly the nine reviewed ids — scoped identities, roots, "
         "order-independence, completion both ways, indirect on one site, on two distinct "
         "sites and on the SAME expression twice, provenance binding",
         sorted(EXPECTED_CALLGRAPH_IDS),
         ["completion-diverges", "completion-returns", "entry-roots", "indirect-declared",
          "indirect-multi-site", "indirect-repeated-site", "order-independent",
          "provenance-binding", "scoped-identity"], drives_main=False)
    case("`provenance` is no longer a property a provider can be asked for on its own",
         sorted({l.split("\t")[1] for l in CALLGRAPH_CORPUS.read_text().splitlines()
                 if l.strip() and not l.startswith("#")}),
         ["completion", "edges", "identical-to", "indirect", "roots"], drives_main=False)
    case("the residue it cannot pin is ONE clause, named — fault injection left the list "
         "because it is enforced",
         len(GI11_HUMAN_REVIEW_RESIDUE), 1, drives_main=False)
    case("fault injection is no longer called human review",
         any("fault injection" in r.lower() for r in GI11_HUMAN_REVIEW_RESIDUE), False,
         drives_main=False)
    case("every structural row carries a mutation, so none can be passed by a lookup table",
         all("=>" in l.split("\t")[4]
             for l in CALLGRAPH_CORPUS.read_text().splitlines()
             if l.strip() and not l.startswith("#")), True, drives_main=False)
    case("every structural row pins the answer the MUTATED program must give",
         all(l.split("\t")[5].strip()
             for l in CALLGRAPH_CORPUS.read_text().splitlines()
             if l.strip() and not l.startswith("#")), True, drives_main=False)

    print("\n  the MUTATION BRANCH, with providers injected — it never ran before")
    import hashlib as _h

    def _fam(row, seed="SEED"):
        src, exp, expm, mu = family_instance(row[2], row[3], row[5], row[4], seed)
        f, _, r = mu.partition("=>")
        return (src.replace("\\n", "\n"), exp, expm,
                src.replace("\\n", "\n").replace(f, r))

    _cg = {r[0]: r for r in
           (l.split("\t") for l in CALLGRAPH_CORPUS.read_text().splitlines()
            if l.strip() and not l.startswith("#"))}

    _ids = sorted(EXPECTED_CALLGRAPH_IDS)

    def _run(prov):
        """-> (failures, total) with `prov` injected as the provider."""
        globals()["CALLGRAPH_PROVIDER_OVERRIDE"] = prov
        try:
            return callgraph_differential()
        finally:
            globals()["CALLGRAPH_PROVIDER_OVERRIDE"] = None

    # Every headline figure this file quotes is produced HERE, by an adversary that runs,
    # and every score-shaped token in the files that carry these figures is checked against
    # what was measured. The ceiling number used to be asserted by finding it, as a literal,
    # in prose — and the prose then went stale by a row while the check looked elsewhere.
    _MEASURED = {}

    def _score(prov, label=None):
        fl, tot = _run(prov)
        if label:
            # DUPLICATES ARE A HARNESS ERROR, like duplicate case labels. `_MEASURED` is the
            # attribution mechanism — label -> score — and a dict assignment made it
            # last-write-wins, so two adversaries sharing a label would have silently
            # collapsed into one entry and the map would have attributed a score to whoever
            # ran last.
            if label in _MEASURED:
                raise HarnessError(
                    f"self-test: duplicate adversary label {label!r}. The scoreboard is a "
                    "one-to-one attribution; give the new adversary its own name.")
            _MEASURED[label] = (tot - len(fl), tot)
        return tot - len(fl), tot

    def _stages(prov):
        """{row id: the stage it failed at}. PER ROW, WITH ITS REASON — the assertion the
        mutation controls lacked. `score < total` was satisfied by ONE row failing, so an
        adversary wrong on six rows and a mutation branch that never ran looked the same."""
        return {rid: stage for rid, stage, _w, _g in _run(prov)[0]}

    def _reasons(prov):
        return {rid: got for rid, _s, _w, got in _run(prov)[0]}

    _PROPS = ("edges", "roots", "completion", "indirect")

    def _bound(fn):
        """A value-only provider, wrapped into the SNAPSHOT interface — one object per unit,
        `(provenance, {property: value})`, with an honest digest of whatever it was handed.

        NAMED FOR WHAT IT PROVES, WHICH IS NOTHING ABOUT THE PROVIDER. Handing a lookup a
        correct digest is free, and reviewers used this very function to refute the round-15
        sentence "the coupling is what makes a stale graph fail": stale VALUES are caught by
        the ordinary expectation checks, never by the digest. `_bound` is kept deliberately
        as the standing demonstration that provenance establishes origin-labelling and
        single-object projection, and not derivation.
        """
        def _p(s):
            graph = {prop: fn(s, prop) for prop in _PROPS}
            graph = {k: v for k, v in graph.items() if v is not None}
            return (_h.sha256(s.encode()).hexdigest(), graph) if graph else None
        return _p

    def _expectation_oracle(src, prop, seed_env):
        """AN EXACT-SOURCE LOOKUP BUILT FROM THE CORPUS'S OWN EXPECTATIONS.

        Named for what it is. It was called "a CORRECT provider", which reads as "a correct
        implementation" — it is neither an implementation nor evidence that one can exist.
        Scoring full marks with it proves the checks are MUTUALLY SATISFIABLE: no row demands
        something another row forbids, so a red run means a defect and not an unsatisfiable
        corpus. That is worth having and is all it is.
        """
        # `identical-to` asks for `edges` on the OTHER row's source too, so the oracle has
        # to answer that even though the other row's own property is `roots`.
        if prop == "edges":
            for rid in ("entry-roots", "order-independent"):
                o, _e, expm, mut = _fam(_cg[rid], seed_env)
                if src == o:
                    return f"main>helper_{seed_env}"
                if src == mut:
                    return expm if rid == "order-independent" else "(none)"
        for rid, row in _cg.items():
            o, exp, expm, mut = _fam(row, seed_env)
            want_prop = "edges" if row[1] == "identical-to" else row[1]
            if prop != want_prop:
                continue
            if src == o:
                return (exp.split("|")[0] if row[1] != "identical-to" else "main>helper_" + seed_env)
            if src == mut:
                return (expm.split("|")[0] if row[1] != "identical-to" else expm)
        return None

    import os as _os
    _os.environ["THESIS_FAMILY_SEED"] = "SEED"
    try:
        _honest = _bound(lambda s, p: _expectation_oracle(s, p, "SEED"))
        _ok, _tot = _score(_honest, "the expectation oracle")
        case("the EXPECTATION ORACLE passes every row — an exact-source lookup built from "
             "the corpus, so this proves the checks are mutually satisfiable and nothing "
             "about any implementation",
             (_ok, _tot), (CALLGRAPH_ROWS, CALLGRAPH_ROWS), drives_main=False)

        # ONE SNAPSHOT PER UNIT, AND THE GATE PROVES IT BY COUNTING (MUST-FIX 1). The claim
        # is about the GATE's behaviour, not the provider's honesty: if the runner asks once
        # per unit and projects, then no second call can hand back a different snapshot
        # wearing the same digest. Two properties of one unit — `entry-roots`' program is
        # read for `roots` by its own row and for `edges` by `order-independent`'s
        # comparison — come from a single call, which is what the count shows.
        _calls = []

        def _counting(s):
            _calls.append(s)
            return _honest(s)
        _run(_counting)
        # DERIVED, not written down: the units the corpus submits are one original and one
        # mutation per row, minus any the rows share. Writing `16` was a figure with no
        # owner, which is the class this round is about.
        _units = set()
        for _row in _cg.values():
            _o, _e, _em, _m = _fam(_row, "SEED")
            _units |= {_o, _m}
        case("the gate asks the provider ONCE PER DISTINCT UNIT and reads every property "
             "from that one container — no call is repeated, so two projections of one "
             "unit cannot disagree",
             (len(_calls), len(set(_calls))), (len(_units), len(_units)),
             drives_main=False)
        case("...and the unit count is one original plus one mutation per row, with NO "
             "extra call for the second property of `entry-roots`' program",
             len(_units), 2 * _tot, drives_main=False)

        # right on the original, wrong on the mutation: the branch that never ran
        def _wrong_mut(s):
            for rid, row in _cg.items():
                if s == _fam(row, "SEED")[3]:
                    return (_h.sha256(s.encode()).hexdigest(),
                            {prop: "WRONG" for prop in _PROPS})
            return _honest(s)
        _score(_wrong_mut, "right on the original, wrong on every mutation")
        case("right on the original, WRONG on the mutation -> EVERY row fails, each at "
             "the MUTATION stage",
             _stages(_wrong_mut), {rid: "mutation" for rid in _ids}, drives_main=False)
        case("...and every one of them names the wrong mutated answer as its reason",
             sorted(r for r, g in _reasons(_wrong_mut).items() if "WRONG" in g), _ids,
             drives_main=False)

        # THE FIGURE THE ROADMAP QUOTED WITH NO CONTROL BEHIND IT (MUST-FIX 3). An adversary
        # wrong on exactly ONE mutation is the thing `score < total` could not tell from an
        # adversary wrong on all of them, and it was quoted as measured while nothing ran it.
        _one_bad = _fam(_cg["scoped-identity"], "SEED")[3]

        def _one_row_wrong(s):
            if s == _one_bad:
                return (_h.sha256(s.encode()).hexdigest(),
                        {prop: "WRONG" for prop in _PROPS})
            return _honest(s)
        case("an adversary wrong on EXACTLY ONE mutation scores one short of full marks — "
             "the figure the roadmap quoted, now produced by something that runs",
             _score(_one_row_wrong, "wrong on exactly one mutation"),
             (CALLGRAPH_ROWS - 1, CALLGRAPH_ROWS), drives_main=False)
        case("...and it fails exactly that one row, at the MUTATION stage — which is what "
             "`score < total` could not distinguish from failing all eight",
             _stages(_one_row_wrong), {"scoped-identity": "mutation"}, drives_main=False)

        def _silent_mut(s):
            for rid, row in _cg.items():
                if s == _fam(row, "SEED")[3]:
                    return (_h.sha256(s.encode()).hexdigest(), {})
            return _honest(s)
        _score(_silent_mut, "right on the original, silent on every mutation")
        case("right on the original, SILENT on the mutation -> EVERY row fails at the "
             "MUTATION stage",
             _stages(_silent_mut), {rid: "mutation" for rid in _ids}, drives_main=False)
        case("...and every one of them names silence, not a wrong answer, as its reason",
             sorted(r for r, g in _reasons(_silent_mut).items()
                    if g.startswith("silence —")), _ids, drives_main=False)

        # A CONSTANT and a SILENT graph never reach the mutation branch, and the exact map
        # says so rather than a total hiding it. `order-independent` USED TO BE the
        # exception — two identical wrong answers are identical, so a constant cleared the
        # identical-to comparison and only died on the mutation. Measured by review with
        # the literal `CONST`, and closed by pinning an edge set on that row rather than
        # only equality between the two units. Every row dies at the ORIGINAL stage now.
        _const = _bound(lambda s, p: "main>x")
        _score(_const, "a constant graph")
        case("a constant graph fails EVERY row at the ORIGINAL stage — the identical-to row "
             "no longer passes on equality between two identical wrong answers",
             _stages(_const), {rid: "original" for rid in _ids}, drives_main=False)
        # THE CONTROL FOR THAT FIX: equality alone is not an answer. A provider returning
        # ONE constant for both units of the identical-to row satisfies `a == b`; the pinned
        # edge set is what refuses it.
        def _equal_but_wrong(s):
            got = _honest(s)
            if got is None:
                return None
            return (got[0], {**got[1], "edges": "CONST"})
        case("a provider answering ONE CONSTANT for both units of the identical-to row is "
             "refused at the ORIGINAL stage — measured to pass when only equality was "
             "compared",
             _stages(_equal_but_wrong).get("order-independent"), "original",
             drives_main=False)
        _silent = lambda s: None                                          # noqa: E731
        _score(_silent, "a silent graph")
        case("a silent graph fails every row at the ORIGINAL stage, for want of a graph",
             _stages(_silent), {rid: "original" for rid in _ids}, drives_main=False)
        case("...and says so, rather than reporting a wrong answer",
             sorted(r for r, g in _reasons(_silent).items()
                    if g == "no call graph is wired"), _ids, drives_main=False)

        # WHAT PROVENANCE DOES AND DOES NOT CATCH (MUST-FIX 1). All three of these carry a
        # CORRECT graph for every row: they pass every edge, root, completion and indirect
        # check and fail only on the shape or the label of the object carrying them. None of
        # them shows that provenance catches a stale VALUE — nothing here does, because it
        # does not: `_bound` above hashes whatever it is handed and passes.
        _unbound = lambda s: {p: _expectation_oracle(s, p, "SEED") for p in _PROPS}
        _score(_unbound, "a correct graph returned without provenance")
        case("a CORRECT graph returned as a bare map — no provenance — fails every row",
             _stages(_unbound), {rid: "original" for rid in _ids}, drives_main=False)

        def _stale_prov(s):
            got = _honest(s)
            return None if got is None else (
                _h.sha256(b"a unit this graph is not about").hexdigest(), got[1])
        _score(_stale_prov, "a correct graph carrying another unit's provenance")
        case("a CORRECT graph carrying ANOTHER UNIT'S provenance fails every row — a "
             "snapshot cannot wear an identity that is not the unit's",
             _stages(_stale_prov), {rid: "original" for rid in _ids}, drives_main=False)
        case("...and the reason names the binding, not the graph",
             sorted(r for r, g in _reasons(_stale_prov).items()
                    if "not bound to the program asked about" in g), _ids, drives_main=False)

        # THE TWO COUNTEREXAMPLES THE SENTENCE NAMES (MUST-FIX 2). Both PASS, on purpose:
        # they are what makes "one invocation, projected from a copy" the honest reading and
        # "the provider assembled one snapshot" the reading the code cannot support.
        _world = {"reads": 0}

        def _per_property_assembly(s):
            """Assembles the container by reading a CHANGING world once per property."""
            graph = {}
            for prop in _PROPS:
                _world["reads"] += 1          # the world moves between property reads
                v = _expectation_oracle(s, prop, "SEED")
                if v is not None:
                    graph[prop] = v
            return (_h.sha256(s.encode()).hexdigest(), graph) if graph else None
        case("a container ASSEMBLED PER PROPERTY from a changing world scores full marks — "
             "the gate cannot see how it was built, which is why the sentence claims one "
             "INVOCATION and not one observation",
             _score(_per_property_assembly, "a container assembled per property"),
             (CALLGRAPH_ROWS, CALLGRAPH_ROWS), drives_main=False)
        case("...and it really did observe more states than there were units — a vacuous "
             "counterexample would prove nothing",
             _world["reads"] > len(_units), True, drives_main=False)

        _handed = []

        def _mutates_after_return(s):
            """Hands back a live reference, then corrupts everything handed out so far.

            The window this opens is real and the corpus has one: `entry-roots`' program is
            snapshotted for its own `roots` row and projected again, from the CACHE, for
            `order-independent`'s `edges` comparison several rows later — by which time this
            provider has corrupted the container it returned. Holding the provider's
            reference, the second projection reads the corruption.
            """
            got = _honest(s)
            if got is None:
                return None
            live = dict(got[1])
            for prev in _handed:
                prev.clear()
                prev["edges"] = "CORRUPTED"
            _handed.append(live)
            return (got[0], live)
        case("a provider that CORRUPTS the container after returning it cannot change the "
             "gate's answers — the container is copied at return, so the reads are atomic "
             "with respect to it even across the cache",
             _score(_mutates_after_return, "a provider that corrupts what it returned"),
             (CALLGRAPH_ROWS, CALLGRAPH_ROWS), drives_main=False)
        case("...and the corruption was real, not a no-op: every container it handed out "
             "but the last was emptied",
             sorted({str(_l.get("edges")) for _l in _handed[:-1]}), ["CORRUPTED"],
             drives_main=False)

        # THE COPY IS SHALLOW, SO THE VALUES MUST BE IMMUTABLE (MUST-FIX 2). A list as a
        # value stays shared with the provider, and `dict(graph)` copies the reference —
        # so "a provider's live references cannot make two projections disagree" was true
        # of the top level only. Values are now required to be strings, and a read-only
        # mapping is accepted where a `dict` was demanded.
        def _mutable_values(s):
            got = _honest(s)
            return None if got is None else (got[0], {k: [v] for k, v in got[1].items()})
        _score(_mutable_values, "a snapshot whose values are mutable")
        case("a snapshot with a MUTABLE value fails every row — a shallow copy of the map "
             "is not a copy of the graph",
             _stages(_mutable_values), {rid: "original" for rid in _ids}, drives_main=False)
        case("...and the reason names the value, not the map",
             sorted(r for r, g in _reasons(_mutable_values).items()
                    if "are not strings" in g), _ids, drives_main=False)

        def _readonly_map(s):
            got = _honest(s)
            return None if got is None else (got[0], MappingProxyType(dict(got[1])))
        case("a READ-ONLY mapping is accepted — the boundary asks for a property map, and "
             "demanding a concrete `dict` refused the shape a careful provider would send",
             _score(_readonly_map, "a snapshot delivered as a read-only mapping"),
             (CALLGRAPH_ROWS, CALLGRAPH_ROWS), drives_main=False)

        def _not_a_map(s):
            got = _honest(s)
            return None if got is None else (got[0], "main>helper_SEED")
        _score(_not_a_map, "a snapshot that is not a property map")
        case("a snapshot that is not a PROPERTY MAP fails every row — the properties have "
             "to be projectable from one object, not concatenated into one string",
             _stages(_not_a_map), {rid: "original" for rid in _ids}, drives_main=False)

        # THE INDIRECT CONTRACT, BOTH CLAUSES (MUST-FIX 2), NOW PER SITE. Settled as TWO
        # situations: a site whose target cannot be determined may be declared `unresolved`,
        # and a site that claims resolution while naming NO target cannot be scored at all.
        def _grammar_raises(v):
            try:
                indirect_grammar_or_raise("indirect-declared", v)
            except HarnessError:
                return True
            return False
        case("the grammar refuses a site that claims `resolved:` and names nothing",
             _grammar_raises("run#1=resolved:"), True, drives_main=False)
        case("the grammar refuses it carrying only separators",
             _grammar_raises("run#1=resolved: ; "), True, drives_main=False)
        case("the grammar refuses ONE bad site among good ones — it is applied per entry, "
             "which a scalar answer could not express",
             _grammar_raises("run#1=resolved:target,run#2=resolved:"), True,
             drives_main=False)
        case("the grammar admits a site resolved to one target",
             _grammar_raises("run#1=resolved:target"), False, drives_main=False)
        case("the grammar admits a site resolved to TWO targets",
             _grammar_raises("run#1=resolved:target;other"), False, drives_main=False)
        case("the grammar admits `unresolved`",
             _grammar_raises("run#1=unresolved"), False, drives_main=False)
        case("the grammar admits `(none)` — a unit with no indirect site at all",
             _grammar_raises("(none)"), False, drives_main=False)
        case("the grammar leaves everything else to the SCORE — it is not a second "
             "wrong-answer check",
             _grammar_raises("maybe"), False, drives_main=False)
        case("a scalar answer parses as ONE UNKEYED ENTRY, so it can never satisfy a "
             "site-keyed row",
             indirect_entries("resolved:target"), [("", "resolved:target")],
             drives_main=False)
        case("two sites parse as two entries",
             indirect_entries("run#1=resolved:target,run#2=unresolved"),
             [("run#1", "resolved:target"), ("run#2", "unresolved")], drives_main=False)

        # THE KEY IS A POSITION, AND THE ALPHABET IS WHY THE ENCODING IS INJECTIVE
        # (MUST-FIX 3). The old key embedded a callee EXPRESSION in a string delimited by
        # `,` `=` `;` `|` with no escaping, so an expression containing any of them could
        # not round-trip and two different answers could parse to the same entries.
        case("every site key the CORPUS pins is well-formed, so no row is unanswerable by "
             "construction",
             sorted({site for _r in _cg.values() if _r[1] == "indirect"
                     for _alt in (_r[3] + "|" + _r[5]).split("|")
                     for site, _st in indirect_entries(_alt) if site}
                    - {s for s in
                       {site for _r in _cg.values() if _r[1] == "indirect"
                        for _alt in (_r[3] + "|" + _r[5]).split("|")
                        for site, _st in indirect_entries(_alt) if site}
                       if indirect_site_wellformed(s)}), [], drives_main=False)
        case("a caller and an index is well-formed; a scoped caller too",
             [indirect_site_wellformed("run#1"), indirect_site_wellformed("Buf::len#2")],
             [True, True], drives_main=False)
        case("a key carrying a DELIMITER is not well-formed — that is the round-trip the "
             "old callee-expression key could not survive",
             [indirect_site_wellformed("run>a,b()"), indirect_site_wellformed("run>x=y()"),
              indirect_site_wellformed("run>f")], [False, False, False], drives_main=False)
        case("...and it is not merely ugly: an answer keyed on `a,b()` PARSES AS TWO "
             "ENTRIES, so two different answers collapse to the same reading",
             indirect_entries("run>a,b()=resolved:t"),
             [("", "run>a"), ("b()", "resolved:t")], drives_main=False)
        case("the well-formed encoding round-trips: entries in, same entries out",
             indirect_entries(",".join(f"{s}={st}" for s, st in
                                       [("Buf::len#1", "resolved:a;b"), ("run#2", "unresolved")])),
             [("Buf::len#1", "resolved:a;b"), ("run#2", "unresolved")], drives_main=False)

        def _ind(alt, rid="indirect-declared", mutated_alt=None):
            """Override the `indirect` answer for a row's ORIGINAL unit, and — when
            `mutated_alt` is given — for its MUTATED unit as well.

            THE SECOND PARAMETER IS THE POINT. Without it, a case about what the MUTATION
            must answer could only ever drive the original: the mutated unit kept using the
            honest oracle, returned the expected value, and the row passed — which is what
            a case labelled "keeping both entries fails at the mutation stage" was actually
            asserting, one row over from the thing it named.
            """
            _u, _, _, _m = _fam(_cg[rid], "SEED")

            def _p(s):
                got = _honest(s)
                if got is None:
                    return got
                if s == _u:
                    return (got[0], {**got[1], "indirect": alt})
                if mutated_alt is not None and s == _m:
                    return (got[0], {**got[1], "indirect": mutated_alt})
                return got
            return _p

        def _ind_outcome(alt, rid="indirect-declared", mutated_alt=None):
            """-> `passes`, the stage it failed at, or `harness failure`."""
            try:
                fl, _t = _run(_ind(alt, rid, mutated_alt))
            except HarnessError:
                return "harness failure"
            for _rid, stage, _w, _g in fl:
                if _rid == rid:
                    return stage
            return "passes"
        case("clause one: a site resolved to a target passes",
             _ind_outcome("run_SEED#1=resolved:target_SEED"), "passes",
             drives_main=False)
        case("clause one: `unresolved` passes — a site whose target cannot be determined "
             "is a fact about the program, honestly reported",
             _ind_outcome("run_SEED#1=unresolved"), "passes", drives_main=False)
        case("clause two: a site claiming resolution with NO target is a HARNESS FAILURE — "
             "the gate cannot tell a resolution from a declination, so it refuses to score",
             _ind_outcome("run_SEED#1=resolved:"), "harness failure", drives_main=False)
        case("and OMISSION is the other situation, SCORED not refused: a graph that drops "
             "the call site fails the row",
             _ind_outcome("(none)"), "original", drives_main=False)
        case("an answer that is neither resolution nor declination is scored, not refused",
             _ind_outcome("maybe"), "original", drives_main=False)
        case("THE SITE KEY IS LOAD-BEARING: the old scalar answer, correct in every other "
             "respect, no longer satisfies the single-site row",
             _ind_outcome("resolved:target_SEED"), "original", drives_main=False)

        # THE MULTI-SITE ROW — what a scalar answer could not express at all.
        _MS = "indirect-multi-site"
        _ms = "run_SEED#1={},run_SEED#2={}"
        case("two sites, both resolved, passes",
             _ind_outcome(_ms.format("resolved:target_SEED", "resolved:other_SEED"), _MS),
             "passes", drives_main=False)
        case("two sites, one resolved and one declared, passes — the contract is per site, "
             "so a provider may resolve what it can and declare what it cannot",
             _ind_outcome(_ms.format("resolved:target_SEED", "unresolved"), _MS), "passes",
             drives_main=False)
        case("DROPPING ONE OF TWO SITES fails the row — the omission a single-site corpus "
             "could not detect",
             _ind_outcome("run_SEED#1=resolved:target_SEED", _MS), "original",
             drives_main=False)
        case("`resolved:a;b` on ONE site is not the same answer as one target on each of "
             "two — the ambiguity the scalar encoding could not resolve",
             _ind_outcome("run_SEED#1=resolved:target_SEED;other_SEED", _MS),
             "original", drives_main=False)
        case("one bad site among two is a HARNESS FAILURE, not a score",
             _ind_outcome(_ms.format("resolved:target_SEED", "resolved:"), _MS),
             "harness failure", drives_main=False)

        # THE SAME EXPRESSION TWICE (MUST-FIX 4). The `#<n>` convention was documented as a
        # disambiguator and then pinned by no program: the multi-site row calls two DIFFERENT
        # expressions, so nothing fixed the numbering, the ordering, or what a mutation does
        # to it. This row calls the same parameter twice, so the index is the only thing that
        # can tell the two sites apart.
        _RS = "indirect-repeated-site"
        _rs = "twice_SEED#1={},twice_SEED#2={}"
        case("two sites through the SAME expression, both resolved, passes — the index is "
             "an identity, not a decoration",
             _ind_outcome(_rs.format("resolved:target_SEED", "resolved:target_SEED"), _RS),
             "passes", drives_main=False)
        case("...and they may be answered differently from each other, because they are "
             "different sites",
             _ind_outcome(_rs.format("resolved:target_SEED", "unresolved"), _RS), "passes",
             drives_main=False)
        case("collapsing the two same-expression sites into ONE entry fails the row — "
             "exactly what a callee-expression key would have been forced to do",
             _ind_outcome("twice_SEED#1=resolved:target_SEED", _RS), "original",
             drives_main=False)
        case("numbering them from zero fails the row, so the ORDINAL is pinned and not "
             "merely the count",
             _ind_outcome("twice_SEED#0=resolved:target_SEED,"
                          "twice_SEED#1=resolved:target_SEED", _RS), "original",
             drives_main=False)
        # THE MUTATION DELETES ONE CALL, so exactly one entry may remain — and this case
        # now INJECTS the answer into the mutated unit, which is what its label always
        # said and its predecessor never did.
        case("and the MUTATION deletes one call: keeping BOTH entries fails, at the "
             "MUTATION stage",
             _ind_outcome(_rs.format("resolved:target_SEED", "resolved:target_SEED"), _RS,
                          mutated_alt=_rs.format("resolved:target_SEED",
                                                 "resolved:target_SEED")),
             "mutation", drives_main=False)
        case("...while answering the mutated unit with the single surviving site passes, "
             "so the case above fails for the RIGHT reason",
             _ind_outcome(_rs.format("resolved:target_SEED", "resolved:target_SEED"), _RS,
                          mutated_alt="twice_SEED#1=resolved:target_SEED"),
             "passes", drives_main=False)
        case("and the same injection point works on the two-DISTINCT-site row: keeping the "
             "deleted site fails at the MUTATION stage",
             _ind_outcome(_ms.format("resolved:target_SEED", "resolved:other_SEED"), _MS,
                          mutated_alt=_ms.format("resolved:target_SEED",
                                                 "resolved:other_SEED")),
             "mutation", drives_main=False)

        # THE MEASUREMENT: a table of every program and every mutation
        _tbl = {}
        for rid, row in _cg.items():
            o, exp, expm, mut = _fam(row, "SEED")
            wp = "edges" if row[1] == "identical-to" else row[1]
            _tbl[(o, wp)] = exp.split("|")[0] if row[1] != "identical-to" else "main>helper_SEED"
            _tbl[(mut, wp)] = expm.split("|")[0]
        # the adversary also reads the identical-to comparison and tabulates its edges
        _tbl[(_fam(_cg["entry-roots"], "SEED")[0], "edges")] = "main>helper_SEED"
        _exact = _bound(lambda s, p: _tbl.get((s, p)))
        case("a table of ALL originals AND ALL mutations still passes every row — a finite "
             "public corpus cannot defeat a reader, and hashing its input satisfies "
             "provenance on the way past",
             _score(_exact, "an exact-source table, seed pinned"),
             (CALLGRAPH_ROWS, CALLGRAPH_ROWS), drives_main=False)
    finally:
        _os.environ.pop("THESIS_FAMILY_SEED", None)
    case("with a FRESH seed per run that same table scores exactly zero",
         _score(_exact, "the same table under a fresh seed"), (0, CALLGRAPH_ROWS),
         drives_main=False)

    # THE CEILING, EXECUTABLE (MUST-FIX 5 of round 15). Its number was quoted in the gate's
    # output and tested by finding that number, as a literal, in prose; no adversary ran.
    _SEEDPAT = re.compile(r"_(?:m[0-9a-f]{6}|SEED)\b")
    _norm_tbl = {(_SEEDPAT.sub("", src), p): val.replace("_SEED", "\x00")
                 for (src, p), val in _tbl.items()}

    def _norm_lookup(s, p):
        """Normalise identifiers, look up, re-apply the run's suffix. NOT an implementation
        of anything — it computes no graph — and it scores full marks, which is why the
        residue says these rows reject SPECIFIC strategies rather than wrong ones."""
        tmpl = _norm_tbl.get((_SEEDPAT.sub("", s), p))
        if tmpl is None:
            return None
        found = re.search(r"_(m[0-9a-f]{6}|SEED)\b", s)
        return tmpl.replace("\x00", "_" + found.group(1) if found else "")
    _normalising = _bound(_norm_lookup)
    case("an adversary that NORMALISES identifiers, looks up and re-suffixes scores FULL "
         "MARKS under a fresh seed — the metamorphic family's ceiling, measured rather "
         "than asserted, and it satisfies the provenance obligation by hashing whatever it "
         "is handed",
         _score(_normalising, "an identifier-normalising adversary"),
         (CALLGRAPH_ROWS, CALLGRAPH_ROWS), drives_main=False)

    # THE SCOREBOARD, PINNED BY LABEL. Round 15 compared SETS of score tokens, so a figure
    # could be attributed to the wrong adversary and pass; round 16 widened the scan to the
    # roadmap but not to THIS FILE, which was still quoting the previous corpus size in four
    # places. Scope, attribution, encoding — and the fix for a scope failure had the same
    # scope failure one file over. So: derive the numbers, and check every file that carries
    # one.
    case("the adversary scoreboard is exactly this — label -> score, so attribution is "
         "checked and not merely membership in a set of numbers",
         _MEASURED,
         {"the expectation oracle": (CALLGRAPH_ROWS, CALLGRAPH_ROWS),
          "right on the original, wrong on every mutation": (0, CALLGRAPH_ROWS),
          "wrong on exactly one mutation": (CALLGRAPH_ROWS - 1, CALLGRAPH_ROWS),
          "right on the original, silent on every mutation": (0, CALLGRAPH_ROWS),
          "a constant graph": (0, CALLGRAPH_ROWS),
          "a silent graph": (0, CALLGRAPH_ROWS),
          "a correct graph returned without provenance": (0, CALLGRAPH_ROWS),
          "a correct graph carrying another unit's provenance": (0, CALLGRAPH_ROWS),
          "a container assembled per property": (CALLGRAPH_ROWS, CALLGRAPH_ROWS),
          "a provider that corrupts what it returned": (CALLGRAPH_ROWS, CALLGRAPH_ROWS),
          "a snapshot whose values are mutable": (0, CALLGRAPH_ROWS),
          "a snapshot delivered as a read-only mapping": (CALLGRAPH_ROWS, CALLGRAPH_ROWS),
          "a snapshot that is not a property map": (0, CALLGRAPH_ROWS),
          "an exact-source table, seed pinned": (CALLGRAPH_ROWS, CALLGRAPH_ROWS),
          "the same table under a fresh seed": (0, CALLGRAPH_ROWS),
          "an identifier-normalising adversary": (CALLGRAPH_ROWS, CALLGRAPH_ROWS)},
         drives_main=False)
    case("the residual boundary is stated as ONE thing",
         len(GI11_HUMAN_REVIEW_RESIDUE), 1, drives_main=False)
    case("every figure the RESIDUE quotes is a score measured this run (a set check over "
         "ASCII `N/N` tokens — it cannot tell which adversary produced which, which is "
         "what the scoreboard above is for)",
         sorted(set(re.findall(r"\b\d+/\d+\b", GI11_HUMAN_REVIEW_RESIDUE[0]))),
         sorted({f"{ok}/{tot}" for ok, tot in _MEASURED.values()}), drives_main=False)
    # THE SCOPE FIX, APPLIED TO ITS OWN FILE THIS TIME (MUST-FIX 1). Round 15 checked the
    # residue string; round 16 added the roadmap; neither looked at the script, which was
    # quoting a superseded corpus size four lines above the check. Every score-shaped token
    # in every file that carries one must be a score measured this run or a pinned non-score
    # token — and the current figures are DERIVED from CALLGRAPH_ROWS, so they cannot rot in
    # the first place.
    _measured_tokens = {f"{ok}/{tot}" for ok, tot in _MEASURED.values()}
    _scan = score_bearing_files()
    # THREE FILES JOINED THIS SET on 2026-08-23, and their membership is the review this
    # pin exists to force. All three belong to the M2 milestone exit (GI-08/GI-09) and all
    # three cite `thesis-exit` for the same two reasons: to say what they are NOT — they
    # answer "does milestone X still owe a row", never "is 1.0 real" — and because they
    # copy this gate's three-valued exit contract, where NO_VERDICT is a distinct code from
    # FALSE, rather than inventing a second dialect for it.
    #   scripts/requirements.py              the inventory reader
    #   scripts/m2-exit.sh                   the aggregator that publishes the tri-state
    #   scripts/test-requirements-runner.sh  GI-09, which drives both
    # None carries an adversary score, so the backstop below has nothing to find in them.
    case("the scanned set is DERIVED from the tree, and is the reviewed one — a hand list "
         "of four was missing three files that cite this gate",
         _scan,
         ["Makefile", "docs/contributing/1.0-requirements.tsv",
          "docs/contributing/MILESTONES.md", "scripts/m2-exit.sh",
          "scripts/requirements.py", "scripts/test-requirements-runner.sh",
          "scripts/thesis-exit.sh", "scripts/thesis_exit.py",
          "tests/callgraph-differential.tsv",
          "tests/liveness-differential.tsv"], drives_main=False)

    def _outside_block(text):
        if SCOREBOARD_BEGIN in text and SCOREBOARD_END in text:
            return text[:text.index(SCOREBOARD_BEGIN)] + text[text.index(SCOREBOARD_END):]
        return text
    _stale = {rel: sorted(score_shaped_tokens(_outside_block((ROOT / rel).read_text()))
                          - _measured_tokens) for rel in _scan}
    case("no unmeasured figure survives in any file that cites this gate — a BACKSTOP over "
         "loose prose, which cannot tell which adversary a figure belongs to; the "
         "scoreboard below is what does that",
         {rel: bad for rel, bad in _stale.items() if bad}, {}, drives_main=False)

    # ATTRIBUTION (MUST-FIX 3). Membership in the measured set let a hand-written figure
    # pass beside an adversary that never produced it. The roadmap now carries a GENERATED
    # block: every figure sits in the row of the label that produced it, and the block is
    # byte-compared against a fresh render of this run's scoreboard.
    _road_text = (ROOT / "docs/contributing/MILESTONES.md").read_text()
    _want_block = scoreboard_block(_MEASURED)
    _have_block = (_road_text[_road_text.index(SCOREBOARD_BEGIN):
                              _road_text.index(SCOREBOARD_END) + len(SCOREBOARD_END)]
                   if SCOREBOARD_BEGIN in _road_text and SCOREBOARD_END in _road_text
                   else "(the generated scoreboard block is missing)")
    case("the roadmap's scoreboard is byte-identical to a fresh render of this run's "
         "measurements — a figure cannot be transcribed, mis-attributed or left behind",
         _have_block, _want_block, drives_main=False)
    case("editing one score in that block is caught",
         _have_block.replace("/", "/9", 1) == _want_block, False, drives_main=False)
    # AN ANCHOR MUST NOT BE WEAKER THAN WHAT IT ANCHORS. A byte-comparison against a
    # render is satisfied by a DEGENERATE render — an empty measurement set produces an
    # empty table, and an empty table matches an empty table. Two citation pins on a
    # sibling branch had relocated onto a blank line and a bare `}` while the pin gate
    # reported no movement; this is the same shape, so it is closed rather than trusted to
    # the pinned scoreboard map alone.
    case("a DEGENERATE render — no adversaries measured — does not match the committed "
         "block, so an emptied scoreboard cannot pass by comparing equal to itself",
         scoreboard_block({}) == _have_block, False, drives_main=False)
    case("...and the committed block carries one row per adversary, counted, not assumed",
         sum(1 for ln in _have_block.splitlines()
             if ln.startswith("| ") and not ln.startswith("| adversary")),
         len(_MEASURED), drives_main=False)
    case("...and every measured label appears in it, so a row cannot be dropped while the "
         "row count is padded",
         sorted(lbl for lbl in _MEASURED if f"| {lbl} |" not in _have_block), [],
         drives_main=False)
    case("a whitespace-only block is not a render either",
         scoreboard_block({}).strip() == "", False, drives_main=False)
    # Assembled at run time: written whole, the bare token would be a real unmeasured
    # figure in a file this check reads, which is how the previous planted control was
    # caught by its own check.
    _bare_id_token = "03" + "/" + "04"
    case("a non-score EXPRESSION is stripped whole, not exempted as a bare token — it is "
         "excused inside the requirement-id list and nowhere else",
         (sorted(score_shaped_tokens("cites TH-03" + "/" + "04" + "/" + "05 only")),
          sorted(score_shaped_tokens(f"the adversary scored {_bare_id_token}"))),
         ([], [_bare_id_token]), drives_main=False)
    # Assembled at run time: written whole, the planted figure would be a real unmeasured
    # token in a file this check now reads. Same reason as the planted dead mechanism.
    _planted_score = "3" + "/4"
    case("...and that check would see a planted one",
         sorted(score_shaped_tokens(f"the adversary scored {_planted_score}")
                - _measured_tokens), [_planted_score], drives_main=False)
    # NICE-TO-HAVE 2 of round 22: the backstop could not see the two forms the miscount it
    # was built after was written in. Both are assembled at run time, for the same reason
    # the planted score token is.
    _planted_pct = "52" + "%"
    _planted_of = "125" + " of " + "236"
    case("the prose-figure scan sees a PERCENTAGE and an `N of M`, the two forms the "
         "score-token backstop was blind to",
         sorted(prose_figures(f"we measured {_planted_pct} and {_planted_of} cases")),
         sorted([_planted_of, _planted_pct]), drives_main=False)
    case("...and no UNDECLARED prose figure survives in any file that cites this gate",
         {rel: sorted(prose_figures((ROOT / rel).read_text()))
          for rel in _scan if prose_figures((ROOT / rel).read_text())}, {},
         drives_main=False)
    # The list is pinned HERE as well, so that adding to it is a two-line edit in one
    # commit rather than one word — the same shape as the roster in
    # scripts/requirements.py, and for the same reason: a declaration list nobody has to
    # re-declare is a place to hide a figure.
    case("the pinned prose figures are the two quoted retractions and the four gate counts",
         sorted(PINNED_PROSE_FIGURES),
         ["0 of 21", "1 of 23", "100" + "%", "47 of 47", "51 of 82", "85" + "%"],
         drives_main=False)
    case("the scan is NARROW and says so: an English spelling of a measurement passes",
         sorted(prose_figures("roughly half of the cases, most of them")), [],
         drives_main=False)
    case("the non-score EXPRESSIONS are exactly the requirement-id lists",
         sorted(NON_SCORE_EXPRESSIONS),
         ["N7-13" + "/" + "15" + "/" + "17", "TH-03" + "/" + "04" + "/" + "05"],
         drives_main=False)
    case("and the residue no longer claims that a wrong implementation fails in general",
         "a WRONG implementation" in GI11_HUMAN_REVIEW_RESIDUE[0], False,
         drives_main=False)

    print("\n  the LIVENESS DIFFERENTIAL — the escape from the fifth rung")
    _fails, _total = liveness_differential()
    case("the corpus is non-trivial", _total >= 12, True, drives_main=False)
    case("it is RED with the model actually wired — every other precondition was not",
         len(_fails) > 0, True, drives_main=False)
    case("it fails exactly the shapes that broke the three lexical designs, and their "
         "metamorphic variants",
         sorted(r for r, _w, _g in _fails),
         ["dead-caller", "diverging-if", "false-branch", "mm-diverging-if-renamed",
          "mm-false-branch-reordered", "mm-while-true-reordered", "while-true"],
         drives_main=False)
    # A REAL relation: for each variant that names a base, the two must be scored the same
    # way. The predecessor of this case was `X or True` — always true, asserting nothing,
    # written in the round that added the mechanism it was meant to prove. Ninth sighting.
    _failed = {r for r, _w, _g in _fails}
    # Actual VERDICTS, not membership in the failure set: a base answering `live` and its
    # variant answering `refused` are both "failing" and would have compared equal.
    _answers = {}
    for _l in LIVENESS_CORPUS.read_text().splitlines():
        if not _l.strip() or _l.startswith("#"):
            continue
        _f = _l.split("\t")
        _answers[_f[0]] = liveness_oracle(strip_literals(_f[3].replace("\\n", "\n")), _f[2])
    case("every metamorphic variant gets the same VERDICT as its base — not merely the "
         "same pass/fail",
         sorted(b for b, v in VARIANT_OF_BASE.items() if _answers[b] != _answers[v]),
         [], drives_main=False)
    _diff_rows = [l.split("\t") for l in LIVENESS_CORPUS.read_text().splitlines()
                  if l.strip() and not l.startswith("#")]

    def _score(oracle):
        return sum(1 for rid, ans, subj, src, _w, _p in _diff_rows if oracle(src, subj) != ans)

    case("answering `live` everywhere fails it", _score(lambda s, x: "live") > 0, True,
         drives_main=False)
    case("answering `dead` everywhere fails it", _score(lambda s, x: "dead") > 0, True,
         drives_main=False)
    case("a RENAMED WRAPPER round the lexical probe fails it — the fifth rung closed",
         _score(lambda s, x: liveness_oracle(strip_literals(s.replace("\\n", "\n")), x)) > 0,
         True, drives_main=False)
    case("the reviewed answers pass it, so it is satisfiable",
         _score(lambda s, x: {r[0]: r[1] for r in _diff_rows}[
             next(r[0] for r in _diff_rows if r[2] == x and r[3] == s)]), 0, drives_main=False)
    # INJECTED, not grepped. This case used to search for an explanatory phrase, so it
    # would not have noticed the guard being deleted. Tenth sighting of that shape.
    # POINTED AT A TEMPORARY FILE, NEVER WRITTEN OVER THE TRACKED ONE. This control used to
    # blank tests/liveness-differential.tsv and restore it in a `finally`: a concurrent gate
    # could read the empty corpus, and a SIGKILL between the two writes left the
    # repository's corpus destroyed. The check is the same; the blast radius is not.
    _empty_corpus = Path(tempfile.mkdtemp(prefix="thesis-empty-corpus-")) / LIVENESS_CORPUS.name
    _empty_corpus.write_text("# every row removed\n")
    _real_corpus = LIVENESS_CORPUS
    globals()["LIVENESS_CORPUS"] = _empty_corpus
    try:
        liveness_differential()
        _empty = "accepted"
    except HarnessError:
        _empty = "rejected"
    finally:
        globals()["LIVENESS_CORPUS"] = _real_corpus
    case("an emptied corpus is REJECTED — injected on a TEMPORARY path, so the control "
         "cannot destroy the tracked corpus it is testing",
         _empty, "rejected", drives_main=False)
    case("...and the tracked corpus is intact afterwards, with its pinned row count",
         (LIVENESS_CORPUS == _real_corpus, liveness_differential()[1]),
         (True, len(EXPECTED_LIVENESS_IDS)), drives_main=False)

    print("\n  incomplete_definition — THE LAST UNAUDITED DECISION FUNCTION")
    # Round 21. The function between the corpora and the exit code: it decides whether this
    # command may compute a verdict at all. Two defects and one coverage gap, each
    # separated from the code by construction.

    # F1. THE PRECONDITION SET WAS THE ONE DEFINITIONAL SET THAT WAS NOT PINNED.
    _saved_pre = PRECONDITIONS
    try:
        globals()["PRECONDITIONS"] = tuple(p for p in _saved_pre if p[0] != "GI-12")
        # THROUGH THE DECISION FUNCTION, not through the validator. The first version of
        # this case called `validate_preconditions()` directly — so deleting the CALL SITE
        # in `incomplete_definition` left it green, which is a check that exists and is not
        # on the path. Same class this branch keeps finding; caught here by reverting.
        _dropped = _raises_harness(incomplete_definition)
        globals()["PRECONDITIONS"] = _saved_pre
        _no_check = sorted({r for r, _w in incomplete_definition()})
    finally:
        globals()["PRECONDITIONS"] = _saved_pre
    case("DELETING a precondition is a HARNESS ERROR now — it used to remove the refusal "
         "silently while the unsound constant stayed put",
         _dropped, True, drives_main=False)
    case("...and repointing one at a different constant is caught too, because retiring a "
         "safeguard by aiming it elsewhere reads as a rename in a diff",
         _raises_harness(lambda: (globals().__setitem__(
             "PRECONDITIONS",
             (("GI-11", "LIVENESS_MODEL", "lexical", "call-graph", "x"),
              ("GI-12", "LIVENESS_MODEL", "substring", "code", "x"))),
             validate_preconditions())[-1]), True, drives_main=False)
    globals()["PRECONDITIONS"] = _saved_pre
    case("as committed, both safeguards are outstanding and BOTH refuse",
         _no_check, ["GI-11", "GI-12"], drives_main=False)

    # F3/MF2. WHAT "MET" MEANS. It used to mean two labels agree: the decision read
    # `globals()[const] != sound`, and `validate_preconditions()` compared `sound` against
    # a pin in the SAME FILE — pin against pin. Dual-edit both and GI-12 read as met while
    # the substring adjudicator was still deciding every reject row. The decision is
    # physical now, and these cases drive both directions.
    _saved_attr = ATTRIBUTION_MODEL
    try:
        globals()["ATTRIBUTION_MODEL"] = "code"
        _label_only = sorted({r for r, _w in incomplete_definition(_me_for_drive)})
    finally:
        globals()["ATTRIBUTION_MODEL"] = _saved_attr
    case("editing the LABEL does not lift GI-12 — `met` is not `two labels agree`, and a "
         "dual edit of the tuple and its pin was the whole attack",
         _label_only, ["GI-11", "GI-12"], drives_main=False)
    # The lift now requires BOTH routes closed — this gate's comparator AND the shell that
    # actually adjudicates — which is what round 23 added, so the in-file half alone no
    # longer lifts it and this case says so by asserting GI-12 is STILL there.
    case("closing only THIS file's comparator does not lift GI-12 while `conformance.sh` "
         "still matches by fixed string — one route closed is not the mechanism retired",
         sorted({r for r, _w in incomplete_definition(mutate(
             _me_for_drive, _fp_anchor,
             "if want_fp.casefold() not in decl.casefold():"))}),
         ["GI-11", "GI-12"], drives_main=False)
    case("...and a NO-OP OPERAND SWAP does not lift it: `a != b` is `b != a`, and matching "
         "one order read a semantically identical refactor as the mechanism being retired",
         sorted({r for r, _w in incomplete_definition(mutate(
             _me_for_drive, _fp_anchor, "if decl.strip() != want_fp.strip():"))}),
         ["GI-11", "GI-12"], drives_main=False)
    case("GI-11 refuses on THREE grounds now — the model in use, and each corpus — so a "
         "corpus that passes cannot declare the model replaced",
         len([r for r, _w in incomplete_definition(_me_for_drive) if r == "GI-11"]), 3,
         drives_main=False)

    # F2. `_validate_contract` names the defect "a reject row with no required fingerprint";
    # a whitespace one reached that state without tripping it.
    case("a reject row pinned with WHITESPACE is refused, like `-` and `` — it made the "
         "fingerprint comparison vacuous against a corpus that declares nothing",
         [_raises_harness(lambda fp=fp: _validate_contract({"X-01": ("reject", "x.pd", fp)}))
          for fp in ("-", "", " ", "   ", "\t")], [True] * 5, drives_main=False)
    case("...while a real fingerprint still validates",
         _raises_harness(lambda: _validate_contract(
             {"X-01": ("reject", "x.pd", "declared pure")})), False, drives_main=False)

    print("\n  THE CATCHING TOOLS THEMSELVES — round 23")
    # Twenty-two rounds of finding self-satisfying checks, and the last instances were in
    # the instruments. Each of these is the measured attack, run.

    # MF1. Deleting a key retired that row's pin in silence.
    # THROUGH THE PATH, not through the validator: this is the second time a control of
    # mine tested the checker while the CALL SITE was what could be deleted.
    _saved_pins = dict(PINNED_ACCEPTANCE_SHA)
    try:
        PINNED_ACCEPTANCE_SHA.pop("GI-11")
        _key_dropped = _why(_drive(gate_source=_me_for_drive))
    finally:
        PINNED_ACCEPTANCE_SHA.clear()
        PINNED_ACCEPTANCE_SHA.update(_saved_pins)
    case("REMOVING an acceptance-pin key is a HARNESS ERROR ON THE RELEASE PATH — `.get()` "
         "plus `if want_sha and …` meant the identical weakening raised nothing once the "
         "key was gone",
         _key_dropped.startswith("2 HARNESS=the acceptance-pin key set changed"), True)
    case("...and the reviewed key set is exactly the rows whose TEXT is the contract",
         sorted(PINNED_ACCEPTANCE_IDS), ["GI-11", "GI-12"], drives_main=False)

    # MF2. A blank pin disabled the coverage check while the receipt kept claiming it.
    case("a NON-SHA case pin is refused — blanking it disabled the inventory check while "
         "the summary went on printing `the set is pinned`, byte-identically",
         [case_pin_is_real(v) for v in ("", None, "off", "x" * 64, EXPECTED_CASE_SHA)],
         [False, False, False, False, True], drives_main=False)
    case("...and re-pinning is a COMMAND, not a constant that silently means off",
         "--print-case-digest" in _me_for_drive, True, drives_main=False)

    # MF3. A false universally-quantified absence in the normative annex, twice.
    case("the normative annex is under the claim scanner now — it was the one authority "
         "outside every mechanism here",
         "docs/specification/language-spec.md" in CLAIM_SCANNED, True, drives_main=False)
    case("`over N fixtures` is MEASURED from the manifest, so a corpus that grows leaves a "
         "red gate rather than a stale authority",
         stale_corpus_figures(), [], drives_main=False)
    # PLANTED, and BOTH BRANCHES EXERCISED. The predecessor of this case planted nothing:
    # its two assertions were a duplicate of the case above and an inline `re.search` that
    # bypassed the function entirely, so gutting the function to `return []` left the
    # self-test green.
    # ASSEMBLED AT RUN TIME. Written whole, each plant would be a LIVE `over N fixtures`
    # in a file this very check scans — the fifth time this session that a control's own
    # text tripped the check it was written for, and every time the guard caught it.
    _stale_fig, _live_fig = "over " + "53 fixtures", "over " + "70 fixtures"
    case("a planted STALE figure is caught",
         corpus_figures_in("planted.md", f"measured {_stale_fig}", 70),
         [f"planted.md: `{_stale_fig}`, measured 70"], drives_main=False)
    case("...a planted CURRENT figure is not",
         corpus_figures_in("planted.md", f"measured {_live_fig}", 70), [],
         drives_main=False)
    case("...and a QUOTED stale figure is read as history, which is what lets F7 record "
         "what an authority used to say",
         corpus_figures_in("planted.md", f'A11 said "{_stale_fig}" once', 70), [],
         drives_main=False)
    case("...while the same figure unquoted is a live claim and is caught",
         len(corpus_figures_in("planted.md", f"A11 says {_stale_fig} today", 70)), 1,
         drives_main=False)

    # MF4. Rename + label defeated the identifier-keyed detector with `grep -qF` live.
    _renamed = mutate(_me_for_drive, _fp_anchor,
                      "_l, _r = want_fp, decl\n        if _l.strip() != _r.strip():")
    case("the RENAME+LABEL attack no longer retires GI-12: attribution is anchored to the "
         "file that adjudicates, and `conformance.sh` still matches by fixed string",
         [s for s in substring_attribution_live(_renamed, CONFORMANCE_SH.read_text())
          if "conformance.sh" in s] != [], True, drives_main=False)
    case("...and with BOTH gone the refusal lifts, so the anchor is not a tautology",
         substring_attribution_live(_renamed, "no fixed-string matcher here"), [],
         drives_main=False)
    case("...and GI-12 stays outstanding on the real tree",
         "GI-12" in {r for r, _w in incomplete_definition(_renamed)}, True,
         drives_main=False)

    # MF5. Module-level I/O escaped the three-state boundary entirely.
    # THE CONTROL THAT MEASURES THE REQUIREMENT: run the real command against a tree whose
    # manifest is gone, and read the exit code. The AST walker below is a pre-check that
    # names spellings; this asks the question.
    _mf5_dir = Path(tempfile.mkdtemp(prefix="thesis-no-manifest-"))
    for _rel in ("scripts", "docs/contributing", "tests"):
        (_mf5_dir / _rel).mkdir(parents=True, exist_ok=True)
    for _rel in ("scripts/thesis_exit.py", "scripts/gate_probe.py",
                 "scripts/conformance.sh", "tests/liveness-differential.tsv",
                 "tests/callgraph-differential.tsv"):
        (_mf5_dir / _rel).write_text((ROOT / _rel).read_text())
    # docs/contributing/1.0-requirements.tsv is deliberately NOT copied.
    _mf5 = subprocess.run([sys.executable, str(_mf5_dir / "scripts/thesis_exit.py")],
                          capture_output=True, text=True, cwd=_mf5_dir)
    _mf5_out = _mf5.stdout + _mf5.stderr
    case("a MISSING manifest is exit 2 with exactly one THESIS_RESULT line — measured by "
         "RUNNING the command, because the requirement is about what happens before "
         "`_entry()` exists and no list of attribute names decides that",
         (_mf5.returncode, _mf5_out.count("THESIS_RESULT"),
          "Traceback" in _mf5_out and "THESIS_RESULT" not in _mf5_out),
         (2, 1, False), drives_main=False)
    case("...and it says NO_VERDICT, not that the thesis is false",
         "THESIS_RESULT 2 NO_VERDICT" in _mf5_out, True, drives_main=False)
    case("the AST pre-check is clean too, and is named for what it is — a spelling scan, "
         "not the durable half: `_EAGER = real_acceptance()` escapes it entirely",
         (module_level_file_reads(_me_for_drive),
          module_level_file_reads("X = real_acceptance()\n")), ([], []), drives_main=False)
    case("...it does catch the spelling it names",
         len(module_level_file_reads(
             "from pathlib import Path\nX = Path('a').read_text()\n")), 1,
         drives_main=False)

    # MF6. An absence check standing where an existence check is required fails open.
    _cg = mutate(_me_for_drive, '\nLIVENESS_MODEL = "lexical"',
                 '\nLIVENESS_MODEL = "call-graph"')
    for _o, _n in (("p_has_ref_param", "cg_has_ref_param"),
                   ("p_total_on_fn", "cg_total_on_fn"),
                   ("p_effect_is_transitive", "cg_effect_is_transitive")):
        _cg = re.sub(rf"(?<![A-Za-z_0-9]){_o}(?![A-Za-z_0-9])", _n, _cg)
    case("RENAMED lexical probes no longer clear the declaration: the old names being "
         "absent is not the graph being consumed, and an absence check where an existence "
         "check belongs fails OPEN",
         any("reaches `_ask_provider`" in p for p in wiring_matches_declaration(_cg)), True,
         drives_main=False)
    case("...and as committed nothing reaches the provider, which is why `call-graph` "
         "cannot be declared today",
         _dispatch_reaches(__import__("ast").parse(_me_for_drive), THESIS_DISPATCH_ROOTS,
                           "_ask_provider"), False, drives_main=False)

    print("\n  THE PROCESS BOUNDARY — concluded is not succeeded, and env is not inherited")
    # MF3. `classify(r, reject_codes=(1,))` exists because for a REJECT probe exit 1 is a
    # verdict. For `conformance.sh` exit 1 means the corpus run FAILED, and the reject
    # default was inherited: a failed run was `Concluded`, and its partial verdict lines
    # were parsed and scored.
    case("a conformance run that prints verdicts and EXITS 1 is refused — the reject "
         "default made a failed corpus run read as evidence",
         _why(_drive(real_conformance="#!/bin/sh\necho 'f/ref.pd PASS_VERIFIED'\nexit 1\n"))
         .startswith("2 HARNESS=scripts/conformance.sh did not conclude"), True)
    case("...and exit 0 with verdicts is still read",
         _why(_drive(real_conformance="#!/bin/sh\n" + "".join(
             f"echo '{ln}'\n" for ln in ALL_VERDICTS.splitlines()) + "exit 0\n",
             real_pdc="#!/bin/sh\necho \"Function 'header' has effects: [Io]\"\nexit 0\n",
             real_make=True)), "0 RED=")

    # MF4. `_probe` inherited the environment, so `CONFORMANCE_MANIFEST=… thesis-exit.sh`
    # made the delegate measure one corpus while `declared_fingerprint` read another — two
    # corpora, one verdict — and `CONFORMANCE_BLESS=1` made it rewrite goldens instead of
    # measuring them.
    _saved_env = {k: os.environ.get(k) for k in CORPUS_ENV_OVERRIDES}
    try:
        for k in CORPUS_ENV_OVERRIDES:
            os.environ[k] = "hostile"
        _seen = _probe(["sh", "-c", "echo B=${CONFORMANCE_BLESS} "
                        "F=[${CONFORMANCE_FORBID_OWNER}]"], ROOT).text.strip()
        _stated = _probe(["sh", "-c", "echo M=${CONFORMANCE_MANIFEST-unset}"], ROOT,
                         env_overrides={"CONFORMANCE_MANIFEST": "chosen"}).text.strip()
    finally:
        for k, v in _saved_env.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v
    case("the corpus-selecting environment is OVERRIDDEN for a delegated run, so an "
         "exported variable cannot make it rewrite goldens or filter the corpus — "
         "overridden and not unset, because the boundary merges into os.environ",
         _seen, "B=0 F=[]", drives_main=False)
    case("...and a caller that needs one states it, rather than inheriting it",
         _stated, "M=chosen", drives_main=False)

    # MF5. The block's own documented regeneration path did nothing: the flag was read
    # inside `self_test()` and the dispatch only entered `self_test()` for `--self-test`.
    _entry_src = __import__("ast").get_source_segment(
        _me_for_drive, next(n for n in __import__("ast").parse(_me_for_drive).body
                            if getattr(n, "name", "") == "_entry"))
    case("`--update-scoreboard` is read by the DISPATCH, not only inside the function it "
         "drives — the documented regeneration path was inert, which is a claim that the "
         "generated block is maintained",
         _entry_src.count("--update-scoreboard") >= 2, True, drives_main=False)
    case("...and it suppresses the verdict line, because regenerating a table is not a "
         "measurement of the thesis",
         "--update-scoreboard" in _entry_src.split("def finish")[1].split("return code")[0],
         True, drives_main=False)

    # MF6. A mandatory case recorded `skipped (pdc not built)` on a clean checkout.
    _mk = (ROOT / "Makefile").read_text()
    case("`test-thesis-runner` declares its `build` prerequisite, like every other target "
         "that runs the compiler — without it a clean checkout records a SKIP where a "
         "REJECTED is mandatory",
         "test-thesis-runner: build" in _mk, True, drives_main=False)
    case("`make gates` runs this gate's own self-test and the xfail gate — every defence "
         "here was reachable only from outside the umbrella",
         all(x in _mk.split("gates:")[1].split("\n")[0]
             for x in ("test-thesis-runner", "test-xfail", "check-retracted-claims")),
         True, drives_main=False)
    case("...and `make gates` does NOT run `thesis-exit`, which exits 2 by design: a green "
         "umbrella swallowing a NO_VERDICT is the reading this branch exists to prevent",
         "thesis-exit" in _mk.split("gates:")[1].split("\n")[0].replace(
             "test-thesis-runner", ""), False, drives_main=False)

    print("\n  THE SELF-TEST HARNESS — the region every other control runs through")
    # Round 20. `_drive`, `case`, `mutate`, `mutate_fp`, `_verdict`. A defect here does not
    # make a case fail; it makes a case MEAN LESS, which is invisible in a green run.
    case("the failure signature DISCRIMINATES: the same exit code with a DIFFERENT red row "
         "is a different signature, which `== 1` could not distinguish",
         (_why(_drive(verdicts=_verdict("N9-01", "OUTPUT_MISMATCH"))),
          _why(_drive(verdicts=_verdict("N8-01", "OUTPUT_MISMATCH")))),
         ("1 RED=N9-01", "1 RED=N8-01"))
    case("...and a harness error names its reason, so one malfunction cannot discharge a "
         "case written for another",
         _why(_drive(witness_b=GOOD_WITNESS + "fn m(s: S) { s.len(); }\n")).startswith(
             "2 HARNESS=TH-03 / `ref` parameter: this gate cannot"), True)
    case("the signature does not carry the synthetic repository's path, which changes "
         "every run and would pin the temp directory instead of the reason",
         "/var/folders" in _why(_drive(unreadable_requirements=True)), False)

    # `Context` claimed to be "every input the gate reads" and did not cover the gate's own
    # source, so the drift branch — the one that decides whether a verdict may be computed
    # at all — could not be driven by any injected state.
    case("the WIRING SOURCE is an injected input now: a source whose declaration and "
         "dispatch disagree drives the gate to exit 2, naming the drift",
         _why(_drive(gate_source=mutate(
             _me_for_drive, '\nLIVENESS_MODEL = "lexical"',
             '\nLIVENESS_MODEL = "call-graph"'))).startswith(
                 "2 HARNESS=the gate's declared models do not match"), True)
    case("...and the real run still reads the real file, because None means the file",
         Context().gate_source, None, drives_main=False)
    case("the drives_main split is MEASURED, not declared — `_drive` counts its own "
         "invocations and `case()` compares",
         isinstance(_drive.calls, int) and _drive.calls > 0, True, drives_main=False)

    print("\n  THE LEXICAL PROBE FAMILY — its own prose, audited against its own code")
    # Round 19. Every claim below was separated from the code by CONSTRUCTION, and the
    # construction is the case: a claim nobody can separate is either sound or untestable,
    # and each is labelled with which.
    _rp = ("fn main() { }\n"
           "fn dead_caller() -> i64 { return helper(); }\n"
           "fn helper(x: ref String) -> i64 { return 1; }\n")
    case("WIDER, and now corrected: a `ref` param on a function unreachable from `main` "
         "is GREEN — the probe decides `some other body mentions it`, never reachability",
         p_has_ref_param(strip_literals(_rp))[0], True, drives_main=False)
    case("...and both spellings of that retracted property are banned, so the docstring "
         "cannot say `reachable` again",
         # Assembled at run time: written whole, each would be a live occurrence of the
         # phrase it bans, in a file the banned-list check reads. Fourth sighting of that
         # shape this session, and the guard caught it every time.
         (len(stale_claims("a ref PARAMETER on a function reachable" + " from `main`")),
          len(stale_claims("it is unreachable, is not defined" + " here, or calls nothing"))),
         (1, 1), drives_main=False)

    case("WIDER, and now corrected: `callees` returns identifiers followed by `(`, not "
         "names called — `return (1 + 2);` yields `return`",
         (sorted(callees("return (1 + 2);")), sorted(callees("if (x) { }"))),
         (["return"], ["if"]), drives_main=False)
    case("...and the over-approximation is benign in its ONE consumer: a spurious name "
         "has no body, so it cannot exhibit an IO edge",
         p_effect_is_transitive("Function 'h' has effects: [Io]\n", strip_literals(
             "fn main() { h(); }\nfn h() { return (1); }\n"))[0], False,
         drives_main=False)

    # THE PREMISE P2 NEVER STATED: the unit is the whole program.
    case("a unit with NO entry root refuses the dead-code refutation instead of guessing "
         "— an exported function of a library fragment is not dead because this file does "
         "not name it",
         _raises_harness(lambda: provably_dead(
             function_bodies(strip_literals("pub fn exported(x: i64) -> i64 { return 1; }")),
             "exported")), True, drives_main=False)
    case("...and a whole program still answers",
         provably_dead(function_bodies(strip_literals(
             "fn main() { }\nfn lonely() -> i64 { return 1; }\n")), "lonely"), True,
         drives_main=False)
    case("...and `main` itself is never dead",
         provably_dead(function_bodies(strip_literals("fn main() { }\n")), "main"), False,
         drives_main=False)

    # SOUND, AND SAID SO PLAINLY — no counterexample exists because the construction
    # forbids one, which is a proof rather than a failed search.
    case("SOUND: the callee half of the effect edge is vacuous by construction — a callee "
         "is FOUND by being named in the caller's body, so `provably_dead` is False for it",
         provably_dead({"caller": "callee();", "callee": "return 1;", "main": ""},
                       "callee"), False, drives_main=False)

    # THE CONTROL THAT FLIPPED WITH N2-08. It was written to pin NON-nesting so that the
    # gate and the compiler would move together the day the compiler nested. They did:
    # these are the other side of the same case, and every one of them is RED against a
    # `strip_literals` that stops at the first close.
    case("SOUND: block comments NEST, matching the compiler — the inner close ends the "
         "INNER comment, so the whole construct is comment and nothing inside survives",
         strip_literals("/* outer /* inner */ async fn g() { } */").strip(),
         "", drives_main=False)
    case("...so an `async` between an inner close and the outer one is NOT found, which "
         "is how the gate and the compiler stay in lockstep now that N2-08 has landed",
         p_no_async_token(strip_literals("/* a /* b */ async fn g() { } */"))[0], True,
         drives_main=False)
    case("...and a real `async` AFTER the outer close is still FOUND, so nesting did not "
         "turn the probe into one that can never fire (F12)",
         p_no_async_token(strip_literals("/* a /* b */ */ async fn g() { }"))[0], False,
         drives_main=False)

    print("\n  the machine contract Make cannot carry")
    _rc = subprocess.run([sys.executable, str(ROOT / "scripts/thesis_exit.py")],
                         capture_output=True, text=True, cwd=ROOT)
    case("the script emits a typed result line",
         _rc.stdout.strip().splitlines()[-1].startswith("THESIS_RESULT "), True,
         drives_main=False)
    case("the line names the same code the script exits with",
         _rc.stdout.strip().splitlines()[-1], f"THESIS_RESULT {_rc.returncode} "
         f"{RESULT_NAMES[_rc.returncode]}", drives_main=False)

    print("\n  the acceptance digest fires on a real edit")
    _bad = mutate(real_acceptance()["GI-11"],
                  "a HARNESS FAILURE distinct from omission", "ignored")
    case("weakening GI-11's acceptance text changes its digest",
         acceptance_digest(_bad) != PINNED_ACCEPTANCE_SHA["GI-11"], True, drives_main=False)
    case("the pinned digest is a FULL sha256, not a truncation",
         len(PINNED_ACCEPTANCE_SHA["GI-11"]), 64, drives_main=False)

    print("\n  a precondition cannot be satisfied by naming an artifact")
    case("both preconditions are outstanding right now",
         sorted(set(r for r, _w in incomplete_definition())), ["GI-11", "GI-12"],
         drives_main=False)
    case("GI-11 is outstanding for BOTH reasons — the structural AND the liveness corpus",
         len([r for r, _w in incomplete_definition() if r == "GI-11"]), 3,
         drives_main=False)
    # Through mutate(), so that when these constants legitimately change the controls
    # fail loudly instead of quietly mutating nothing. Eighth sighting of that class, and
    # it was sitting on the two mutations that matter most.
    _me = (ROOT / "scripts/thesis_exit.py").read_text()

    # Anchored at column zero so the mutation hits the DECLARATION and not the copies of
    # it inside prose. mutate() rejected the ambiguous form, which is why it exists.
    case("declaring `call-graph` while the lexical probes are still wired is caught",
         bool(wiring_matches_declaration(mutate(
             _me, '\nLIVENESS_MODEL = "lexical"', '\nLIVENESS_MODEL = "call-graph"'))),
         True, drives_main=False)
    case("declaring `code` while the substring comparison is still wired is caught",
         bool(wiring_matches_declaration(mutate(
             _me, '\nATTRIBUTION_MODEL = "substring"', '\nATTRIBUTION_MODEL = "code"'))),
         True, drives_main=False)
    case("the declaration and the wiring agree as committed",
         wiring_matches_declaration((ROOT / "scripts/thesis_exit.py").read_text()), [],
         drives_main=False)

    # THE CHECK THAT GATES EVERYTHING ELSE USED TO SATISFY ITSELF (MUST-FIX 1). Both halves
    # were answerable by text this file contains for other reasons, so both are now read as
    # STRUCTURE, and both reverts are driven here rather than described.
    # BOTH ANCHORS ARE ASSEMBLED AT RUN TIME. Written whole, each would occur twice in
    # this file — once in the code being mutated and once here — and `mutate()` refuses an
    # ambiguous anchor. That refusal is the same guard, one level down, and it fired.
    case("DELETING the real TH-05 dispatch makes the check fail — it was satisfied by the "
         "probe name appearing in a tuple, a comment or its own `def`",
         bool(wiring_matches_declaration(mutate(
             _me, _disp_anchor, 'return (False, "dispatch removed")'))),
         True, drives_main=False)
    case("...and the probe names still occur in the source it just rejected, which is "
         "exactly why counting occurrences was not a check",
         mutate(_me, _disp_anchor,
                'return (False, "dispatch removed")').count(
                    "p_effect_is_" + "transitive") > 1,
         True, drives_main=False)
    case("REPLACING the fingerprint comparison with a STRING of itself makes the check "
         "fail — the defect was that the asking line contained the answer",
         bool(wiring_matches_declaration(mutate(
             _me, _fp_anchor, 'if ' + '"want_fp' + '.strip() != decl.strip()" and False:'))),
         True, drives_main=False)
    case("a source whose ONLY occurrence of the comparison is a string literal reads as "
         "NOT wired — a literal is a Constant, not a Compare",
         _has_fingerprint_comparison(__import__("ast").parse(
             'x = "want_fp.strip() != decl.strip()"\n')), False, drives_main=False)
    case("...while the real expression reads as wired",
         _has_fingerprint_comparison(__import__("ast").parse(
             _fp_anchor + "\n    pass\n")), True, drives_main=False)
    case("a reference from the SELF-TEST is not wiring — measured: one audit case calling "
         "a probe directly kept it `wired` with its dispatch deleted",
         ("p_effect_is_transitive" in _load_references(__import__("ast").parse(
             "def self_test():\n    p_effect_is_transitive(a, b)\n"),
             skip=WIRING_SCOPE_SKIP),
          "p_effect_is_transitive" in _load_references(__import__("ast").parse(
              "def evaluate():\n    p_effect_is_transitive(a, b)\n"),
              skip=WIRING_SCOPE_SKIP)),
         (False, True), drives_main=False)
    case("a name mentioned only as a STRING is not a load reference",
         "p_has_ref_param" in _load_references(__import__("ast").parse(
             'PROBES = ("p_has_ref_param",)\n')), False, drives_main=False)
    case("...and a name used as a value is",
         "p_has_ref_param" in _load_references(__import__("ast").parse(
             'D = {"TH-03": p_has_ref_param}\n')), True, drives_main=False)
    case("unparseable source is a failure to MEASURE, not a wiring finding",
         _raises_harness(lambda: wiring_matches_declaration("def (:\n")), True,
         drives_main=False)

    print("\n  the safeguards for this gate's weaknesses BLOCK green (MUST-FIX 5)")
    _req = (ROOT / "docs/contributing/1.0-requirements.tsv").read_text().split("\n")
    _disp = {r.split("\t")[0]: r.split("\t")[7] for r in _req
             if len(r.split("\t")) == 9 and not r.startswith("#")}
    case("GI-11 is a THESIS row in the real manifest, not an ordinary 1.0 row",
         _disp.get("GI-11"), "thesis", drives_main=False)
    case("GI-12 is a THESIS row in the real manifest, not an ordinary 1.0 row",
         _disp.get("GI-12"), "thesis", drives_main=False)
    case("both are in the pinned contract, so demoting one is a harness error",
         sorted(k for k in EXPECTED_THESIS_CONTRACT if k.startswith("GI-")),
         ["GI-11", "GI-12"], drives_main=False)
    case("GI-12 is NOT a reject row — it cannot be judged by the matcher it replaces",
         _C["GI-12"][0] != "reject", True, drives_main=False)

    print("\n  R4 — the detected set, and the name collision reviewers found")
    case("two functions sharing a name are a HARNESS ERROR, not a silent overwrite",
         _why(_drive(witness_b=GOOD_WITNESS + "fn header(mut c: C) { emit(c, \"y\"); }\n")), '2 HARNESS=TH-03 / `ref` parameter: this gate cannot se')

    print("\n  TH-05 — the effect must be TRANSITIVE, and the edge must be exhibited")
    case("a DIRECT-only effect goes RED", _why(_drive(report=GOOD_REPORT.splitlines()[0])), '1 RED=TH-05')
    case("a reported function that CALLS NOTHING goes RED",
         _why(_drive(witness_b=GOOD_WITNESS + "fn ghost() { }\n",
                report_b="Function 'ghost' has effects: [Io]\n")), '1 RED=TH-05')
    case("a report naming a function not defined here goes RED",
         _why(_drive(report="Function 'nowhere' has effects: [Io]\n")), '1 RED=TH-05')
    case("no IO reported at all goes RED",
         _why(_drive(report="Function 'pure' has effects: [Memory]\n")), '1 RED=TH-05')

    print("\n  the REAL subprocess boundary — no injection, so gate_probe decides")
    case("conformance, pdc and make all RUN and conclude successfully -> exit 0",
         _drive(real_conformance="#!/bin/sh\n" + "".join(
                    f"echo '{ln}'\n" for ln in ALL_VERDICTS.splitlines()) + "exit 0\n",
                real_pdc="#!/bin/sh\necho \"Function 'header' has effects: [Io]\"\nexit 0\n",
                real_make=True), 0)
    case("conformance that prints verdicts and then FAILS is exit 2, not a verdict",
         _why(_drive(real_conformance="#!/bin/sh\necho 'f/ref.pd PASS_VERIFIED'\nexit 3\n")), '2 HARNESS=scripts/conformance.sh did not conclude (exi')
    case("conformance that prints verdicts and is then KILLED is exit 2",
         _why(_drive(real_conformance="#!/bin/sh\necho 'f/ref.pd PASS_VERIFIED'\nkill -9 $$\n")), '2 HARNESS=scripts/conformance.sh did not conclude (kil')
    case("a pdc that prints an effect line and then FAILS is exit 2",
         _why(_drive(real_pdc="#!/bin/sh\necho \"Function 'header' has effects: [Io]\"\nexit 1\n")), '2 HARNESS=pdc rejected a.pd (exit 1); an effect report')
    case("a pdc that prints an effect line and is then KILLED is exit 2",
         _why(_drive(real_pdc="#!/bin/sh\necho \"Function 'header' has effects: [Io]\"\nkill -9 $$\n")), '2 HARNESS=pdc did not conclude while compiling a.pd (k')

    print("\n  a failure to MEASURE never wears the exit code of a finding")
    with tempfile.TemporaryDirectory() as d:
        broken = Path(d)
        (broken / "scripts").mkdir()
        (broken / "scripts/gate_probe.py").write_text("this is not python(((\n")
        saved, globals()["ROOT"] = ROOT, broken
        try:
            _load_gate_probe()
            got = "no exception"
        except HarnessError:
            got = "HarnessError"
        except Exception as exc:            # noqa: BLE001
            got = type(exc).__name__
        finally:
            globals()["ROOT"] = saved
    case("a broken gate_probe.py raises HarnessError, not a bare SyntaxError",
         got, "HarnessError", drives_main=False)
    rc = subprocess.run(
        [sys.executable, str(ROOT / "scripts/thesis_exit.py"), "--crash-for-self-test"],
        capture_output=True, text=True, cwd=ROOT).returncode
    case("an arbitrary crash in the real entry point exits 2, never 1",
         rc, 2, drives_main=False)
    rc_se = subprocess.run(
        [sys.executable, str(ROOT / "scripts/thesis_exit.py"), "--systemexit-for-self-test"],
        capture_output=True, text=True, cwd=ROOT).returncode
    case("a dependency's SystemExit(1) exits 2, not 1 — it is not a thesis verdict",
         rc_se, 2, drives_main=False)
    rc_sh = subprocess.run(
        ["bash", str(ROOT / "scripts/thesis-exit.sh"), "--crash-for-self-test"],
        capture_output=True, text=True, cwd=ROOT).returncode
    case("the shell wrapper preserves the code (exec, no re-mapping)",
         rc_sh, 2, drives_main=False)
    case("an unreadable requirements file is exit 2",
         _why(_drive(rows=None, unreadable_requirements=True)), '2 HARNESS=cannot read <tmp> [Errno 21] Is a directory:')
    case("an unreadable Makefile is exit 2, not a red `make` row",
         _why(_drive(make=None, unreadable_makefile=True)), '2 HARNESS=cannot read <tmp> [Errno 2] No such file or ')

    print("\n  a mutation that mutates nothing is caught, not silently passed")

    def _mut(old, new, expect=1):
        try:
            mutate(GOOD_WITNESS, old, new, expect)
            return "accepted"
        except HarnessError:
            return "rejected"

    case("a real mutation is accepted", _mut("return depth(1);", "return 1;"), "accepted",
         drives_main=False)
    case("a mutation matching NOTHING is rejected — the four-time disease, mechanised",
         _mut("this text is not in the witness", "x"), "rejected", drives_main=False)
    case("a mutation matching more often than declared is rejected",
         _mut("c", "d"), "rejected", drives_main=False)
    def _vd(row_id, verdict):
        try:
            _verdict(row_id, verdict)
            return "accepted"
        except HarnessError:
            return "rejected"

    case("_verdict on a row with a real verdict line is accepted",
         _vd("N9-01", "OUTPUT_MISMATCH"), "accepted", drives_main=False)
    case("_verdict on a GATE row (no verdict line) is rejected, not silently a no-op",
         _vd("SH-01", "WHATEVER"), "rejected", drives_main=False)
    def _fp(row, wrong):
        try:
            mutate_fp(row, wrong)
            return "accepted"
        except HarnessError:
            return "rejected"

    case("mutate_fp on a real reject row with a different value is accepted",
         _fp("N9-06", "something else entirely"), "accepted", drives_main=False)
    case("mutate_fp with the CORRECT fingerprint is rejected — it would mutate nothing",
         _fp("N9-06", _C["N9-06"][2]), "rejected", drives_main=False)
    case("mutate_fp on a non-reject row is rejected",
         _fp("N9-01", "x"), "rejected", drives_main=False)
    case("a mutation whose replacement equals the original is rejected",
         _mut("return depth(1);", "return depth(1);"), "rejected", drives_main=False)

    print("\n  CROSS-LAYER: what `REJECTED` does and does not prove (GI-12)")
    # Not an argument — a measurement, driven through the REAL scripts/conformance.sh.
    # A fixture that fails for an entirely incidental reason, whose log happens to carry
    # the pinned phrase because the compiler echoes the source line, is reported REJECTED
    # and counted as coverage. The equality tightening proves the row and the corpus
    # AGREE; it cannot prove the matching text came from the intended diagnostic, because
    # `grep -qF` searches the whole ANSI-stripped log.
    incidental_verdict = "skipped (pdc not built)"
    if (ROOT / "target/release/pdc").is_file():
        with tempfile.TemporaryDirectory(dir=ROOT / "build_output") as d:
            probe = Path(d)
            phrase = _C["N7-01"][2]
            (probe / "incidental.pd").write_text(
                'fn main() {\n    let msg = "' + phrase + '";\n    @@@\n}\n')
            man = probe / "manifest.txt"
            rel = probe.relative_to(ROOT)
            man.write_text(f"{rel}/incidental.pd\treject\tcompile\t{phrase}\t-\tprobe\n")
            env_probe = dict(os.environ, CONFORMANCE_MANIFEST=str(man.relative_to(ROOT)))
            GPmod = GP
            res = GPmod.classify(GPmod.run(
                ["bash", str(ROOT / "scripts/conformance.sh"), str(rel)],
                cwd=str(ROOT), env={"CONFORMANCE_MANIFEST": str(man.relative_to(ROOT))}))
            text = getattr(res, "text", "")
            incidental_verdict = "REJECTED" if "REJECTED" in text else "not REJECTED"
    case("an INCIDENTAL diagnostic satisfies a pinned fingerprint — measured, not argued",
         incidental_verdict, "REJECTED", drives_main=False)

    print("\n  the PIN itself is checked, not only the manifest against it")

    def _pin_verdict(contract):
        try:
            _validate_contract(contract)
            return "accepted"
        except HarnessError:
            return "rejected"

    good = dict(EXPECTED_THESIS_CONTRACT)
    case("the real pinned contract is well formed", _pin_verdict(good), "accepted",
         drives_main=False)
    case("a pinned reject row with `-` for a fingerprint is REJECTED",
         _pin_verdict({**good, "N9-06": ("reject", good["N9-06"][1], "-")}), "rejected",
         drives_main=False)
    case("a pinned reject row with an empty fingerprint is REJECTED",
         _pin_verdict({**good, "N9-06": ("reject", good["N9-06"][1], "")}), "rejected",
         drives_main=False)
    case("a pinned fixture row carrying a fingerprint is REJECTED",
         _pin_verdict({**good, "N9-01": ("fixture", good["N9-01"][1], "something")}),
         "rejected", drives_main=False)
    case("a pinned row with an unknown kind is REJECTED",
         _pin_verdict({**good, "N9-01": ("vibes", good["N9-01"][1], "-")}), "rejected",
         drives_main=False)

    print("\n  the row set is CLOSED")
    case("ADDING a thesis row is a harness error (exit 2)",
         _why(_drive(rows=_rows(extra="ZZ-99\tM9\tsrc\tsneaked in\tfixture\tx.pd\towed\tthesis\t-\n"))), '2 HARNESS=the thesis row set changed, and that is a ch')
    case("REMOVING a thesis row is a harness error", _why(_drive(rows=_rows(drop="N9-06"))), '2 HARNESS=the thesis row set changed, and that is a ch')
    case("RETYPING a row out of dispatch is a harness error, not a silent skip",
         _why(_drive(rows=_rows(retype=("N8-08", "observable", "t.rs::x")))), '2 HARNESS=N8-08: the thesis contract changed, which ch')
    # The control above only exercises a kind that falls OUT of dispatch. The dangerous
    # retype is into another DISPATCHED kind: reject -> fixture turns a negative test
    # into a positive one and the row set still looks intact.
    case("retyping reject -> FIXTURE (still dispatched) is a harness error",
         _why(_drive(rows=_rows(retype=("N8-08", "fixture", _C["N8-08"][1])))), '2 HARNESS=N8-08: the thesis contract changed, which ch')
    case("repointing a row at a different fixture is a harness error",
         _why(_drive(rows=_rows(repoint=("N9-01", "tests/somewhere_else.pd")))), '2 HARNESS=N9-01: the thesis contract changed, which ch')
    case("BLANKING a thesis reject fingerprint to `-` is a harness error",
         _why(_drive(rows=_rows(blank_fp="N9-06"))), '2 HARNESS=N9-06: the thesis contract changed, which ch')
    case("an unknown evidence kind is a harness error",
         _why(_drive(rows=_rows(retype=("N8-08", "vibes", "x")))), "2 HARNESS=req.tsv:9: unknown evidence kind 'vibes'")
    case("a duplicate id is a harness error",
         _why(_drive(rows=_rows(extra="N9-06\tM7\tsrc\tdup\treject\tx.pd\towed\tthesis\t-\n"))), '2 HARNESS=req.tsv:28: duplicate id N9-06')
    case("a short row is a harness error",
         _why(_drive(rows=_rows(extra="BAD\tM9\tsrc\tonly six\tgate\tx\n"))), '2 HARNESS=req.tsv:28: 6 columns, want 9')

    print("\n  the lexer")
    case("a char literal '<' is not a lifetime",
         p_no_lifetime_param_list(strip_literals("fn f() { let x = '<'; }"))[0], True,
         drives_main=False)
    case("block comments NEST, matching src/lexer/token.rs `slash_or_comment` (flipped "
         "with N2-08; bootstrap/pdc.pd's own scanner still does not, and no PBS-1 source "
         "contains a nested comment for them to disagree about)",
         "async" in strip_literals("/* a /* b */ async fn f() {} */"), False,
         drives_main=False)
    case("an unterminated comment does not crash the lexer",
         isinstance(strip_literals("/* unterminated"), str), True, drives_main=False)

    print("\n  lexical stripping is part of the proof, so brace depth is controlled")
    for src, want, label in [
        ('fn a() { let s = "}"; } fn main() { a(); }', ["a", "main"], "brace in a string"),
        ("fn a() { let c = '}'; } fn main() { a(); }", ["a", "main"], "brace in a char"),
        ("fn a() { // }\n } fn main() { a(); }", ["a", "main"], "brace in a line comment"),
        ("fn a() { /* } */ } fn main() { a(); }", ["a", "main"], "brace in a block comment"),
    ]:
        case(f"function_bodies survives a {label}",
             sorted(function_bodies(strip_literals(src))), want, drives_main=False)
    case("a body whose braces are only inside literals is still bounded",
         "a" in function_bodies(strip_literals('fn a() { let s = "{{{"; }')), True,
         drives_main=False)

    print("\n  the typed process boundary (scripts/gate_probe.py)")
    killed = GP.classify(GP.run(["sh", "-c", "echo 'error: buffered' >&2; kill -9 $$"]))
    case("a killed producer yields Malfunction with no `.text`",
         hasattr(killed, "text"), False, drives_main=False)
    case("a concluded producer yields readable text",
         hasattr(GP.classify(GP.run(["sh", "-c", "echo fine"])), "text"), True,
         drives_main=False)

    print("\n  NO NEGATIVE CONTROL — an explicit disclosure, pinned verbatim")
    case("the uncovered disclosure is exactly what it was reviewed as",
         sorted(EXPECTED_UNCOVERED), sorted(_UNCOVERED_AS_REVIEWED), drives_main=False)
    for u in sorted(EXPECTED_UNCOVERED):
        print(f"  {GREY}--   {u}{OFF}")

    if "--update-scoreboard" in sys.argv:
        # The generated-file convention this repository already uses for citation pins:
        # never edit by hand, regenerate and read the diff.
        _road = ROOT / "docs/contributing/MILESTONES.md"
        _txt = _road.read_text()
        if SCOREBOARD_BEGIN not in _txt or SCOREBOARD_END not in _txt:
            raise HarnessError("the roadmap has no ADVERSARY-SCOREBOARD block to update")
        _road.write_text(_txt[:_txt.index(SCOREBOARD_BEGIN)] + scoreboard_block(_MEASURED)
                         + _txt[_txt.index(SCOREBOARD_END) + len(SCOREBOARD_END):])
        print(f"  {GREEN}wrote{OFF} the adversary scoreboard into "
              f"docs/contributing/MILESTONES.md — read the diff")

    print("=" * 78)
    if fails == 0:
        # UNIQUE cases. Ten labels were duplicated by an earlier block-insert, inflating
        # the count without adding a single new fault. A test count that grows by copying
        # is the same defect as a green gate that measures nothing.
        # CHECKED, not asserted: `case()` rejects a repeat, so `cases == len(seen_names)`
        # holds by construction, and the pinned digest below fails if the set changes at
        # all — which is the same closure the thesis rows and the liveness corpus have.
        digest = hashlib.sha256("\n".join(sorted(seen_names)).encode()).hexdigest()
        # NO `and`, AND NO BLANK ESCAPE HATCH. `if EXPECTED_CASE_SHA and …` meant blanking
        # one constant disabled the coverage pin while the summary line went on saying "the
        # set is pinned" — measured: blank it, rename a case, and the receipt is
        # BYTE-IDENTICAL to the true one at exit 0. A false claim that is PRINTED is worse
        # than one in a comment. Re-pinning has an explicit path now
        # (`--print-case-digest`), which is a command someone runs rather than a constant
        # that silently means "off".
        if os.environ.get("THESIS_PRINT_CASE_DIGEST") == "1":
            print(f"  case-inventory digest for a deliberate re-pin: {digest}")
            return 0
        if not case_pin_is_real(EXPECTED_CASE_SHA):
            print(f"  {RED}the case inventory is NOT PINNED{OFF}: EXPECTED_CASE_SHA is "
                  f"{EXPECTED_CASE_SHA!r}, which is not a sha256. The summary below claims "
                  f"the set is pinned, so this cannot be allowed to pass. This run's "
                  f"digest is {digest}; re-pin with `--print-case-digest`.")
            return 1
        if digest != EXPECTED_CASE_SHA:
            print(f"  {RED}the case inventory changed{OFF} (digest {digest}, pinned "
                  f"{EXPECTED_CASE_SHA}). Adding or removing a control changes what this "
                  f"self-test covers; re-pin EXPECTED_CASE_SHA deliberately.")
            return 1
        assert cases == len(seen_names)
        print(f"  self-test green — {cases} unique cases (checked: duplicates are a "
              f"harness error, and the set is pinned): {driven} drive main() end to end, "
              f"{cases - driven} exercise a helper directly")
        print(f"  {len(EXPECTED_UNCOVERED)} probe group(s) pinned as uncovered, listed above")
        print("=" * 78)
        return 0
    print(f"  self-test RED — {fails} of {cases} cases failed")
    print("=" * 78)
    return 1


RESULT_NAMES = {0: "THESIS_HOLDS", 1: "THESIS_FALSE", 2: "NO_VERDICT"}


def _emit_result(code: int) -> None:
    """The machine contract, because `make` cannot carry it.

    Make maps every nonzero recipe status to 2, so a status-only consumer cannot tell
    `thesis false` from `could not measure` from `the build broke` — and those are three
    different facts, two of which are about Palladium and one of which is about the
    machine. The script's own exit code distinguishes them; this line survives the Make
    layer as well, so a consumer can parse rather than infer.
    """
    try:
        # `.get(code, "HARNESS_ERROR")` implied a FOURTH outcome. There are three, and a
        # harness error is one of them: it becomes exit 2 / NO_VERDICT by construction, in
        # `_entry`, so the name was unreachable and told a reader the contract distinguishes
        # something it does not. An unexpected code is a bug in this file, and saying so is
        # honest where inventing a category is not.
        print(f"THESIS_RESULT {code} {RESULT_NAMES.get(code, 'UNSPECIFIED_CODE')}")
    except BaseException:  # noqa: BLE001
        pass


def _entry(argv: list[str]) -> int:
    """Compute the exit code. NEVER calls sys.exit, so nothing here can leak one.

    The previous entry point had two holes, both of which could land on 0 or 1 where the
    code means 2 — and 1 is reserved for "the thesis does not hold", a claim about the
    language:

      * `except SystemExit: raise` re-raised a dependency's `SystemExit(0)` or
        `SystemExit(1)` verbatim. A library calling sys.exit() on a bad argument would
        have been reported as a thesis verdict.
      * the error report was printed INSIDE the handler, so a failure while writing it
        (a closed stderr, a broken pipe) propagated out of the handler rather than into
        a sibling clause, taking the chosen code with it.

    So: decide the number in here, report best-effort, and let the caller do the exiting.
    """
    def report(kind: str, detail: str) -> None:
        # Best effort by construction. If stderr is gone there is nothing to say and
        # nothing said may change the verdict already decided above.
        try:
            print(f"{RED}{kind}{OFF}: {detail}", file=sys.stderr)
            print("This is a failure to MEASURE, not a verdict about the language.",
                  file=sys.stderr)
        except BaseException:  # noqa: BLE001
            pass

    def finish(code: int) -> int:
        if not ({"--self-test", "--check-retracted-claims", "--update-scoreboard"}
                & set(argv)):
            _emit_result(code)
        return code

    try:
        if "--print-case-digest" in argv:
            # The deliberate re-pin path, as a COMMAND. It used to be "set the constant to
            # an empty string", which is indistinguishable from "someone turned the pin off".
            os.environ["THESIS_PRINT_CASE_DIGEST"] = "1"
            return self_test()
        if "--check-retracted-claims" in argv:
            return check_retracted_claims()
        if "--crash-for-self-test" in argv:
            # Exercised by the self-test: any unwrapped failure must leave as exit 2, the
            # measurement code, never as Python's default 1.
            raise RuntimeError("deliberate crash, exercised by --self-test")
        if "--systemexit-for-self-test" in argv:
            # A dependency calling sys.exit(1) must not be mistaken for a thesis verdict.
            raise SystemExit(1)
        # `--update-scoreboard` IMPLIES the self-test, because that is where the
        # measurements it writes are produced. It was read inside `self_test()` while the
        # dispatch only entered `self_test()` for `--self-test`, so the command the
        # generated block documents as its regeneration path printed a verdict line and
        # wrote nothing. A documented path that does nothing is worse than none: it is a
        # claim that the block is maintained.
        if "--self-test" in argv or "--update-scoreboard" in argv:
            return finish(self_test())
        return finish(main())
    except HarnessError as e:
        report("harness error", str(e))
        return finish(2)
    except SystemExit as e:
        # NOT re-raised. A SystemExit from anywhere below is a dependency's opinion about
        # the process, not this gate's verdict about Palladium.
        report("harness error", f"a dependency raised SystemExit({e.code!r})")
        return finish(2)
    except BaseException as e:  # noqa: BLE001
        try:
            traceback.print_exc()
        except BaseException:  # noqa: BLE001
            pass
        report("harness error", f"{type(e).__name__}: {e}")
        return finish(2)


if __name__ == "__main__":
    sys.exit(_entry(sys.argv))
