#!/usr/bin/env python3
"""Typed-result boundary for every process this gate treats as evidence.

WHY THIS FILE EXISTS
--------------------
Four review rounds found the same defect class in four different places, twice
inside the machinery built to close it: a process failed, the shell read its
output, and a semantic verdict was issued without anyone first establishing that
the process had actually finished its experiment.

    THE RULE, AND IT IS THE WHOLE POINT OF THIS MODULE:
    diagnostic text is never sufficient evidence of a verdict.
    The EXIT CODE says whether the experiment ran.
    The TEXT only says what it found, and may be read only afterwards.

Text can be buffered by a process that then dies. Measured:

    $ sh -c 'echo "error: No main function found" >&2; kill -9 $$'
    exit 137, and the expected diagnostic is sitting on stderr

Under the old shell code that was classified `ACCEPTED_NO_MAIN` — a green
verdict from a process that was killed. The same shape was reachable in the
forced-import probe, in the UNUSABLE probes, and in both generated-C nets.

So every producer — `pdc`, the Python analysis, and the C compiler alike — is
run through `run()` here, which returns one of exactly three outcomes:

    SUCCESS      exit 0
    REJECTED     exit is one of the producer's EMPIRICALLY PINNED reject codes
    MALFUNCTION  anything else: a signal, or a code the producer is not known
                 to use for rejection

Sub-classification by text happens only inside REJECTED, and it cannot be
written any other way, because `Run.text` is not reachable without going through
`classify()` first.

EMPIRICALLY PINNED EXIT CODES (measured on this tree, 2026-08-22)
    pdc  0 success; 1 for EVERY rejection — parse error, no-main, gcc failure,
         and a missing input file all exit 1, which is exactly why the text
         sub-classification below is still needed *within* a proven rejection.
    cc   0 success; 1 when it reports errors. >128 is a signal.

EXIT CODES OF THIS TOOL (uniform across every subcommand, so the caller has
exactly one decision to make):
    0  the experiment ran and reached a normal conclusion; details on stdout
    1  the experiment ran and produced a reportable FINDING
    2  MALFUNCTION — nothing was established, and no verdict may be inferred
"""

from __future__ import annotations

import argparse
import importlib.util
import os
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

EXIT_OK = 0
EXIT_FINDING = 1
EXIT_MALFUNCTION = 2

SUCCESS = "success"
REJECTED = "rejected"
MALFUNCTION = "malfunction"

HERE = Path(__file__).resolve().parent


@dataclass
class Run:
    """A finished process. `text` is deliberately private until classified."""

    argv: list
    rc: int
    _text: str

    @property
    def signal(self):
        """Signal number if the process was killed, else None.

        TWO CONVENTIONS, and getting this wrong is how a signal check silently
        never fires: Python's subprocess reports a signal-killed child as
        NEGATIVE (-9 for SIGKILL), while a POSIX shell reports 128+signum (137).
        This module sees the former; the shell callers see the latter. Both are
        recognised so the same rule holds on either side of the boundary.
        """
        if self.rc < 0:
            return -self.rc
        if self.rc > 128:
            return self.rc - 128
        return None

    @property
    def signaled(self) -> bool:
        return self.signal is not None

    def describe(self) -> str:
        if self.signaled:
            return f"killed by signal {self.signal}"
        return f"exit {self.rc}"


def run(argv, cwd=None, env=None) -> Run:
    """Run a process, capturing stdout and stderr together."""
    merged = dict(os.environ)
    if env:
        merged.update(env)
    try:
        proc = subprocess.run(
            argv,
            cwd=cwd,
            env=merged,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            timeout=300,
        )
    except FileNotFoundError as exc:
        return Run(list(argv), 127, f"HARNESS: cannot execute: {exc}")
    except subprocess.TimeoutExpired:
        return Run(list(argv), 124, "HARNESS: timed out after 300s")
    return Run(list(argv), proc.returncode, proc.stdout.decode("utf-8", "replace"))


def classify(r: Run, reject_codes=(1,)):
    """(outcome, text_or_None). Text is None unless the experiment concluded.

    Returning None for the text on MALFUNCTION is the enforcement: a caller
    physically cannot grep the output of a process that did not finish.
    """
    if r.rc == 0:
        return SUCCESS, r._text
    if r.rc in reject_codes and not r.signaled:
        return REJECTED, r._text
    return MALFUNCTION, None


ANSI = re.compile(r"\033\[[0-9;]*m")


def strip_ansi(s: str) -> str:
    return ANSI.sub("", s)


def error_lines(text: str):
    return [ln.strip() for ln in strip_ansi(text).splitlines() if "error" in ln]


def emit(**fields):
    for k, v in fields.items():
        print(f"{k.upper()} {v}")


