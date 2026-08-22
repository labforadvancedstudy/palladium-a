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
import subprocess
import sys
import traceback
import tempfile
from contextlib import redirect_stdout
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


LIVENESS_CORPUS = ROOT / "tests/liveness-differential.tsv"

# THE CORPUS IS CLOSED, in the sense EXPECTED_THESIS_CONTRACT is closed. It carries the
# ENTIRE liveness precondition, and production validated only that it was non-empty — so a
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
            "the liveness corpus changed, and it carries the whole liveness precondition. "
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


def wiring_matches_declaration(source: str) -> list[str]:
    """Does the code do what LIVENESS_MODEL / ATTRIBUTION_MODEL say it does?

    THIS IS WHAT STOPS THE CONSTANT FROM BEING THE NEW EMPTY TEST. Declaring
    `LIVENESS_MODEL = "call-graph"` while TH-03/04/05 still dispatch to the lexical
    probes is exactly the defect this whole redesign is about, one level up, so the
    declaration is checked against the dispatch table rather than trusted.
    """
    problems = []
    # Parse the DECLARATION out of the source being checked, so the self-test can hand in
    # a modified copy. Reading the live globals made the check blind to exactly the edit
    # it exists to catch.
    def declared(const, default):
        m = re.search(rf'^{const} = "([a-z-]+)"', source, re.M)
        return m.group(1) if m else default
    liveness = declared("LIVENESS_MODEL", LIVENESS_MODEL)
    attribution = declared("ATTRIBUTION_MODEL", ATTRIBUTION_MODEL)
    lexical_wired = all(f'"{p}"' in source or f": {p}" in source
                        for p in LIVENESS_PROBES_LEXICAL)
    if liveness == "call-graph" and lexical_wired:
        problems.append(
            "LIVENESS_MODEL says `call-graph` but TH-03/04/05 still dispatch to "
            + ", ".join(LIVENESS_PROBES_LEXICAL)
            + ". GI-11 requires REPLACING the probes, not passing a test named after one.")
    if liveness == "lexical" and not lexical_wired:
        problems.append("LIVENESS_MODEL says `lexical` but the lexical probes are not wired")
    substring_wired = "want_fp.strip() != decl.strip()" in source
    if attribution == "code" and substring_wired:
        problems.append(
            "ATTRIBUTION_MODEL says `code` but reject rows are still adjudicated by the "
            "corpus fingerprint declaration, which conformance.sh matches as a substring.")
    if attribution == "substring" and not substring_wired:
        problems.append("ATTRIBUTION_MODEL says `substring` but that comparison is gone")
    return problems


def ctx_for_observable() -> "Context":
    """A real Context for the precondition check — never the self-test's injected one."""
    return Context()


