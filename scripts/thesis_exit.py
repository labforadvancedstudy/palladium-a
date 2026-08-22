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
AGGREGATE_ROW = "D1-01"          # cites this command as its evidence: it is the summary

# THE COMPLETE THESIS CONTRACT, not just its ids. Pinning ids alone left three ways to
# change the definition of 1.0 without tripping anything: retype a row to another
# DISPATCHED kind (reject -> fixture turns a negative test into a positive one), point it
# at a different fixture, or blank its required fingerprint back to `-` — which made
# `p_verdict` skip the fingerprint comparison entirely and reopened the exact hole the
# ninth column was added to close. id -> (kind, evidence, fingerprint).
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
TOTAL_ON_FN = re.compile(
    r"#!?\[\s*total\s*(?:\([^)]*\))?\s*\]\s*(?:#!?\[[^\]]*\]\s*)*(?:pub\s+)?fn\s+([A-Za-z_][A-Za-z_0-9]*)"
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
    require_modellable(src, "TH-03 / `ref` parameter reachability")
    bodies = function_bodies(src)
    reachable = reachable_from_main(bodies)
    if not reachable:
        return False, "no `fn main`, so nothing is reachable"
    dead = []
    for m in FN_HEADER.finditer(src):
        if not REF_PARAM.search(balanced_span(src, m.end() - 1)):
            continue
        if m.group(1) in reachable:
            return True, f"fn {m.group(1)}, reachable from main"
        dead.append(m.group(1))
    if dead:
        return False, (f"`ref` parameters exist but only on function(s) unreachable from "
                       f"main: {', '.join(sorted(set(dead)))}")
    return False, "no `fn` declares a `ref` / `ref mut` PARAMETER"


# --- THE STATIC-REACHABILITY CONTRACT ------------------------------------------------
#
# All three differentiator probes ask the same question — "is this on a path the program
# can actually run?" — so the answer needs a written contract rather than whatever a
# regex happens to do. This is that contract. It is deliberately small and deliberately
# LOUD at its edges.
#
#   R1  A CALL is `name(` appearing in a function body, where `name` is a function
#       defined in the same compilation unit. Self-edges do not count: a function's own
#       recursive call is not evidence anything reaches it.
#
#   R2  A call inside a block guarded by a LITERAL FALSE condition — `if false { … }`,
#       `while false { … }` — is NOT a call. This is the only folding performed, and the
#       minimality is the point: it refuses the cheapest ornament without pretending to
#       be a partial evaluator. `if 1 == 2 { … }` is NOT folded; it is modelled as live,
#       which over-approximates, which is the safe direction for a gate that is trying to
#       catch decoration.
#
#   R3  An INDIRECT call has no statically visible target: dispatch through a trait bound
#       (`T::method(…)` where `T` is a type parameter), a `.`-method call, a closure, or a
#       function value. A GENERIC FUNCTION IS NOT ITSELF INDIRECT — `fn map<T>(x: T)` is
#       called as `map(…)` and that edge is perfectly visible, which is why the fix for it
#       was to teach FN_HEADER the optional parameter list, not to give up. What the model
#       cannot see is the DISPATCH, not the genericity.
#
#   R4  THEREFORE: any construct whose edges this model cannot see is a HARNESS ERROR,
#       not a silent edge and not a silent non-edge. The gate stops and says it cannot
#       measure (exit 2) instead of returning an answer it cannot support.
#
#       This matters more than it looks. M3 — the very next milestone — is traits and
#       generics, moved to second on the argument that bootstrap/pdc.pd is 991 lines
#       BECAUSE it cannot abstract. Trait-bound dispatch and `.`-methods arrive with it,
#       and a model that silently under-approximated would falsely reject a legitimate
#       1.0 compiler for using the features this roadmap schedules first. Failing loudly
#       turns that into a prompt to replace the model rather than a wrong verdict.
#
#   R5  The replacement is not a better regex. `pdc` already builds a call graph for
#       effect analysis; the gate should consume it. That is requirement GI-11, owned by
#       M3, so it is scheduled rather than merely regretted.

FALSE_BLOCK = re.compile(r"(?<![A-Za-z_0-9])(if|while)\s+false\s*\{")
FN_TYPE_PARAM = re.compile(r":\s*fn\s*\(")
CLOSURE = re.compile(r"\|[^|\n]*\|\s*(\{|[A-Za-z_0-9])")
METHOD_CALL = re.compile(r"\.\s*[A-Za-z_][A-Za-z_0-9]*\s*\(")

UNMODELLABLE = (
    (FN_TYPE_PARAM, "a function-typed parameter — R3/R4"),
    (CLOSURE, "a closure — R3/R4"),
    (METHOD_CALL, "a `.`-method call — R3/R4"),
)

# Dispatch through a type parameter: `T::method(…)` where `T` is bound by the enclosing
# function's generic list. This is what actually arrives with M3, and it is the case the
# model must refuse rather than guess at.
GENERIC_PARAMS = re.compile(
    r"(?<![A-Za-z_0-9])fn\s+[A-Za-z_][A-Za-z_0-9]*\s*<([^(){}]*)>\s*\(")


def unmodellable(src: str) -> list[str]:
    """Constructs whose call edges R1-R3 cannot see. Non-empty means: do not answer."""
    found = [why for pat, why in UNMODELLABLE if pat.search(src)]
    params = set()
    for m in GENERIC_PARAMS.finditer(src):
        for raw in m.group(1).split(","):
            name = raw.split(":")[0].strip().lstrip("'")
            # A lifetime-only list (`fn f<'a>(…)`) binds no type, so it dispatches
            # nothing; TH-02 already gives a definitive answer about it.
            if name and not raw.strip().startswith("'"):
                params.add(name)
    for tp in sorted(params):
        if re.search(rf"(?<![A-Za-z_0-9]){re.escape(tp)}::\s*[A-Za-z_]", src):
            found.append(f"dispatch through the type parameter `{tp}::…` — R3/R4")
    return found


def require_modellable(src: str, what: str) -> None:
    found = unmodellable(src)
    if found:
        raise HarnessError(
            f"{what}: this gate's reachability model cannot see the call edges of "
            + "; ".join(found)
            + ". Refusing to report reachability it cannot support — see the "
              "static-reachability contract in scripts/thesis_exit.py (R4), and GI-11, "
              "which replaces this model with the compiler's own call graph.")


def strip_false_blocks(body: str) -> str:
    """R2. Remove `if false { … }` / `while false { … }`, braces balanced."""
    out, i = [], 0
    while i < len(body):
        m = FALSE_BLOCK.search(body, i)
        if not m:
            out.append(body[i:])
            break
        out.append(body[i:m.start()])
        depth, j = 0, m.end() - 1
        while j < len(body):
            if body[j] == "{":
                depth += 1
            elif body[j] == "}":
                depth -= 1
                if depth == 0:
                    j += 1
                    break
            j += 1
        i = j
    return "".join(out)


def reachable_from_main(bodies: dict[str, str]) -> set[str]:
    """Functions the program can actually run, per the contract above.

    The single discipline for all three differentiator probes. Written for `#[total]`
    and once applied only there, so `fn ornament(x: ref T)` in dead code satisfied the
    reference probe and an unreachable chain satisfied the effect probe.
    """
    if "main" not in bodies:
        return set()
    reachable, frontier = {"main"}, ["main"]
    while frontier:
        cur = frontier.pop()
        for c in callees(strip_false_blocks(bodies.get(cur, "")), exclude=cur):
            if c in bodies and c not in reachable:
                reachable.add(c)
                frontier.append(c)
    return reachable


def p_total_on_fn(src: str) -> tuple[bool, str]:
    """A `#[total]` on a function REACHABLE FROM `main`.

    "Appears in some body" was not reachability: a dead caller satisfied it, and so did
    the function's own recursive call. Both are the ornament class one level in.
    """
    names = [m.group(1) for m in TOTAL_ON_FN.finditer(src)]
    if not names:
        return False, "no `#[total]` attached to a `fn`"
    require_modellable(src, "TH-04 / #[total] reachability")
    bodies = function_bodies(src)
    reachable = reachable_from_main(bodies)
    if not reachable:
        return False, "no `fn main`, so nothing is reachable"
    live = [n for n in names if n in reachable]
    if not live:
        return False, f"`#[total]` only on function(s) unreachable from main: {', '.join(names)}"
    return True, f"#[total] on {', '.join(live)}, reachable from main"


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

    reachable = reachable_from_main(bodies)
    for caller in sorted(reported):
        if caller not in bodies:
            continue                    # reported but not defined here: no edge to show
        if caller not in reachable:
            continue                    # an unreachable chain is an ornament, not a path
        if direct_io(caller):
            continue                    # a DIRECT effect proves nothing about propagation
        for callee in sorted(callees(bodies[caller], exclude=caller)):
            io = direct_io(callee)
            if io:
                return True, (f"`{caller}` performs no IO itself -> calls `{callee}` -> "
                              f"`{sorted(io)[0]}`; reported {reported[caller]}")
    named = ", ".join(sorted(reported))
    return False, (f"no caller REACHABLE FROM MAIN exhibits the edge caller -> callee -> "
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

    for r in rows:
        rid, kind = r["id"], r["kind"]
        if rid == AGGREGATE_ROW:
            continue                              # it IS the summary; see the docstring
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

    expected = {r["id"] for r in rows} - {AGGREGATE_ROW}
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
    rows = thesis_rows(ctx)
    results = evaluate(ctx, rows)
    by_id = {r["id"]: r for r in rows}

    print("=" * 78)
    print("  make thesis-exit — the definition of Palladium 1.0")
    print(f"  {len(rows)} `thesis` rows from {ctx.requirements.name}; "
          f"{AGGREGATE_ROW} is the aggregate and is answered by the summary")
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

    red = [r for r in results if not r[1]]
    print("\n" + "=" * 78)
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
]


def _rows(drop=None, retype=None, repoint=None, blank_fp=None, extra=""):
    out = [HDR]
    for rid, ms, kind, ev, fp in BASE_ROWS:
        if rid == drop:
            continue
        if retype and rid == retype[0]:
            kind, ev = retype[1], retype[2]
        if repoint and rid == repoint[0]:
            ev = repoint[1]
        if rid == blank_fp:
            fp = "-"
        out.append(f"{rid}\t{ms}\tsrc\treq {rid}\t{kind}\t{ev}\towed\tthesis\t{fp}\n")
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
GOOD_MAKE = {"selfhost": 0, "selfhost-corpus": 0, "thesis-exit": 0}


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
    if old not in ALL_VERDICTS:
        raise HarnessError(f"self-test: no verdict line for {row_id} ({old!r}) to mutate")
    return ALL_VERDICTS.replace(old, new)
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
           real_make=False) -> int:
    """Run the WHOLE gate against an injected repository state."""
    with tempfile.TemporaryDirectory() as d:
        tmp = Path(d)
        (tmp / "a.pd").write_text(GOOD_WITNESS)
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
                      effect_reports=reports)
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
                          effect_reports=reports)
        try:
            with redirect_stdout(io.StringIO()):
                return main(ctx)
        except HarnessError:
            return 2