def malfunction(what: str, r: Run, extra: str = "") -> int:
    emit(outcome=MALFUNCTION, reason=f"{what} {r.describe()}")
    body = strip_ansi(r._text).strip()
    if body:
        for ln in body.splitlines()[:6]:
            print(f"HARNESS   {ln}")
    if extra:
        print(f"HARNESS   {extra}")
    return EXIT_MALFUNCTION


LOC_RE = re.compile(r"--> ([^ :]+):(\d+):(\d+)")


def classify_blocker(path: str, text: str) -> str:
    """WHY a file was rejected, as a stable CATEGORY rather than a wording.

    Runs only on text from a PROVEN rejection. Categories, not messages, are what
    stdlib/MANIFEST.tsv pins, so rephrasing a diagnostic does not fail the gate
    but failing for a genuinely different reason does.
    """
    plain = strip_ansi(text)
    first = next(iter(error_lines(text)), "")
    src = ""
    m = LOC_RE.search(plain)
    if m:
        try:
            lines = Path(path).read_text(errors="replace").splitlines()
            n = int(m.group(2))
            if 1 <= n <= len(lines):
                src = lines[n - 1]
        except OSError:
            src = ""
    if "Unexpected character '#'" in first:
        return "ATTRIBUTE"
    if "Unexpected character '\\'" in first:
        return "CHAR_ESCAPE"
    if re.search(r"[0-9]+\.[0-9]+", src):
        return "FLOAT_LITERAL"
    if re.match(r"^\s*(pub\s+)?use\s", src):
        return "USE_DECL"
    if re.match(r"^\s*(pub\s+)?mod\s", src):
        return "MOD_DECL"
    if re.match(r"^\s*pub\s+fn", src):
        return "PUB_FN_IN_IMPL"
    if "found 'type'" in first:
        return "ASSOC_TYPE"
    if re.search(r"<[A-Za-z_]+\s*=", src):
        return "GENERIC_DEFAULT"
    if "Expected '=' after variable name" in first:
        return "UNINIT_LET"
    return "OTHER"


# ---------------------------------------------------------------------------
# pdc-verdict — Phase 1 classification of a stdlib file
# ---------------------------------------------------------------------------
def cmd_pdc_verdict(args) -> int:
    r = run([args.pdc, "compile", args.file, "-o", args.out])
    outcome, text = classify(r)
    if outcome is MALFUNCTION or outcome == MALFUNCTION:
        return malfunction(f"pdc on {args.file}", r)
    if outcome == SUCCESS:
        emit(outcome=SUCCESS, verdict="COMPILE_OK")
        print("BLOCKER -")
        return EXIT_OK

    errs = error_lines(text)
    # ACCEPTED_NO_MAIN means the language accepted the file and ONLY the
    # harness's entry-point requirement stands in the way. It therefore
    # requires the no-main diagnostic to be the sole distinct error; a file that
    # also fails to parse is not "accepted".
    no_main = [e for e in errs if "No main function found" in e]
    others = [e for e in errs if "No main function found" not in e]
    if no_main and not others:
        verdict = "ACCEPTED_NO_MAIN"
    elif "gcc compilation failed" in strip_ansi(text):
        verdict = "LINK_FAIL"
    else:
        verdict = "COMPILE_FAIL"
    emit(outcome=REJECTED, verdict=verdict)
    if verdict != "ACCEPTED_NO_MAIN":
        print(f"BLOCKER {classify_blocker(args.file, text)}")
    else:
        print("BLOCKER -")
    first = next((e for e in errs), "")
    if first:
        print(f"DIAG {first}")
    return EXIT_OK


# ---------------------------------------------------------------------------
# pdc-reject — "this must be refused, at this stage, with this diagnostic"
# Used by the forced-import probe and by every UNUSABLE builtin probe.
# ---------------------------------------------------------------------------
def cmd_pdc_reject(args) -> int:
    env = {}
    if args.env:
        for pair in args.env:
            k, _, v = pair.partition("=")
            env[k] = v
    r = run([args.pdc, "compile", args.file, "-o", args.out], cwd=args.cwd, env=env)
    outcome, text = classify(r)
    if outcome == MALFUNCTION:
        return malfunction(f"pdc on {args.file}", r)
    if outcome == SUCCESS:
        emit(outcome="accepted")
        return EXIT_OK

    stage = "link" if "gcc compilation failed" in strip_ansi(text) else "compile"
    plain = strip_ansi(text)
    if args.expect_stage and stage != args.expect_stage:
        emit(outcome="rejected-other", stage=stage,
             reason=f"expected rejection at {args.expect_stage}, got {stage}")
        first = next(iter(error_lines(text)), "")
        if first:
            print(f"DIAG {first}")
        return EXIT_OK
    if args.require and args.require not in plain:
        emit(outcome="rejected-other", stage=stage,
             reason="rejected at the expected stage but not with the expected diagnostic")
        first = next(iter(error_lines(text)), "")
        if first:
            print(f"DIAG {first}")
        return EXIT_OK
    emit(outcome="rejected-as-expected", stage=stage)
    return EXIT_OK