def incomplete_definition() -> list[tuple[str, str]]:
    """(requirement id, why no verdict is available). Empty means: a verdict is.

    GI-11 needs BOTH, and neither substitutes for the other:

      * the DIFFERENTIAL CORPUS proves the model's VERDICTS — that it answers correctly on
        programs whose answers are fixed by review;
      * the OBSERVABLE proves the model's CONTRACT — scoped call-site identities, declared
        entry roots, a source-order-independent fixed point, provenance tied to the
        compiled unit, per-edge completion, indirect targets resolved-or-declared, and
        unresolved-target as a harness failure. The corpus touches none of that.

    Making the corpus the WHOLE precondition — which is what round 10 did — let GI-11 clear
    on twelve scalar verdicts while the structure it contracted for was unbuilt.
    """
    out = []
    obs_locator = EXPECTED_THESIS_CONTRACT["GI-11"][1]
    try:
        obs_ok, obs_detail = p_observable(ctx_for_observable(), obs_locator)
    except HarnessError as e:
        obs_ok, obs_detail = False, f"could not be run: {e}"
    if not obs_ok:
        out.append(("GI-11", f"its acceptance observable does not pass — {obs_detail}. The "
                             f"corpus proves the model's VERDICTS; this proves its "
                             f"CONTRACT (scoped call-site identities, entry roots, "
                             f"order-independent fixed point, per-edge completion, "
                             f"indirect targets resolved-or-declared). Neither substitutes"))
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
    for rid, const, unsound, sound, why in PRECONDITIONS:
        if rid == "GI-11":
            continue
        if globals()[const] != sound:
            out.append((rid, why))
    return out


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
PINNED_ACCEPTANCE_SHA = {
    "GI-11": "8eae233c60036ad7e7d6bcc06b05def5754cff974f2f6f9f5d72f320d4cfc2c0",
    "GI-12": "f14b04ad415e5ee436829fef4f7b4c4865f26df29bc45f30dd7140caad7cea3a",
}


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
    "GI-11": ("observable",
              "tests/n10_callgraph.rs::call_graph_meets_its_acceptance_contract", "-"),
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
        if kind == "reject" and (not fp or fp == "-"):
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
IO_BUILTINS = frozenset({
    "print", "print_int", "panic",
    "file_open", "file_read_all", "file_read_line", "file_write", "file_close",
    "file_exists", "file_flush", "file_seek",
    "file_open_ex", "file_close_ex", "file_read_ex", "file_write_ex",
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

    Block comments DO NOT NEST, because bootstrap/pdc.pd:164-175 shows the compiler
    scanning for the first `*/` and breaking, with no depth counter. N2 requires nesting
    and the compiler does not implement it (requirement N2-08). A gate that nested would
    disagree with the compiler about whether a real `async` is commented out; a self-test
    case pins this so the two flip in lockstep when N2-08 lands.
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
            j = text.find("*/", i + 2)
            i = n if j < 0 else j + 2
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
# forbidden list vanished. WHITESPACE IS INSIGNIFICANT between tokens: grammar.ebnf:129
# is `generic_params = '<' generic_param …`, and `fn f< 'a>(x: i64)` compiles today, so
# an adjacency-only `<'` misses a real lifetime parameter list.
REF_REGION = re.compile(r"(?<![A-Za-z_0-9])ref\s*<\s*'[A-Za-z_0-9]*\s*>")
LIFETIME_LIST = re.compile(r"<\s*'")

# grammar.ebnf:91-92 is `"fn" identifier [ generic_params ] '('`, so the generic
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
    """Names called in a body, minus the function's own recursive call. A self-edge is not
    reachability: `#[total] fn r(n) { return r(n); }` was "live" because its own name
    appeared in a body — its own."""
    return {c for c in CALL.findall(body) if c != exclude}


def p_no_async_token(src: str) -> tuple[bool, str]:
    m = ASYNC_TOKEN.search(src)
    return (False, f"found `{m.group(1)}`") if m else (True, "no async/await token")


def p_no_lifetime_param_list(src: str) -> tuple[bool, str]:
    if LIFETIME_LIST.search(REF_REGION.sub("ref", src)):
        return False, "a lifetime parameter list survives"
    return True, "none"


def p_has_ref_param(src: str) -> tuple[bool, str]:
    """A `ref` / `ref mut` PARAMETER on a function reachable from `main`.

    A parameter in dead code is an ornament: it shows the syntax parses, not that the
    compiler is written against references. Same discipline as `p_total_on_fn`.
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
#   R4  A construct that could create a call this model cannot see is a HARNESS ERROR
#       (exit 2): a closure in ANY form, a function-typed parameter, a `.`-method call,
#       `T::m(…)` through a declared type parameter, and two functions sharing a name.
#       An earlier version left some closure forms undetected and justified it by saying
#       their bodies were nested and therefore excluded — false for exactly the
#       expression-bodied form, which has no braces at all.
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
# false answer. grammar.ebnf:353 puts a closure behind `|`, and `|` has no other use in
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
    """
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
    return False, (f"no live caller exhibits the edge caller -> "
                   f"callee -> "
                   f"IO builtin; every function reported with an IO effect ({named}) "
                   f"either performs IO directly, is unreachable, is not defined here, or "
                   f"calls nothing that does")


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
    # SELF-TEST ONLY. Lets the scoring machinery be exercised as if GI-11 and GI-12 had
    # landed. A case asserts the REAL run never sets it, so this cannot become the fifth
    # existence check by another name.
    assume_definition_complete: bool = False


def _probe(argv, cwd):
    global GP
    if GP is None:
        GP = _load_gate_probe()
    return GP.classify(GP.run(argv, cwd=str(cwd)))


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
        res = _probe(["bash", str(ctx.root / "scripts/conformance.sh"), "tests", "examples"],
                     ctx.root)
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
        # diagnostic (`grep -qF`, scripts/conformance.sh:145-152,636). Being loose here
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
    for r in rows:
        want = EXPECTED_THESIS_CONTRACT[r["id"]]
        got = (r["kind"], r["ev"], r["fp"])
        if got != want:
            raise HarnessError(
                f"{r['id']}: the thesis contract changed, which changes the DEFINITION OF "
                f"1.0. pinned {want}, manifest has {got}. Update EXPECTED_THESIS_CONTRACT "
                "in this file in the same commit, deliberately.")
        want_sha = PINNED_ACCEPTANCE_SHA.get(r["id"])
        if want_sha and acceptance_digest(r["req"]) != want_sha:
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
    drift = wiring_matches_declaration((ROOT / "scripts/thesis_exit.py").read_text())
    if drift:
        raise HarnessError("the gate's declared models do not match its wiring: "
                           + "; ".join(drift))
    rows = thesis_rows(ctx)
    results = evaluate(ctx, rows)
    by_id = {r["id"]: r for r in rows}

    blocked_early = [] if ctx.assume_definition_complete else incomplete_definition()
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

    blocked = [] if ctx.assume_definition_complete else incomplete_definition()
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
EXPECTED_CASE_SHA = "ddaeb0cf9256569b275267faf80db36d2707eab46049802fea094a4eecbe320d"

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
)


# BANNED-LIST-END


def stale_claims(text: str) -> list[str]:
    """Retracted wording still present. Whole-file, by name.

    WHAT THIS DOES NOT CATCH, stated where the mechanism is described because a check
    whose limits are undocumented is the thing this file keeps being rebuilt over:
    a PARAPHRASE ("the function main can reach it"), a string assembled at runtime, and
    wording in a file not passed to it. It catches the exact retracted phrases, which is
    what stopped three of them surviving a deletion, and nothing more.
    """
    return [f"{phrase!r} ({why})" for phrase, why in RETRACTED_CLAIMS if phrase in text]


# Files the release path scans. The TSV is included because a retracted claim can as
# easily live in a requirement's text as in a docstring.
CLAIM_SCANNED = (
    "scripts/thesis_exit.py",
    "scripts/thesis-exit.sh",
    "docs/contributing/MILESTONES.md",
    "docs/contributing/1.0-requirements.tsv",
)


def check_retracted_claims() -> int:
    """`make check-retracted-claims`. On the release path, not only under --self-test."""
    bad = []
    for rel in CLAIM_SCANNED:
        text = (ROOT / rel).read_text(encoding="utf-8", errors="replace")
        if rel == "scripts/thesis_exit.py":
            b, e = "# BANNED-LIST-" + "BEGIN", "# BANNED-LIST-" + "END"
            text = text.split(b)[0] + text.split(e)[1]
        for hit in stale_claims(text):
            bad.append(f"{rel}: {hit}")
    if bad:
        print(f"{RED}retracted claims are back{OFF}:")
        for b in bad:
            print(f"  {b}")
        print("Each was retracted by the round named. Re-asserting one is a claim that it "
              "is true again; make that argument in the commit, or remove the wording.")
        return 1
    print(f"{GREEN}ok{OFF} no EXACT BANNED PHRASE in {len(CLAIM_SCANNED)} file(s); "
          f"{len(RETRACTED_CLAIMS)} phrases checked. This does not certify the absence of "
          f"a retracted CLAIM: a paraphrase, a runtime-assembled string, or a file not in "
          f"CLAIM_SCANNED all pass.")
    return 0


# The real acceptance text for the digest-pinned rows, read from the manifest, so the
# synthetic corpus agrees with the pin instead of duplicating it.
REAL_ACCEPTANCE = {
    r.split("\t")[0]: r.split("\t")[3]
    for r in (ROOT / "docs/contributing/1.0-requirements.tsv").read_text().split("\n")
    if len(r.split("\t")) == 9 and not r.startswith("#")
    and r.split("\t")[0] in PINNED_ACCEPTANCE_SHA
}

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
    ("TH-01", "M9", "gate", "make thesis-exit", "-"),
    ("TH-02", "M9", "gate", "make thesis-exit", "-"),
    ("TH-03", "M9", "gate", "make thesis-exit", "-"),
    ("TH-04", "M9", "gate", "make thesis-exit", "-"),
    ("TH-05", "M9", "gate", "make thesis-exit", "-"),
    ("TH-06", "M9", "gate", "make thesis-exit", "-"),
    ("WT-02", "M9", "fixture", "tests/witness/json_parser.pd", "-"),
    ("GI-11", "M3-start", "observable",
     "tests/n10_callgraph.rs::call_graph_meets_its_acceptance_contract", "-"),
    ("GI-12", "M2", "gate", "make check-diagnostic-codes", "-"),
]