def self_test() -> int:
    global GP
    if GP is None:
        GP = _load_gate_probe()
    fails = cases = driven = 0

    def case(name, got, want, drives_main=True):
        nonlocal fails, cases, driven
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
         _drive(fingerprints={**GOOD_FP,
                              _C["N9-06"][1]: "Unsupported type in reference parameter"}), 1)
    case("the other five declarations are untouched by that mutation",
         len(GOOD_FP), 6, drives_main=False)
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
    case("an ornament called only from `if false { … }` goes RED (R2)",
         _drive(witness_b=mutate(GOOD_WITNESS, "fn main() { drive(s, c); }",
                                 "fn main() { drive2(c); if false { drive(s, c); } }\n"
                                 "fn drive2(mut c: C) { header(c); }")), 1)
    case("a GENERIC function is visible to the model, not silently invisible (R3)",
         _drive(witness_b=mutate(GOOD_WITNESS, "fn drive(x: ref String, mut c: C)",
                                 "fn drive<T>(x: ref String, mut c: C)")), 0)
    case("trait-bound dispatch `T::m(…)` is a HARNESS ERROR, never a guess (R4)",
         _drive(witness_b=GOOD_WITNESS + "fn show<T: Display>(x: T) { T::fmt(x); }\n"), 2)
    case("a `.`-method call is a HARNESS ERROR (R4)",
         _drive(witness_b=GOOD_WITNESS + "fn m(s: S) { s.len(); }\n"), 2)
    case("a function-typed parameter is a HARNESS ERROR (R4)",
         _drive(witness_b=GOOD_WITNESS + "fn hof(f: fn(i64) -> i64) { }\n"), 2)
    case("`#[total]` reachable only from a DEAD fn goes RED",
         _drive(witness_b=mutate(GOOD_WITNESS, "return depth(1);", "return 1;")
                          + "fn dead() -> i64 { return depth(2); }\n"), 1)
    case("`#[total]` on a self-recursive-only fn goes RED",
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
    case("a mutation whose replacement equals the original is rejected",
         _mut("return depth(1);", "return depth(1);"), "rejected", drives_main=False)

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
        print(f"  self-test green — {cases} cases: {driven} drive main() end to end, "
              f"{cases - driven} exercise a helper directly (lexer, process boundary)")
        print(f"  {len(EXPECTED_UNCOVERED)} probe group(s) pinned as uncovered, listed above")
        print("=" * 78)
        return 0
    print(f"  self-test RED — {fails} of {cases} cases failed")
    print("=" * 78)
    return 1


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

    try:
        if "--crash-for-self-test" in argv:
            # Exercised by the self-test: any unwrapped failure must leave as exit 2, the
            # measurement code, never as Python's default 1.
            raise RuntimeError("deliberate crash, exercised by --self-test")
        if "--systemexit-for-self-test" in argv:
            # A dependency calling sys.exit(1) must not be mistaken for a thesis verdict.
            raise SystemExit(1)
        return self_test() if "--self-test" in argv else main()
    except HarnessError as e:
        report("harness error", str(e))
        return 2
    except SystemExit as e:
        # NOT re-raised. A SystemExit from anywhere below is a dependency's opinion about
        # the process, not this gate's verdict about Palladium.
        report("harness error", f"a dependency raised SystemExit({e.code!r})")
        return 2
    except BaseException as e:  # noqa: BLE001
        try:
            traceback.print_exc()
        except BaseException:  # noqa: BLE001
            pass
        report("harness error", f"{type(e).__name__}: {e}")
        return 2


if __name__ == "__main__":
    sys.exit(_entry(sys.argv))
