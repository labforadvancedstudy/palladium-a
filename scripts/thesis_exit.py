#!/usr/bin/env python3
"""THE DEFINITION OF PALLADIUM 1.0, AS A COMMAND.

1.0 is not "the inventory has no unmet rows". That is a completeness criterion, and
completeness criteria are the generator of every fiction this repository has retracted:
`progress: 85%`, "Bootstrap 100% Complete", "Self-Hosting 100%", "v0.6: Self-hosting
achieved". 1.0 is the thesis, proven on the one artifact here that structurally cannot
lie:

    bootstrap/pdc.pd, rewritten in the differentiated dialect, still reaching a
    byte-identical stage1/stage2 fixed point, plus a second witness program.

A conformance fixture can print "not yet implemented" and PASS. A compiler cannot
compile itself vacuously.

WHAT THIS GATE DOES NOT DO, AND WHY THAT MATTERS
------------------------------------------------
The first version of this gate checked that the conformance *manifest text* said `run`
or `reject`. It ran nothing. A reject twin the compiler happily accepted, a fixture that
did not exist, a rejection for an unrelated reason — all reported green. That is the
defect this whole plan is organised against, committed inside the command that defines
done.

So this gate does not validate text. It **delegates to `scripts/conformance.sh`**, which
already compiles, links, runs, diffs stdout against a recorded transcript, checks the
declared failure *stage*, matches the declared *diagnostic fingerprint*, reports
REJECT_ACCEPTED when a negative test is accepted, and reports MISSING when a declared
fixture is not on disk. Every one of those was needed here and every one already existed.

The rows it checks are read from docs/contributing/1.0-requirements.tsv — the rows whose
`disposition` is `thesis`. There is exactly one definition of 1.0 and it is that file;
this command executes it. Hard-coding the list here would give the repository two
definitions and check one, which is how `progress: 85%` happened.

Usage
-----
    scripts/thesis-exit.sh                exit 0 only when 1.0 is real
    scripts/thesis-exit.sh --self-test    fault-inject every probe; prove it can go RED
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REQUIREMENTS = ROOT / "docs/contributing/1.0-requirements.tsv"
CONFORMANCE = ROOT / "scripts/conformance.sh"
PDC = ROOT / "target/release/pdc"

# The two programs the thesis is proven on. A second *filename* is not independence:
# every source probe below runs over BOTH, and each must also pass conformance as an
# executed fixture, so neither can satisfy the thesis with decoration.
WITNESSES = ("bootstrap/pdc.pd", "tests/witness/json_parser.pd")

# N14's effectful set. `string_*`, `char_*` and `int_to_string` are pure and are
# deliberately absent: a caller of those is not evidence of an IO effect.
IO_BUILTINS = {
    "print", "print_int", "panic",
    "file_open", "file_read_all", "file_read_line", "file_write", "file_close",
    "file_exists", "file_flush", "file_seek",
    "file_open_ex", "file_close_ex", "file_read_ex", "file_write_ex",
    "path_exists", "path_is_file", "path_is_dir",
    "create_dir", "create_dir_all", "remove_file", "remove_dir", "remove_dir_all",
    "read_file_to_string", "write_string_to_file", "arg_count", "arg_at",
}


class HarnessError(Exception):
    """The gate could not evaluate something. NOT a verdict about the language.

    An unreadable file used to make the scanner yield the empty string, and TH-01/TH-02
    then reported GREEN — a missing measurement read as a passing one. That is the
    `total=0, exit 0` failure `conformance.sh` already fixed once.
    """


# ---------------------------------------------------------------------------
# Lexing. Deliberately models THE COMPILER, not the specification.
# ---------------------------------------------------------------------------
CHAR_LITERAL = re.compile(r"'(?:\\.|[^\\'])'")


def strip_literals(text: str) -> str:
    """Blank string and char literals and comments; KEEP lifetime ticks.

    Two decisions, both of which a naive scanner gets wrong and both of which would make
    this gate lie:

    1. `'` is ambiguous — it opens a char literal AND introduces a lifetime. Treating
       every `'` as a quote consumes from the tick to end of file, and the lifetime probe
       can then never fire. A char literal is `'x'` or `'\\x'` and nothing else; anything
       else starting with `'` is a tick and is kept.

    2. Block comments DO NOT NEST here, because `bootstrap/pdc.pd:164-175` shows the
       compiler scanning for the first `*/` and breaking, with no depth counter. The
       specification (N2) requires nesting and the compiler does not implement it — that
       is requirement N2-08, owned by M2. Until it lands, a gate that nested would
       disagree with the compiler about whether a real `async` is commented out. When
       N2-08 lands, this function and the compiler flip together, and the self-test's
       `nesting is NOT supported` case is what forces them to flip in lockstep.
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
            j = text.find("*/", i + 2)          # first close wins: see the docstring
            i = n if j < 0 else j + 2
            out.append(" ")
        else:
            out.append(c)
            i += 1
    return "".join(out)