# ---------------------------------------------------------------------------
# generated-c — Net A (structural) + Net B (the C compiler)
# ---------------------------------------------------------------------------
def _load_net_a():
    path = HERE / "check-c-returns.py"
    if not path.is_file():
        return None
    spec = importlib.util.spec_from_file_location("c_returns", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


RETURN_TYPE_DIAG = re.compile(
    r"error: .*(non-void function does not return a value"
    r"|control reaches end of non-void function"
    r"|\[-Werror?=?return-type\])"
)


def cmd_generated_c(args) -> int:
    net_a = _load_net_a()
    if net_a is None:
        print("HARNESS scripts/check-c-returns.py is missing; Net A cannot run")
        emit(outcome=MALFUNCTION, reason="Net A analyser missing")
        return EXIT_MALFUNCTION

    violations = 0
    harness = 0
    recognised_total = 0

    for path in args.files:
        p = Path(path)
        if not p.is_file():
            print(f"HARNESS {path}: does not exist — nothing was analysed")
            harness += 1
            continue
        if not os.access(path, os.R_OK):
            print(f"HARNESS {path}: is not readable — nothing was analysed")
            harness += 1
            continue

        # ---- Net A: structural, needs no compiler to have an opinion --------
        try:
            v, h, recognised = net_a.check_file(path)
        except Exception as exc:  # noqa: BLE001 — an analyser bug is a malfunction
            print(f"HARNESS {path}: Net A raised: {exc!r}")
            harness += 1
            continue
        recognised_total += recognised
        if h:
            harness += 1
            continue
        if v:
            violations += v

        # ---- Net B: the same question, answered by a real compiler ----------
        rb = run([args.cc, "-fsyntax-only", "-Werror=return-type", "-I", args.runtime, path])
        outcome, text = classify(rb)
        if outcome == MALFUNCTION:
            print(f"HARNESS {path}: Net B — {args.cc} {rb.describe()}, so it proves nothing here")
            harness += 1
            continue
        if outcome == REJECTED:
            errs = [e for e in strip_ansi(text).splitlines() if "error:" in e]
            if not errs:
                print(f"HARNESS {path}: Net B — {args.cc} failed with no error diagnostic to classify")
                harness += 1
                continue
            unrelated = [e for e in errs if not RETURN_TYPE_DIAG.search(e)]
            if unrelated:
                print(
                    f"HARNESS {path}: Net B — {len(unrelated)} of {len(errs)} diagnostics are NOT "
                    f"return-type errors, so {args.cc} failed for another reason"
                )
                for e in unrelated[:3]:
                    print(f"HARNESS   {e.strip()}")
                harness += 1
                continue
            for e in errs[:5]:
                print(f"FINDING {e.strip()}")
            violations += len(errs)

    print(f"ANALYSED {recognised_total} function definition(s) in {len(args.files)} file(s)")
    if harness:
        emit(outcome=MALFUNCTION, reason=f"{harness} input(s) could not be analysed")
        return EXIT_MALFUNCTION
    if violations:
        emit(outcome="finding", count=violations)
        return EXIT_FINDING
    emit(outcome=SUCCESS, analysed=recognised_total)
    return EXIT_OK


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = ap.add_subparsers(dest="cmd", required=True)

    a = sub.add_parser("pdc-verdict", help="classify how pdc treats one file")
    a.add_argument("file")
    a.add_argument("--pdc", default="./target/release/pdc")
    a.add_argument("--out", required=True)
    a.set_defaults(fn=cmd_pdc_verdict)

    b = sub.add_parser("pdc-reject", help="require pdc to refuse a file in a specific way")
    b.add_argument("file")
    b.add_argument("--pdc", default="./target/release/pdc")
    b.add_argument("--out", required=True)
    b.add_argument("--expect-stage", choices=["compile", "link"])
    b.add_argument("--require", help="substring the diagnostic must contain")
    b.add_argument("--cwd")
    b.add_argument("--env", action="append")
    b.set_defaults(fn=cmd_pdc_reject)

    c = sub.add_parser("generated-c", help="structural invariant on emitted C")
    c.add_argument("files", nargs="+")
    c.add_argument("--cc", default=os.environ.get("CC", "gcc"))
    c.add_argument("--runtime", default="runtime")
    c.set_defaults(fn=cmd_generated_c)

    args = ap.parse_args(argv)
    return args.fn(args)


if __name__ == "__main__":
    sys.exit(main())
