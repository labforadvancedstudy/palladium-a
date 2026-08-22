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
   scripts/gate_probe.py, whose `classify()` returns `Concluded` (has `.text`) or
   `Malfunction` (has NO text attribute). Output buffered by a dying producer is not
   reachable, rather than merely discouraged, and `run()` carries the timeout.

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

THE ROW SET IS CLOSED
---------------------
The definition lives in docs/contributing/1.0-requirements.tsv, in the rows whose
`disposition` is `thesis`. That set is PINNED here by id. Adding a row, removing one,
renaming one, or retyping one so it stops being dispatched is a harness error — exactly as
tests/conformance-manifest.txt treats an undeclared or missing fixture. Every row produces
exactly one result and that is asserted, because "the summary printed 23" while a retyped
row was silently skipped is the same defect one level up.

Usage:
    scripts/thesis-exit.sh                exit 0 only when 1.0 is real
    scripts/thesis-exit.sh --self-test    drive this gate with injected repository states
"""

from __future__ import annotations

import importlib.util
import io
import os
import re
import sys
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
    spec = importlib.util.spec_from_file_location("gate_probe", ROOT / "scripts/gate_probe.py")
    if spec is None or spec.loader is None:
        raise HarnessError("scripts/gate_probe.py is not importable")
    mod = importlib.util.module_from_spec(spec)
    sys.modules["gate_probe"] = mod
    spec.loader.exec_module(mod)
    return mod


GP = None  # the gate_probe module; loaded on first use

# The rows that ARE the definition. Pinned, so the manifest and this command cannot drift
# apart in either direction.
AGGREGATE_ROW = "D1-01"          # cites this command as its evidence: it is the summary
EXPECTED_THESIS_IDS = frozenset({
    "D1-01",
    "N7-01", "N7-02", "N7-04", "N7-08",
    "N8-01", "N8-06", "N8-08",
    "N9-01", "N9-03", "N9-04", "N9-06",
    "SH-01", "SH-02", "SH-03", "SH-04",
    "TH-01", "TH-02", "TH-03", "TH-04", "TH-05", "TH-06",
    "WT-02",
})
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

FN_HEADER = re.compile(r"(?<![A-Za-z_0-9])fn\s+([A-Za-z_][A-Za-z_0-9]*)\s*\(")
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
    for m in FN_HEADER.finditer(src):
        if REF_PARAM.search(balanced_span(src, m.end() - 1)):
            return True, f"fn {m.group(1)}"
    return False, "no `fn` declares a `ref` / `ref mut` PARAMETER"


def p_total_on_fn(src: str) -> tuple[bool, str]:
    """A `#[total]` on a function REACHABLE FROM `main`.

    "Appears in some body" was not reachability: a dead caller satisfied it, and so did
    the function's own recursive call. Both are the ornament class one level in.
    """
    names = [m.group(1) for m in TOTAL_ON_FN.finditer(src)]
    if not names:
        return False, "no `#[total]` attached to a `fn`"
    bodies = function_bodies(src)
    if "main" not in bodies:
        return False, "no `fn main`, so nothing is reachable"
    reachable, frontier = {"main"}, ["main"]
    while frontier:
        cur = frontier.pop()
        for c in callees(bodies.get(cur, ""), exclude=cur):
            if c in bodies and c not in reachable:
                reachable.add(c)
                frontier.append(c)
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
        if direct_io(caller):
            continue                    # a DIRECT effect proves nothing about propagation
        for callee in sorted(callees(bodies[caller], exclude=caller)):
            io = direct_io(callee)
            if io:
                return True, (f"`{caller}` performs no IO itself -> calls `{callee}` -> "
                              f"`{sorted(io)[0]}`; reported {reported[caller]}")
    named = ", ".join(sorted(reported))
    return False, (f"no caller exhibits the edge caller -> callee -> IO builtin; every "
                   f"function reported with an IO effect ({named}) either performs IO "
                   f"directly, is not defined here, or calls nothing that does")


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
        decl = declared_fingerprint(ctx, path)
        if want_fp.lower() not in decl.lower():
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
    mk = (ctx.root / "Makefile").read_text(encoding="utf-8")
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
            "EXPECTED_THESIS_IDS in this file in the same commit, deliberately.")
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

# Probe groups with no negative control, PINNED. An empty list used to print
# "0 probe group(s) explicitly uncovered" and still return success — a comment that looked
# like a check. Changing this set is now a deliberate edit that fails the self-test.
EXPECTED_UNCOVERED = frozenset({
    "the real `make` subprocess: a control would need a deliberately broken build. Its "
    "target-absent and nonzero-exit paths ARE covered, by injection.",
})

BASE_ROWS = [
    ("D1-01", "M9", "gate", "make thesis-exit", "-"),
    ("N7-01", "M5", "reject", "r/async_fn.pd", "there is no `async` keyword"),
    ("N7-02", "M5", "reject", "r/await.pd", "-"),
    ("N7-04", "M5", "fixture", "f/effects.pd", "-"),
    ("N7-08", "M5", "reject", "r/pure_io.pd", "-"),
    ("N8-01", "M6", "fixture", "f/total.pd", "-"),
    ("N8-06", "M6", "fixture", "f/struct_rec.pd", "-"),
    ("N8-08", "M6", "reject", "r/unproven.pd", "-"),
    ("N9-01", "M7", "fixture", "f/ref.pd", "-"),
    ("N9-03", "M7", "fixture", "f/region.pd", "-"),
    ("N9-04", "M7", "reject", "r/lt_list.pd", "-"),
    ("N9-06", "M7", "reject", "r/ambig.pd", "-"),
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
    ("WT-02", "M9", "fixture", "b.pd", "-"),   # == witnesses[1], as in production
]


def _rows(drop=None, retype=None, extra=""):
    out = [HDR]
    for rid, ms, kind, ev, fp in BASE_ROWS:
        if rid == drop:
            continue
        if retype and rid == retype[0]:
            kind, ev = retype[1], retype[2]
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
ALL_VERDICTS = "\n".join([
    "r/async_fn.pd REJECTED", "r/await.pd REJECTED", "f/effects.pd PASS_VERIFIED",
    "r/pure_io.pd REJECTED", "f/total.pd PASS_VERIFIED", "f/struct_rec.pd PASS_VERIFIED",
    "r/unproven.pd REJECTED", "f/ref.pd PASS_VERIFIED", "f/region.pd PASS_VERIFIED",
    "r/lt_list.pd REJECTED", "r/ambig.pd REJECTED", "b.pd PASS_VERIFIED",
])
GOOD_MAKE = {"selfhost": 0, "selfhost-corpus": 0, "thesis-exit": 0}
GOOD_FP = {"r/async_fn.pd": "error: there is no `async` keyword in this language"}


def _drive(*, rows=None, witness_b=GOOD_WITNESS, verdicts=ALL_VERDICTS, make=None,
           report=GOOD_REPORT, report_b=None, fingerprints=None, drop_witness_b=False,
           omit_report_b=False, real_conformance=None, real_pdc=None) -> int:
    """Run the WHOLE gate against an injected repository state."""
    with tempfile.TemporaryDirectory() as d:
        tmp = Path(d)
        (tmp / "a.pd").write_text(GOOD_WITNESS)
        if not drop_witness_b:
            (tmp / "b.pd").write_text(witness_b)
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
            reports["b.pd"] = report if report_b is None else report_b
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
                      witnesses=("a.pd", "b.pd"), verdicts_text=verdicts,
                      make_results=GOOD_MAKE if make is None else make,
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
    fails = cases = 0

    def case(name, got, want):
        nonlocal fails, cases
        cases += 1
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
         _drive(verdicts=ALL_VERDICTS.replace("f/ref.pd PASS_VERIFIED",
                                              "f/ref.pd OUTPUT_MISMATCH")), 1)
    case("a conformance run with no parsable verdicts is exit 2, not a verdict",
         _drive(verdicts="nothing parsable here"), 2)

    print("\n  conditions 2 and 3 — verdicts come from the harness that RUNS things")
    case("a reject twin the compiler ACCEPTED goes RED",
         _drive(verdicts=ALL_VERDICTS.replace("r/unproven.pd REJECTED",
                                              "r/unproven.pd REJECT_ACCEPTED")), 1)
    case("a fixture whose stdout differs goes RED",
         _drive(verdicts=ALL_VERDICTS.replace("f/total.pd PASS_VERIFIED",
                                              "f/total.pd OUTPUT_MISMATCH")), 1)
    case("a DECLARED, ABSENT fixture goes RED — silence is not a pass",
         _drive(verdicts=ALL_VERDICTS.replace("f/region.pd PASS_VERIFIED", "")), 1)
    case("REJECTED for the WRONG reason goes RED (incidental unsupported syntax)",
         _drive(fingerprints={"r/async_fn.pd": "Unsupported type in reference parameter"}), 1)
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
         _drive(witness_b=GOOD_WITNESS.replace("x: ref String", "x: ref<'a> String")), 0)
    case("no `ref` PARAMETER (a struct field only) goes RED",
         _drive(witness_b="struct S { x: ref String }\n" + GOOD_WITNESS.replace(
             "fn drive(x: ref String, mut c: C)", "fn drive(mut c: C)").replace(
             "drive(s, c)", "drive(c)")), 1)
    # These two keep the REST of witness 2 green, so the only thing that can turn the run
    # red is the property under test. An earlier draft mutated the witness so heavily that
    # TH-05 failed too, and the cases passed for the wrong reason.
    case("`#[total]` reachable only from a DEAD fn goes RED",
         _drive(witness_b=GOOD_WITNESS.replace("return depth(1);", "return 1;")
                                      + "fn dead() -> i64 { return depth(2); }\n"), 1)
    case("`#[total]` on a self-recursive-only fn goes RED",
         _drive(witness_b=GOOD_WITNESS.replace("return depth(1);", "return 1;")
                                      .replace("fn depth(n: i64) -> i64 { return n; }",
                                               "fn depth(n: i64) -> i64 { return depth(n); }")), 1)
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
    case("conformance that prints verdicts and then FAILS is exit 2, not a verdict",
         _drive(real_conformance="#!/bin/sh\necho 'f/ref.pd PASS_VERIFIED'\nexit 3\n"), 2)
    case("conformance that prints verdicts and is then KILLED is exit 2",
         _drive(real_conformance="#!/bin/sh\necho 'f/ref.pd PASS_VERIFIED'\nkill -9 $$\n"), 2)
    case("a pdc that prints an effect line and then FAILS is exit 2",
         _drive(real_pdc="#!/bin/sh\necho \"Function 'header' has effects: [Io]\"\nexit 1\n"), 2)
    case("a pdc that prints an effect line and is then KILLED is exit 2",
         _drive(real_pdc="#!/bin/sh\necho \"Function 'header' has effects: [Io]\"\nkill -9 $$\n"), 2)

    print("\n  the row set is CLOSED")
    case("ADDING a thesis row is a harness error (exit 2)",
         _drive(rows=_rows(extra="ZZ-99\tM9\tsrc\tsneaked in\tfixture\tx.pd\towed\tthesis\t-\n")), 2)
    case("REMOVING a thesis row is a harness error", _drive(rows=_rows(drop="N9-06")), 2)
    case("RETYPING a row out of dispatch is a harness error, not a silent skip",
         _drive(rows=_rows(retype=("N8-08", "observable", "t.rs::x"))), 2)
    case("an unknown evidence kind is a harness error",
         _drive(rows=_rows(retype=("N8-08", "vibes", "x"))), 2)
    case("a duplicate id is a harness error",
         _drive(rows=_rows(extra="N9-06\tM7\tsrc\tdup\treject\tx.pd\towed\tthesis\t-\n")), 2)
    case("a short row is a harness error",
         _drive(rows=_rows(extra="BAD\tM9\tsrc\tonly six\tgate\tx\n")), 2)

    print("\n  the lexer")
    case("a char literal '<' is not a lifetime",
         p_no_lifetime_param_list(strip_literals("fn f() { let x = '<'; }"))[0], True)
    case("block comments do NOT nest, matching bootstrap/pdc.pd:164-175 (flips with N2-08)",
         "async" in strip_literals("/* a /* b */ async fn f() {} */"), True)
    case("an unterminated comment does not crash the lexer",
         isinstance(strip_literals("/* unterminated"), str), True)

    print("\n  the typed process boundary (scripts/gate_probe.py)")
    killed = GP.classify(GP.run(["sh", "-c", "echo 'error: buffered' >&2; kill -9 $$"]))
    case("a killed producer yields Malfunction with NO readable text",
         hasattr(killed, "text"), False)
    case("a concluded producer yields readable text",
         hasattr(GP.classify(GP.run(["sh", "-c", "echo fine"])), "text"), True)

    print("\n  NO NEGATIVE CONTROL — pinned, so emptying this list cannot pass silently")
    if not EXPECTED_UNCOVERED:
        case("the uncovered set is pinned and non-empty", False, True)
    for u in sorted(EXPECTED_UNCOVERED):
        print(f"  {GREY}--   {u}{OFF}")

    print("=" * 78)
    if fails == 0:
        print(f"  self-test green — {cases} cases, most driving main() end to end")
        print(f"  {len(EXPECTED_UNCOVERED)} probe group(s) pinned as uncovered, listed above")
        print("=" * 78)
        return 0
    print(f"  self-test RED — {fails} of {cases} cases failed")
    print("=" * 78)
    return 1


if __name__ == "__main__":
    try:
        sys.exit(self_test() if "--self-test" in sys.argv else main())
    except HarnessError as e:
        print(f"{RED}harness error{OFF}: {e}", file=sys.stderr)
        print("This is a failure to MEASURE, not a verdict about the language.", file=sys.stderr)
        sys.exit(2)