def read_source(rel: str) -> str:
    p = ROOT / rel
    if not p.is_file():
        raise HarnessError(f"{rel}: not a file")
    if not os.access(p, os.R_OK):
        raise HarnessError(f"{rel}: unreadable")
    raw = p.read_text(encoding="utf-8", errors="replace")
    if not raw.strip():
        raise HarnessError(f"{rel}: empty")
    src = strip_literals(raw)
    if not src.strip():
        raise HarnessError(f"{rel}: nothing survives lexing — refusing to call that clean")
    return src


# ---------------------------------------------------------------------------
# Source probes. Each is a pure function of text so the self-test can inject.
# Each returns (ok, detail).
# ---------------------------------------------------------------------------
ASYNC_TOKEN = re.compile(r"(?:^|[^A-Za-z_0-9])(async|await)(?:[^A-Za-z_0-9]|$)")

# `ref<'a> T` is the ONE place N9 permits a region name, so it is exempt — but the
# exemption needs an identifier boundary. Without one, `s/ref<'a>/ref/` also rewrote
# `myref<'a>` to `my`, deleting the very lifetime list the probe exists to find.
REF_REGION = re.compile(r"(?<![A-Za-z_0-9])ref<'[A-Za-z_0-9]*>")
LIFETIME_LIST = re.compile(r"<'")

# A `ref` PARAMETER, not any `: ref` anywhere. A struct field or a local annotation is
# not evidence that the compiler is written against references.
FN_HEADER = re.compile(r"(?<![A-Za-z_0-9])fn\s+([A-Za-z_][A-Za-z_0-9]*)\s*\(")
REF_PARAM = re.compile(r":\s*ref(?:\s+mut)?\s+[A-Za-z_\[(]")

# `#[total]` / `#[total(...)]` as an attribute token attached to a `fn`, not the string
# "#[total" occurring anywhere.
TOTAL_ON_FN = re.compile(
    r"#!?\[\s*total\s*(?:\([^)]*\))?\s*\]\s*(?:#!?\[[^\]]*\]\s*)*(?:pub\s+)?fn\s+([A-Za-z_][A-Za-z_0-9]*)"
)
EFFECT_LINE = re.compile(r"Function '([A-Za-z_][A-Za-z_0-9]*)' has effects:\s*(.+)")


def balanced_span(src: str, open_at: int, opener: str = "(", closer: str = ")") -> str:
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
    """name -> body text, by brace matching from each `fn` header."""
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


def p_no_async_token(src: str) -> tuple[bool, str]:
    m = ASYNC_TOKEN.search(src)
    return (False, f"found `{m.group(1)}`") if m else (True, "no async/await token")


def p_no_lifetime_param_list(src: str) -> tuple[bool, str]:
    m = LIFETIME_LIST.search(REF_REGION.sub("ref", src))
    return (False, "a `<'` lifetime parameter list survives") if m else (True, "none")


def p_has_ref_param(src: str) -> tuple[bool, str]:
    for m in FN_HEADER.finditer(src):
        params = balanced_span(src, m.end() - 1)
        if REF_PARAM.search(params):
            return True, f"fn {m.group(1)}"
    return False, "no `fn` declares a `ref` / `ref mut` PARAMETER"


def p_total_on_fn(src: str) -> tuple[bool, str]:
    names = [m.group(1) for m in TOTAL_ON_FN.finditer(src)]
    if not names:
        return False, "no `#[total]` attached to a `fn`"
    bodies = function_bodies(src)
    called = {c for b in bodies.values() for c in re.findall(r"([A-Za-z_][A-Za-z_0-9]*)\s*\(", b)}
    live = [n for n in names if n in called or n == "main"]
    if not live:
        return False, f"`#[total]` only on unused function(s): {', '.join(names)}"
    return True, f"#[total] on {', '.join(live)}, and called"