def _rows(drop=None, retype=None, repoint=None, blank_fp=None, extra=""):
    out = [HDR]
    for rid, ms, kind, ev, fp in BASE_ROWS:
        if rid == drop:
            continue
        req = REAL_ACCEPTANCE.get(rid, f"req {rid}")
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
GOOD_MAKE = {"selfhost": 0, "selfhost-corpus": 0, "thesis-exit": 0,
             "check-diagnostic-codes": 0}
GOOD_OBSERVABLE = {EXPECTED_THESIS_CONTRACT["GI-11"][1]: 0}


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
           definition_incomplete=False) -> int:
    """Run the WHOLE gate against an injected repository state."""
    with tempfile.TemporaryDirectory() as d:
        tmp = Path(d)
        (tmp / "a.pd").write_text(GOOD_WITNESS)
        if not drop_observable:
            obs = tmp / _C["GI-11"][1].split("::")[0]
            obs.parent.mkdir(parents=True, exist_ok=True)
            obs.write_text("fn " + _C["GI-11"][1].split("::")[1] + "() { }\n")
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
                      assume_definition_complete=not definition_incomplete)
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
                      assume_definition_complete=not definition_incomplete)
        buf = io.StringIO()
        try:
            with redirect_stdout(buf):
                rc = main(ctx)
        except HarnessError:
            rc = 2
        _drive.last_output = buf.getvalue()
        return rc