def p_effect_is_transitive(report: str, src: str) -> tuple[bool, str]:
    """An IO effect must reach a caller that performs NO IO itself.

    The previous probe matched any line containing `has effects` and an IO spelling.
    `bootstrap/pdc.pd:49-51` calls `file_write` directly, so it passed on a DIRECT
    effect — while the claim under test is propagation. Naming neither callee nor caller,
    it could not tell the two apart, which is the entire point of N7-04/N7-05.
    """
    bodies = function_bodies(src)
    reported = {}
    for line in report.splitlines():
        m = EFFECT_LINE.search(line)
        if m and re.search(r"\bIo\b|\bIO\b|\bio\b", m.group(2)):
            reported[m.group(1)] = m.group(2).strip()
    if not reported:
        return False, "the compiler reported no function with an IO effect"

    def direct_io(name: str) -> set[str]:
        body = bodies.get(name, "")
        return {c for c in re.findall(r"([A-Za-z_][A-Za-z_0-9]*)\s*\(", body) if c in IO_BUILTINS}

    for caller, effects in sorted(reported.items()):
        if direct_io(caller):
            continue                              # a DIRECT effect proves nothing here
        callees = re.findall(r"([A-Za-z_][A-Za-z_0-9]*)\s*\(", bodies.get(caller, ""))
        for callee in callees:
            io = direct_io(callee)
            if io:
                return True, (f"`{caller}` performs no IO itself, calls `{callee}` "
                              f"(which calls {sorted(io)[0]}), and is reported {effects}")
        if caller in bodies:
            return True, f"`{caller}` performs no IO itself and is reported {effects}"
    direct = ", ".join(sorted(reported))
    return False, (f"every function reported with an IO effect performs IO directly "
                   f"({direct}) — that is not propagation")


# ---------------------------------------------------------------------------
# Conformance delegation. The verdicts come from the harness that RUNS things.
# ---------------------------------------------------------------------------
VERDICT_LINE = re.compile(r"^(\S+\.pd)\s+([A-Z_]+)\s*$")
REQUIRED_VERDICT = {"fixture": "PASS_VERIFIED", "reject": "REJECTED", "skip": "SKIP"}


def run_conformance() -> tuple[dict[str, str], str]:
    if os.environ.get("THESIS_VERDICTS"):        # self-test injection point
        text = Path(os.environ["THESIS_VERDICTS"]).read_text(encoding="utf-8")
    else:
        if not PDC.is_file():
            raise HarnessError("target/release/pdc is not built")
        proc = subprocess.run(["bash", str(CONFORMANCE), "tests", "examples"],
                              capture_output=True, text=True, cwd=ROOT)
        text = proc.stdout + proc.stderr
    verdicts = {}
    for line in text.splitlines():
        m = VERDICT_LINE.match(line.strip())
        if m:
            verdicts[m.group(1)] = m.group(2)
    if not verdicts:
        raise HarnessError("scripts/conformance.sh produced no verdict lines")
    return verdicts, text


def p_verdict(verdicts: dict[str, str], path: str, kind: str) -> tuple[bool, str]:
    want = REQUIRED_VERDICT[kind]
    got = verdicts.get(path)
    if got is None:
        return False, f"DECLARED, ABSENT — no conformance row ran for {path} (want {want})"
    if got != want:
        return False, f"{path} is {got}, want {want}"
    return True, f"{path} {got}"


def p_make_target(target: str) -> tuple[bool, str]:
    mk = (ROOT / "Makefile").read_text(encoding="utf-8")
    if not re.search(rf"^{re.escape(target)}:", mk, re.M):
        return False, f"DECLARED, ABSENT — no `{target}` target exists"
    r = subprocess.run(["make", "-s", target], capture_output=True, text=True, cwd=ROOT)
    return (r.returncode == 0), f"make {target} exit {r.returncode}"


# ---------------------------------------------------------------------------
# The manifest is the definition. This command executes it.
# ---------------------------------------------------------------------------
def thesis_rows() -> list[dict]:
    rows = []
    for line in REQUIREMENTS.read_text(encoding="utf-8").splitlines():
        if not line or line.startswith("#"):
            continue
        f = line.split("\t")
        if len(f) != 8:
            raise HarnessError(f"{REQUIREMENTS.name}: row with {len(f)} columns, want 8")
        if f[7] == "thesis":
            rows.append(dict(id=f[0], milestone=f[1], req=f[3], kind=f[4], ev=f[5]))
    if not rows:
        raise HarnessError("no rows with disposition `thesis` — the definition is empty")
    return rows


GREEN, RED, GREY = "\033[0;32m", "\033[0;31m", "\033[0;90m"
OFF = "\033[0m"


def main() -> int:
    rows = thesis_rows()
    results: list[tuple[str, bool, str, str, str]] = []   # id, ok, owner, detail, group

    def record(rid, ok, owner, detail, group):
        results.append((rid, ok, owner, detail, group))

    print("=" * 78)
    print("  make thesis-exit — the definition of Palladium 1.0")
    print(f"  {len(rows)} `thesis` rows, read from {REQUIREMENTS.relative_to(ROOT)}")
    print("=" * 78)

    # -- conformance-backed rows: executed, transcript-diffed, fingerprint-matched ----
    try:
        verdicts, _ = run_conformance()
        conf_err = None
    except HarnessError as e:
        verdicts, conf_err = {}, str(e)

    for r in rows:
        if r["kind"] not in REQUIRED_VERDICT:
            continue
        g = "C3" if r["kind"] == "reject" else "C2"
        if conf_err:
            record(r["id"], False, r["milestone"], f"HARNESS: {conf_err}", g)
        else:
            ok, detail = p_verdict(verdicts, r["ev"], r["kind"])
            record(r["id"], ok, r["milestone"], detail, g)

    # -- the source probes, over EVERY witness ---------------------------------------
    probe_ids = {"TH-01": p_no_async_token, "TH-02": p_no_lifetime_param_list,
                 "TH-03": p_has_ref_param, "TH-04": p_total_on_fn}
    for rid, fn in probe_ids.items():
        row = next((r for r in rows if r["id"] == rid), None)
        if row is None:
            continue
        oks, details = [], []
        for w in WITNESSES:
            try:
                ok, d = fn(read_source(w))
            except HarnessError as e:
                ok, d = False, f"HARNESS: {e}"
            oks.append(ok)
            details.append(f"{w}: {d}")
        record(rid, all(oks), row["milestone"], " · ".join(details), "C1")

    row = next((r for r in rows if r["id"] == "TH-05"), None)
    if row is not None:
        oks, details = [], []
        for w in WITNESSES:
            try:
                src = read_source(w)
                proc = subprocess.run([str(PDC), "compile", w, "-o", os.devnull],
                                      capture_output=True, text=True, cwd=ROOT)
                ok, d = p_effect_is_transitive(proc.stdout + proc.stderr, src)
            except (HarnessError, OSError) as e:
                ok, d = False, f"HARNESS: {e}"
            oks.append(ok)
            details.append(f"{w}: {d}")
        record("TH-05", all(oks), row["milestone"], " · ".join(details), "C1")

    row = next((r for r in rows if r["id"] == "TH-06"), None)
    if row is not None:
        # Condition 4 is INDEPENDENCE, not a second filename: witness 2 must itself run
        # as a verified fixture AND carry all three differentiators.
        w2 = WITNESSES[1]
        ok2, d2 = (p_verdict(verdicts, w2, "fixture") if not conf_err
                   else (False, f"HARNESS: {conf_err}"))
        sub = [(f"runs", ok2, d2)]
        for name, fn in (("ref param", p_has_ref_param), ("#[total]", p_total_on_fn),
                         ("no async", p_no_async_token),
                         ("no 'a list", p_no_lifetime_param_list)):
            try:
                ok, d = fn(read_source(w2))
            except HarnessError as e:
                ok, d = False, f"HARNESS: {e}"
            sub.append((name, ok, d))
        record("TH-06", all(s[1] for s in sub), row["milestone"],
               "; ".join(f"{n}: {d}" for n, ok, d in sub if not ok) or "witness 2 is independent",
               "C4")

    # -- gate rows --------------------------------------------------------------------
    seen_targets: dict[str, tuple[bool, str]] = {}
    for r in rows:
        if r["kind"] != "gate" or r["id"] in probe_ids or r["id"] in ("TH-05", "TH-06"):
            continue
        target = r["ev"].replace("make ", "").strip()
        if target == "thesis-exit":
            record(r["id"], False, r["milestone"],
                   "this command — 1.0 is reached when every other row is green", "C0")
            continue
        if target not in seen_targets:
            seen_targets[target] = p_make_target(target)
        ok, detail = seen_targets[target]
        record(r["id"], ok, r["milestone"], detail, "C1")

    GROUPS = [
        ("C0", "The definition itself"),
        ("C1", "Condition 1 — the fixed point, and every witness written in the dialect"),
        ("C2", "Condition 2 — one non-vacuous fixture per differentiator, RUN by "
               "scripts/conformance.sh"),
        ("C3", "Condition 3 — the reject twin per differentiator. FOR AN INFERENCE FEATURE "
               "THE REJECTION IS THE PRODUCT"),
        ("C4", "Condition 4 — a second witness, so one program's shape is not the language"),
    ]
    by_id = {r["id"]: r for r in rows}
    for key, title in GROUPS:
        group = sorted(r for r in results if r[4] == key)
        if not group:
            continue
        print(f"\n{title}")
        for rid, ok, owner, detail, _ in group:
            mark = f"{GREEN}ok  {OFF}" if ok else f"{RED}RED {OFF}"
            req = by_id[rid]["req"][:52]
            print(f"  {mark} {rid:<7} {req:<54} {'' if ok else 'owed by ' + owner}")
            print(f"        {GREY}{detail}{OFF}")

    red = [r for r in results if not r[1]]
    print()
    print("=" * 78)
    print(f"  thesis: {len(results) - len(red)} green, {RED}{len(red)} RED{OFF}"
          f"   ({len(rows)} rows in the definition)")
    if not red:
        print("  Palladium 1.0: the thesis holds.")
        print("=" * 78)
        return 0
    print("  1.0 is NOT reached. Every RED line names the milestone that owes it.")
    print("  Committed red on purpose. Do not make it pass by weakening it: dropping the")
    print("  reject twins would let a no-op inferencer look finished.")
    print("=" * 78)
    return 1