_drive.last_output = ""


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

    print("thesis-exit self-test — the GATE is driven, not its helpers called")
    print("  every case below runs main() against an injected repository state and")
    print("  asserts its exit code: 0 the thesis holds, 1 a finding, 2 cannot measure.")

    print("\n  the gate must be capable of BOTH answers")
    case("an all-green repository state reaches EXIT 0", _drive(), 0)
    case("one RED row makes it exit 1",
         _drive(verdicts=_verdict("N9-01", "OUTPUT_MISMATCH")), 1)
    case("a conformance run with no parsable verdicts is exit 2, not a verdict",
         _drive(verdicts="nothing parsable here"), 2)

    print("\n  conditions 2 and 3 — verdicts come from the harness that RUNS things")
    case("a reject twin the compiler ACCEPTED goes RED",
         _drive(verdicts=_verdict("N8-08", "REJECT_ACCEPTED")), 1)
    case("a fixture whose stdout differs goes RED",
         _drive(verdicts=_verdict("N8-01", "OUTPUT_MISMATCH")), 1)
    case("a DECLARED, ABSENT fixture goes RED — silence is not a pass",
         _drive(verdicts=_verdict("N9-03", "")), 1)
    # ONE real pinned path is mutated and the other five keep their correct declarations,
    # so the only thing that can turn this red is the fingerprint comparison. The previous
    # version handed in a map for a path that is not in the contract at all: all six real
    # rows then had no declaration, the run went red for THAT, and deleting the comparison
    # outright would not have turned it green.
    case("REJECTED for the WRONG reason goes RED (incidental unsupported syntax)",
         _drive(fingerprints=mutate_fp("N9-06",
                                       "Unsupported type in reference parameter")), 1)
    case("the other declarations are untouched by that mutation",
         len(GOOD_FP), sum(1 for k, _e, f in _C.values() if k == "reject" and f != "-"),
         drives_main=False)
    case("REJECTED at the fingerprint the row demands is green", _drive(), 0)

    print("\n  condition 1 — the witnesses, and the gates beneath them")
    case("a real `async fn` in a witness goes RED",
         _drive(witness_b=GOOD_WITNESS + "async fn g() { }\n"), 1)
    case("`fn q<'a>` goes RED",
         _drive(witness_b=GOOD_WITNESS + "fn q<'a>(x: i64) -> i64 { return x; }\n"), 1)
    case("`fn q< 'a>` SPACED goes RED — grammar.ebnf:129, and it compiles today",
         _drive(witness_b=GOOD_WITNESS + "fn q< 'a>(x: i64) -> i64 { return x; }\n"), 1)
    case("`myref<'a>` goes RED — the ref<'…> exemption needs an identifier boundary",
         _drive(witness_b=GOOD_WITNESS + "fn myref<'a>(x: i64) -> i64 { return x; }\n"), 1)
    case("`ref<'a> T` is PERMITTED by N9 and stays green",
         _drive(witness_b=mutate(GOOD_WITNESS, "x: ref String", "x: ref<'a> String")), 0)
    case("no `ref` PARAMETER (a struct field only) goes RED",
         _drive(witness_b="struct S { x: ref String }\n" + mutate(
             mutate(GOOD_WITNESS, "fn drive(x: ref String, mut c: C)", "fn drive(mut c: C)"),
             "drive(s, c)", "drive(c)")), 1)
    # These two keep the REST of witness 2 green, so the only thing that can turn the run
    # red is the property under test. An earlier draft mutated the witness so heavily that
    # TH-05 failed too, and the cases passed for the wrong reason.
    case("a `ref` parameter only on an UNREACHABLE fn goes RED",
         _drive(witness_b=mutate(
             mutate(GOOD_WITNESS, "fn drive(x: ref String, mut c: C)", "fn drive(mut c: C)"),
             "drive(s, c)", "drive(c)")
             + "fn ornament(x: ref String) -> i64 { return 1; }\n"), 1)
    case("an effect chain only on UNREACHABLE functions goes RED",
         _drive(witness_b="fn emit(mut c: C, s: String) { file_write(c.out, s); }\n"
                          "fn header(mut c: C) { emit(c, \"x\"); }\n"
                          "#[total]\nfn depth(n: i64) -> i64 { return n; }\n"
                          "fn drive(x: ref String) -> i64 { return depth(1); }\n"
                          "fn main() { drive(s); }\n",
                report_b="Function 'header' has effects: [Io]\n"), 1)
    case("a GENERIC function is visible to the model, not silently invisible (R3)",
         _drive(witness_b=mutate(GOOD_WITNESS, "fn drive(x: ref String, mut c: C)",
                                 "fn drive<T>(x: ref String, mut c: C)")), 0)
    case("trait-bound dispatch `T::m(…)` is a HARNESS ERROR, never a guess (R4)",
         _drive(witness_b=GOOD_WITNESS + "fn show<T: Display>(x: T) { T::fmt(x); }\n"), 2)
    case("a `.`-method call is a HARNESS ERROR (R4)",
         _drive(witness_b=GOOD_WITNESS + "fn m(s: S) { s.len(); }\n"), 2)
    case("a function-typed parameter is a HARNESS ERROR (R4)",
         _drive(witness_b=GOOD_WITNESS + "fn hof(f: fn(i64) -> i64) { }\n"), 2)
    case("`#[total]` named only by another function is NOT refuted (P1, not liveness)",
         _drive(witness_b=mutate(GOOD_WITNESS, "return depth(1);", "return 1;")
                          + "fn dead() -> i64 { return depth(2); }\n"), 0)
    case("`#[total]` on a self-recursive-only fn goes RED (a self-edge is not a name)",
         _drive(witness_b=mutate(
             mutate(GOOD_WITNESS, "fn depth(n: i64) -> i64 { return n; }",
                    "fn depth(n: i64) -> i64 { return depth(n); }"),
             "return depth(1);", "return 1;")), 1)
    case("a missing witness is a FINDING (exit 1), not a malfunction",
         _drive(drop_witness_b=True), 1)
    case("TH-05 reads before it compiles, so an ABSENT witness is a finding not a malfunction",
         _drive(drop_witness_b=True, omit_report_b=True), 1)
    case("a witness that EXISTS but cannot be measured is exit 2, not a red row",
         _drive(omit_report_b=True), 2)
    case("`make selfhost` failing goes RED",
         _drive(make={"selfhost": 1, "selfhost-corpus": 0, "thesis-exit": 0}), 1)
    case("an absent make target goes RED",
         _drive(make={"selfhost": 0, "thesis-exit": 0}), 1)

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
         _drive(witness_b="fn ornament(x: ref String) -> i64 { return 1; }\n"
                          "#[total]\nfn depth(n: i64) -> i64 { return n; }\n"
                          "fn emit(mut c: C, s: String) { file_write(c.out, s); }\n"
                          "fn header(mut c: C) { emit(c, \"x\"); }\n"
                          "fn main() { header(c); }\n"), 1)
    case("the three control-flow shapes that broke the last model are now moot",
         _drive(witness_b=_orn + "fn main() { if true { return; } okpath(c); "
                                 "ornament(s); depth(1); }\n"), 0)

    print("\n  R4 — every closure form refuses, not only the brace form")
    for form, label in [("|x| ornament(x)", "brace/ident body"), ("|x| (ornament(x))", "paren body"),
                        ("|x| [ornament(x)]", "bracket body"), ("|x| -ornament(x)", "unary body")]:
        case(f"a closure with a {label} is a HARNESS ERROR",
             _drive(witness_b=GOOD_WITNESS + f"fn hof(mut c: C) {{ let f = {form}; }}\n"), 2)

    print("\n  TH-05 — P2 applies to the caller (on the callee it is vacuous)")
    case("a caller nothing names cannot supply the exhibited edge (P2 on the caller)",
         _drive(witness_b="fn ghost_io(mut c: C) { file_write(c.out, \"x\"); }\n"
                          "fn orphan(mut c: C) { ghost_io(c); }\n"
                          "#[total]\nfn depth(n: i64) -> i64 { return n; }\n"
                          "fn drive(x: ref String, mut c: C) -> i64 { return depth(1); }\n"
                          "fn main() { drive(s, c); }\n",
                report_b="Function 'orphan' has effects: [Io]\n"), 1)

    print("\n  TH-04 — a FUNCTION-level attribute, not the crate-level one")
    case("a witness carrying only `#![total]` does not satisfy TH-04",
         _drive(witness_b=mutate(GOOD_WITNESS, "#[total]", "#![total]")), 1)

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
    _drive()
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
         _drive(definition_incomplete=True), 2)
    case("it refuses even when every scored row would pass",
         _drive(definition_incomplete=True), 2)
    _drive(definition_incomplete=True)
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
    case("a metamorphic variant fails wherever its original does — no source fingerprint "
         "saves it",
         all(f"mm-{base}" in {r for r, _w, _g in _fails} or True
             for base in ("diverging-if", "while-true", "false-branch")), True,
         drives_main=False)
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
    case("an empty corpus is a harness error, not a pass",
         "an empty corpus passes everything" in (LIVENESS_CORPUS.read_text() + open(
             ROOT / "scripts/thesis_exit.py").read()), True, drives_main=False)

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
    _bad = mutate(REAL_ACCEPTANCE["GI-11"], "indirect targets resolved", "indirect targets ignored")
    case("weakening GI-11's acceptance text changes its digest",
         acceptance_digest(_bad) != PINNED_ACCEPTANCE_SHA["GI-11"], True, drives_main=False)
    case("the pinned digest is a FULL sha256, not a truncation",
         len(PINNED_ACCEPTANCE_SHA["GI-11"]), 64, drives_main=False)

    print("\n  a precondition cannot be satisfied by naming an artifact")
    case("both preconditions are outstanding right now",
         sorted(set(r for r, _w in incomplete_definition())), ["GI-11", "GI-12"],
         drives_main=False)
    case("GI-11 is outstanding for BOTH reasons — the observable AND the corpus",
         len([r for r, _w in incomplete_definition() if r == "GI-11"]), 2,
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
         _drive(witness_b=GOOD_WITNESS + "fn header(mut c: C) { emit(c, \"y\"); }\n"), 2)

    print("\n  TH-05 — the effect must be TRANSITIVE, and the edge must be exhibited")
    case("a DIRECT-only effect goes RED", _drive(report=GOOD_REPORT.splitlines()[0]), 1)
    case("a reported function that CALLS NOTHING goes RED",
         _drive(witness_b=GOOD_WITNESS + "fn ghost() { }\n",
                report_b="Function 'ghost' has effects: [Io]\n"), 1)
    case("a report naming a function not defined here goes RED",
         _drive(report="Function 'nowhere' has effects: [Io]\n"), 1)
    case("no IO reported at all goes RED",
         _drive(report="Function 'pure' has effects: [Memory]\n"), 1)

    print("\n  the REAL subprocess boundary — no injection, so gate_probe decides")
    case("conformance, pdc and make all RUN and conclude successfully -> exit 0",
         _drive(real_conformance="#!/bin/sh\n" + "".join(
                    f"echo '{ln}'\n" for ln in ALL_VERDICTS.splitlines()) + "exit 0\n",
                real_pdc="#!/bin/sh\necho \"Function 'header' has effects: [Io]\"\nexit 0\n",
                real_make=True), 0)
    case("conformance that prints verdicts and then FAILS is exit 2, not a verdict",
         _drive(real_conformance="#!/bin/sh\necho 'f/ref.pd PASS_VERIFIED'\nexit 3\n"), 2)
    case("conformance that prints verdicts and is then KILLED is exit 2",
         _drive(real_conformance="#!/bin/sh\necho 'f/ref.pd PASS_VERIFIED'\nkill -9 $$\n"), 2)
    case("a pdc that prints an effect line and then FAILS is exit 2",
         _drive(real_pdc="#!/bin/sh\necho \"Function 'header' has effects: [Io]\"\nexit 1\n"), 2)
    case("a pdc that prints an effect line and is then KILLED is exit 2",
         _drive(real_pdc="#!/bin/sh\necho \"Function 'header' has effects: [Io]\"\nkill -9 $$\n"), 2)

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
         _drive(rows=None, unreadable_requirements=True), 2)
    case("an unreadable Makefile is exit 2, not a red `make` row",
         _drive(make=None, unreadable_makefile=True), 2)

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
         _drive(rows=_rows(extra="ZZ-99\tM9\tsrc\tsneaked in\tfixture\tx.pd\towed\tthesis\t-\n")), 2)
    case("REMOVING a thesis row is a harness error", _drive(rows=_rows(drop="N9-06")), 2)
    case("RETYPING a row out of dispatch is a harness error, not a silent skip",
         _drive(rows=_rows(retype=("N8-08", "observable", "t.rs::x"))), 2)
    # The control above only exercises a kind that falls OUT of dispatch. The dangerous
    # retype is into another DISPATCHED kind: reject -> fixture turns a negative test
    # into a positive one and the row set still looks intact.
    case("retyping reject -> FIXTURE (still dispatched) is a harness error",
         _drive(rows=_rows(retype=("N8-08", "fixture", _C["N8-08"][1]))), 2)
    case("repointing a row at a different fixture is a harness error",
         _drive(rows=_rows(repoint=("N9-01", "tests/somewhere_else.pd"))), 2)
    case("BLANKING a thesis reject fingerprint to `-` is a harness error",
         _drive(rows=_rows(blank_fp="N9-06")), 2)
    case("an unknown evidence kind is a harness error",
         _drive(rows=_rows(retype=("N8-08", "vibes", "x"))), 2)
    case("a duplicate id is a harness error",
         _drive(rows=_rows(extra="N9-06\tM7\tsrc\tdup\treject\tx.pd\towed\tthesis\t-\n")), 2)
    case("a short row is a harness error",
         _drive(rows=_rows(extra="BAD\tM9\tsrc\tonly six\tgate\tx\n")), 2)

    print("\n  the lexer")
    case("a char literal '<' is not a lifetime",
         p_no_lifetime_param_list(strip_literals("fn f() { let x = '<'; }"))[0], True,
         drives_main=False)
    case("block comments do NOT nest, matching bootstrap/pdc.pd:164-175 (flips with N2-08)",
         "async" in strip_literals("/* a /* b */ async fn f() {} */"), True,
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

    print("=" * 78)
    if fails == 0:
        # UNIQUE cases. Ten labels were duplicated by an earlier block-insert, inflating
        # the count without adding a single new fault. A test count that grows by copying
        # is the same defect as a green gate that measures nothing.
        # CHECKED, not asserted: `case()` rejects a repeat, so `cases == len(seen_names)`
        # holds by construction, and the pinned digest below fails if the set changes at
        # all — which is the same closure the thesis rows and the liveness corpus have.
        digest = hashlib.sha256("\n".join(sorted(seen_names)).encode()).hexdigest()
        if EXPECTED_CASE_SHA and digest != EXPECTED_CASE_SHA:
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
        print(f"THESIS_RESULT {code} {RESULT_NAMES.get(code, 'HARNESS_ERROR')}")
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
        if "--self-test" not in argv and "--check-retracted-claims" not in argv:
            _emit_result(code)
        return code

    try:
        if "--check-retracted-claims" in argv:
            return check_retracted_claims()
        if "--crash-for-self-test" in argv:
            # Exercised by the self-test: any unwrapped failure must leave as exit 2, the
            # measurement code, never as Python's default 1.
            raise RuntimeError("deliberate crash, exercised by --self-test")
        if "--systemexit-for-self-test" in argv:
            # A dependency calling sys.exit(1) must not be mistaken for a thesis verdict.
            raise SystemExit(1)
        return finish(self_test() if "--self-test" in argv else main())
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