# ---------------------------------------------------------------------------
# --self-test: FAULT INJECTION, not a call to the helpers.
# ---------------------------------------------------------------------------
def self_test() -> int:
    """For every probe: a state that violates the property must go RED, and a state that
    satisfies it must go green. A probe with no negative control is NAMED as uncovered
    rather than left silent — the previous self-test exercised two probes, called them
    directly rather than through the gate, and the Makefile advertised it as proof that
    "the thesis gate's source probes can still go RED". Six probes had no control at all.
    """
    fails, cases = 0, 0
    uncovered: list[str] = []

    def case(name, got, want):
        nonlocal fails, cases
        cases += 1
        if got == want:
            print(f"  {GREEN}ok  {OFF} {name}")
        else:
            print(f"  {RED}FAIL{OFF} {name} (got {got}, want {want})")
            fails += 1

    print("thesis-exit self-test — fault injection, one negative control per probe")

    print("\n  lexer")
    case("a char literal '<' is not a lifetime",
         p_no_lifetime_param_list(strip_literals("fn f() { let x = '<'; }"))[0], True)
    case("block comments do NOT nest, matching bootstrap/pdc.pd:164-175 (flips with N2-08)",
         "async" in strip_literals("/* a /* b */ async fn f() {} */"), True)
    case("a comment really is stripped",
         p_no_async_token(strip_literals("fn f() {} // async"))[0], True)

    print("\n  TH-01  no async/await token")
    case("+ a clean source is green", p_no_async_token("fn f() { }")[0], True)
    case("- a real `async fn` goes RED", p_no_async_token("async fn f() { }")[0], False)
    case("- a real `.await` goes RED", p_no_async_token("fn f() { g().await; }")[0], False)

    print("\n  TH-02  no lifetime parameter list")
    case("+ ref<'a> T is PERMITTED by N9",
         p_no_lifetime_param_list("fn f(x: ref<'a> String) { }")[0], True)
    case("- fn f<'a>(…) goes RED",
         p_no_lifetime_param_list("fn f<'a>(x: ref String) { }")[0], False)
    case("- myref<'a>(…) goes RED — the exemption needs an identifier boundary",
         p_no_lifetime_param_list("fn myref<'a>(x: i64) { }")[0], False)
    case("- struct S<'a> goes RED", p_no_lifetime_param_list("struct S<'a> { }")[0], False)

    print("\n  TH-03  a ref PARAMETER, not any `: ref`")
    case("+ a ref parameter is green",
         p_has_ref_param("fn f(x: ref String) -> i64 { }")[0], True)
    case("+ a ref mut parameter is green",
         p_has_ref_param("fn f(a: i64, b: ref mut Buf) { }")[0], True)
    case("- a struct FIELD `: ref T` goes RED",
         p_has_ref_param("struct S { x: ref String }\nfn f(y: i64) { }")[0], False)
    case("- a LOCAL annotation `: ref T` goes RED",
         p_has_ref_param("fn f(y: i64) { let z: ref String = w; }")[0], False)

    print("\n  TH-04  a #[total] attached to a live fn")
    case("+ #[total] on a called fn is green",
         p_total_on_fn("#[total]\nfn h(n: i64) -> i64 { return n; }\nfn main() { h(1); }")[0], True)
    case("- #[total] on an UNUSED fn goes RED",
         p_total_on_fn("#[total]\nfn dead(n: i64) -> i64 { return n; }\nfn main() { }")[0], False)
    case("- the bare text `#[total` with no fn goes RED",
         p_total_on_fn('fn main() { print("#[total]"); h(); }')[0], False)
    case("- no attribute at all goes RED",
         p_total_on_fn("fn h() { }\nfn main() { h(); }")[0], False)

    print("\n  TH-05  the effect must be TRANSITIVE, not direct")
    direct = "fn emit(c: C, s: String) { file_write(c.out, s); }\nfn main() { emit(c, s); }"
    trans = ("fn emit(c: C, s: String) { file_write(c.out, s); }\n"
             "fn header(c: C) { emit(c, \"x\"); }\nfn main() { header(c); }")
    case("- only a DIRECT effect goes RED",
         p_effect_is_transitive("Function 'emit' has effects: [Io]", direct)[0], False)
    case("+ an effect on a caller that does no IO is green",
         p_effect_is_transitive("Function 'emit' has effects: [Io]\n"
                                "Function 'header' has effects: [Io]", trans)[0], True)
    case("- no IO reported at all goes RED",
         p_effect_is_transitive("Function 'pure_thing' has effects: [Memory]", trans)[0], False)

    print("\n  conformance delegation  (conditions 2 and 3)")
    v = {"a.pd": "PASS_VERIFIED", "b.pd": "REJECT_ACCEPTED", "c.pd": "REJECTED",
         "d.pd": "OUTPUT_MISMATCH"}
    case("+ an executed, transcript-matched fixture is green",
         p_verdict(v, "a.pd", "fixture")[0], True)
    case("- a reject twin the compiler ACCEPTED goes RED",
         p_verdict(v, "b.pd", "reject")[0], False)
    case("+ a genuinely refused reject twin is green", p_verdict(v, "c.pd", "reject")[0], True)
    case("- a fixture whose stdout differs goes RED", p_verdict(v, "d.pd", "fixture")[0], False)
    case("- a DECLARED, ABSENT fixture goes RED — silence is not a pass",
         p_verdict(v, "nope.pd", "fixture")[0], False)

    print("\n  harness errors are not verdicts")
    for bad, why in ((ROOT / "no/such/file.pd", "missing"), (ROOT, "a directory")):
        try:
            read_source(str(bad.relative_to(ROOT)) if bad != ROOT else ".")
            case(f"- an unreadable source ({why}) raises", False, True)
        except HarnessError:
            case(f"- an unreadable source ({why}) raises HarnessError, not GREEN", True, True)
    try:
        strip_literals("/* unterminated")
        case("+ an unterminated comment does not crash the lexer", True, True)
    except Exception:
        case("+ an unterminated comment does not crash the lexer", False, True)

    print("\n  NO NEGATIVE CONTROL — named, not left silent")
    uncovered = [
        "SH-01/SH-02..04 (`make selfhost`, `make selfhost-corpus`): the control would be a "
        "deliberately broken build; the target-absent path IS covered by p_make_target",
        "D1-01: names this command; it is reported RED by construction until every other "
        "row is green, so there is nothing to invert",
    ]
    for u in uncovered:
        print(f"  {GREY}--   {u}{OFF}")

    print("=" * 78)
    covered = "every probe that reads source or a verdict has both a positive and a negative case"
    if fails == 0:
        print(f"  self-test green — {cases} cases; {covered}")
        print(f"  {len(uncovered)} probe group(s) explicitly uncovered, listed above")
        print("=" * 78)
        return 0
    print(f"  self-test RED — {fails} of {cases} cases failed")
    print("=" * 78)
    return 1


if __name__ == "__main__":
    try:
        if "--self-test" in sys.argv:
            sys.exit(self_test())
        sys.exit(main())
    except HarnessError as e:
        print(f"{RED}harness error{OFF}: {e}", file=sys.stderr)
        print("This is a failure to MEASURE, not a verdict about the language.",
              file=sys.stderr)
        sys.exit(2)
