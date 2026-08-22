#!/usr/bin/env python3
"""Typed-result boundary for every process this gate treats as evidence.

WHY THIS FILE EXISTS
--------------------
Five review rounds found the same defect class in seven places, twice inside the
machinery built to close it: a process failed, something read its output, and a
semantic verdict was issued without anyone first establishing that the process
had finished its experiment.

    THE RULE:
    diagnostic text is never sufficient evidence of a verdict.
    The EXIT CODE says whether the experiment ran.
    The TEXT only says what it found, and is readable only afterwards.

Text can be buffered by a process that then dies. Measured:

    $ sh -c 'echo "error: No main function found" >&2; kill -9 $$'
    exit 137, expected diagnostic already on stderr

HOW THE RULE IS MADE STRUCTURAL — AND WHICH OF THE TWO OPTIONS THIS IS
----------------------------------------------------------------------
An earlier version claimed the prohibited ordering was "no longer expressible".
That was FALSE: `Run._text` was a public attribute and the malfunction path
re-published six lines of buffered producer output, so a caller could ignore
exit 2 and grep exactly the diagnostic a dying producer had printed. A
convention, not a barrier.

Of the two options offered in review — do not republish, or republish as a
distinct type that cannot be pattern-matched for a verdict — **this module does
not republish**. Exactly what that is worth, because an earlier version of this
paragraph claimed more than the code delivers:

  * ONE BOUNDARY. Every subprocess a gate runs goes through `run()`, so
    "did this producer conclude?" is answered in one place.
  * THE VERDICT TYPE CARRIES NO TEXT. `classify()` returns `Concluded` or
    `Malfunction`; only `Concluded` has `.text`. Writing `res.text` on a result
    that might be a malfunction raises AttributeError instead of yielding a
    silent empty string, so the ACCIDENT is caught by the interpreter.
  * NOTHING REACHES THE PARSED STREAM. The malfunction path prints no producer
    text on stdout; the bytes go to a side file whose PATH is announced
    (`WITHHELD_AT`). Gates grep stdout, so this is the property that actually
    blocks the failure mode — a caller cannot grep what was never printed.

THE INVARIANT IS NON-PUBLICATION, NOT DESTRUCTION
-------------------------------------------------
Say it that way round, because saying it the other way round is how this file
came to make a claim it could not keep. The bytes still exist after
classification, in `Run._out` and in `Withheld._b`; a leading underscore is a
naming convention, and Python has no access control. Earlier text here, and
briefs quoting it, said the output of a producer that did not conclude could
not be got at. Measured, it can: `r._out`, `v.withheld._b`. A false claim in
the module whose entire job is "do not certify what you did not establish" is
the disease it exists to treat.

Fusing `run()` and `classify()` and discarding on the malfunction branch WOULD
be possible — it is a design choice, not an impossibility. It is declined:

  * `spill()` is the DESIGNED channel for a malfunction's output — to a file,
    for a human, never to the stream a shell parses. Delete it and the next
    person debugging a malfunction re-runs the producer under their own ad-hoc
    capture, which is the failure mode with no boundary at all.
  * Destruction would buy nothing the gates need. What the gates need is that
    a producer's text never reaches a stream a verdict is read from, and that
    is a property of what is PRINTED, not of what is retained.

So the guarantee is narrower than "cannot", and it is MECHANICAL.
scripts/test-gate-probe.sh fails if `Malfunction` grows a `text` attribute, if
`Withheld`'s `repr`/`str` starts carrying the bytes, if `spill()` reports a
write it did not complete, or if this file's prose re-grows the overclaim (that
check reads the WHOLE file — an earlier version scanned only the text above this
heading, which is how the docstrings on `Run` and `classify` went on asserting
the retracted version for a round).

AND THE SCOPE OF THE PRIVATE-NAME RULE, STATED AT ITS TRUE SIZE. The gate greps
EVERY GIT-TRACKED FILE (binaries skipped) for the literal spellings `._out`,
`._b` and `.withheld._`, outside this module and the enforcer itself, ignoring
comment lines. That is the whole of it. It is a LEXICAL CONVENTION GUARD:
`getattr(x, "_b")`, a name assembled from tokens, `vars(x)["_b"]`, anything
built at runtime, and an untracked file all walk past it. It does not stop a
determined caller and nothing in Python would; what it does is make the
ORDINARY way of writing the shortcut visible to a gate, so taking it has to be
deliberate and in a form a reviewer notices. An earlier version of this
paragraph described the rule as covering every consumer everywhere, which was
wider than anything any gate checked — the promise is now the same sentence as
the mechanism, and a re-widening is itself a checked phrase.

TOTALITY
--------
Every path out of this module lands in the taxonomy. `run()` catches `OSError`
as a class (not just `FileNotFoundError`, so `PermissionError` and `ENOEXEC` are
covered), the Net A import is guarded, and `main()` converts any uncaught
exception into exit 2. An uncaught exception would otherwise exit 1 — the code
reserved for a genuine finding — which is the RecursionError bug fixed one round
ago, one layer up.

EXIT CODES (uniform across every subcommand, so a caller has one decision)
    0  the experiment ran and reached a normal conclusion; fields on stdout
    1  the experiment ran and produced a reportable FINDING
    2  MALFUNCTION — nothing was established, no verdict may be inferred

PLATFORM CONTRACT
-----------------
`pdc` exits 0 on success and 1 for every rejection — parse error, no-main, gcc
failure and a missing input file alike. Measured on this project's targets, not
assumed, and `calibrate` re-measures it, so a platform where a rejection returns
0 is caught rather than silently read as success.
"""

from __future__ import annotations

import argparse
import importlib.util
import os
import re
import signal
import subprocess
import sys
import tempfile
import traceback
from dataclasses import dataclass
from pathlib import Path

EXIT_OK = 0
EXIT_FINDING = 1
EXIT_MALFUNCTION = 2

HERE = Path(__file__).resolve().parent
TIMEOUT_S = 300


def _unlink(path: Path) -> None:
    """Remove a temporary that must not be mistaken for a finished spill."""
    try:
        path.unlink()
    except OSError:
        pass


class Withheld:
    """Output of a process that did not conclude. Not evidence.

    WHAT THIS TYPE DOES: it stops the bytes from being printed, formatted or
    concatenated BY ACCIDENT. It is not a string and not iterable, and `str()`
    and f-string interpolation give the repr below rather than the output — so
    `print(f"{m.withheld}")`, the shape by which a diagnostic reaches a stream a
    shell greps, cannot leak it.

    WHAT IT DOES NOT DO: hide the bytes. `_b` is a Python convention, not access
    control, and the earlier repr ("<withheld: …>") implied a barrier this class
    cannot provide. The bytes are kept because `spill()` is the intended way to
    get a malfunction's output to a HUMAN — a named file, announced as a path —
    and the alternative to that channel is not "no leak", it is somebody
    re-running the producer with an ad-hoc capture and no boundary at all.

    The rule that IS enforced lives in scripts/test-gate-probe.sh: a grep over
    every git-tracked file for the literal spellings `._b` and `._out`, outside
    this module and the enforcer, comment lines excluded. That is a convention
    guard, not access control — see the module docstring for exactly what walks
    past it.
    """

    __slots__ = ("_b",)

    def __init__(self, b: str) -> None:
        self._b = b

    def __repr__(self) -> str:
        return ("<output of a process that did not conclude; not evidence — "
                "spill(path) writes it out for a human to read>")

    __str__ = __repr__

    def spill(self, path: Path):
        """Write the output to `path`. -> (path, error or None).

        IT USED TO SWALLOW THE ERROR AND RETURN THE PATH ANYWAY, and the caller
        printed `WITHHELD_AT <path>` over a file that does not exist. Measured:
        `spill(Path('/nonexistent-dir-xyz/out.txt'))` returned that path, wrote
        nothing, and the operator was told the diagnostic had been preserved.

        That is worse than an ordinary swallowed error. The whole argument for
        keeping these bytes is that `spill()` is the DESIGNED channel — the
        alternative to a designed channel is somebody re-running the producer
        under an ad-hoc capture with no boundary at all. A channel that fails
        silently exactly when it is needed, and reports success, is not a
        channel; it is the failure mode wearing the channel's name.

        The error is RETURNED rather than raised so the malfunction report can
        still be emitted — losing the report as well as the bytes helps nobody —
        but it must be announced. `report_malfunction` prints `WITHHELD_LOST`
        instead of `WITHHELD_AT`, and says why.

        EXISTENCE IS NOT PRESERVATION — the second correction. The first fix
        asked `path.is_file()`, which passes on a STALE file left by an earlier
        run and on a PARTIALLY written one, so `WITHHELD_AT` could still name
        something that is not this producer's output. The guarantee is now the
        whole of it: the bytes go to a temporary sibling, are READ BACK AND
        COMPARED BYTE FOR BYTE, and only then renamed into place. `os.replace`
        is atomic within a filesystem, so after a successful spill the target
        holds the complete output of THIS run, and after a failed one it is
        never a half-written file — it is whatever was there before, which the
        caller is told about explicitly.
        """
        data = self._b.encode("utf-8", "replace")
        tmp = path.with_name(f"{path.name}.partial.{os.getpid()}")
        try:
            tmp.write_bytes(data)
        except OSError as exc:
            _unlink(tmp)
            return path, f"{type(exc).__name__}: {exc}"
        # Read back. A short write, a full disk, or a filesystem that accepted
        # the call and kept less than it was given, all look like success from
        # write_bytes() alone.
        try:
            back = tmp.read_bytes()
        except OSError as exc:
            _unlink(tmp)
            return path, f"cannot read back what was just written: {exc}"
        if back != data:
            _unlink(tmp)
            return path, (f"read-back differs: wrote {len(data)} byte(s), read "
                          f"back {len(back)}; that file would not have been the "
                          f"output")
        try:
            os.replace(tmp, path)
        except OSError as exc:
            _unlink(tmp)
            return path, f"cannot move the spill into place: {exc}"
        return path, None


@dataclass(frozen=True)
class Concluded:
    """The experiment finished at a pinned exit code. `text` IS evidence."""

    text: str
    rc: int
    succeeded: bool


@dataclass(frozen=True)
class Malfunction:
    """Nothing was established. Carries how it died, and NO text."""

    how: str
    withheld: Withheld


class Run:
    """A finished process, its exit status, and its output.

    `classify()` is the route a caller is meant to take: it turns the status
    into a verdict and only then hands over the text. The output is in `_out`,
    an ordinary attribute behind a naming convention — reachable, and not
    pretended otherwise. See the module docstring for what is actually
    guaranteed, and scripts/test-gate-probe.sh for the rule that is enforced.

    `capture_error` is set when the OUTPUT could not be obtained, whatever the
    producer's exit status was. It is a separate field rather than a smuggled
    exit code so that the rule is legible where it is enforced, in `classify()`:
    a run whose evidence was not captured is a malfunction even if the producer
    exited 0.
    """

    __slots__ = ("argv", "rc", "_out", "capture_error")

    def __init__(self, argv, rc: int, out: str, capture_error=None) -> None:
        self.argv = list(argv)
        self.rc = rc
        self._out = out
        self.capture_error = capture_error

    @property
    def signal_number(self):
        """Signal that killed the process, else None.

        TWO CONVENTIONS, and getting this wrong is how a signal check silently
        never fires: subprocess reports a signal-killed child as NEGATIVE (-9),
        a POSIX shell reports 128+signum (137). Both are recognised.
        """
        if self.rc < 0:
            return -self.rc
        if self.rc > 128:
            return self.rc - 128
        return None

    def describe(self) -> str:
        if self.capture_error is not None:
            # The producer's own status is reported too, because "it exited 0"
            # is exactly the fact that would otherwise have been mistaken for a
            # verdict. It is context, not a conclusion.
            s = self.signal_number
            how = f"killed by signal {s}" if s is not None else f"exit {self.rc}"
            return (f"{how}, but its output could not be captured "
                    f"({self.capture_error}), so nothing it printed was read")
        s = self.signal_number
        return f"killed by signal {s}" if s is not None else f"exit {self.rc}"


def run(argv, cwd=None, env=None) -> Run:
    """Run a process. Every failure mode becomes a Run, never an exception.

    The child gets its own process group so a timeout kills DESCENDANTS too: a
    grandchild holding the output open would otherwise outlive the timeout.

    THE OUTPUT GOES TO A TEMPORARY FILE, NOT A PIPE — and that is not a style
    choice. WAIT ON THE CHILD, NOT ON THE PIPE (measured: a grandchild running
    `sleep 600` kept a pipe read blocked for the full timeout even though the
    direct child had exited) means nobody drains the pipe while the child runs,
    and a pipe holds only one buffer — 64 KiB here. A producer that writes more
    than that BLOCKS in write(2) forever, the parent blocks in wait(), and the
    harness reports `timed out after 300s`: a MALFUNCTION VERDICT MANUFACTURED
    BY THE HARNESS ITSELF, indistinguishable from a producer that really hung.

    Measured, 2026-08-22: `cargo test --release --no-fail-fast -- --ignored`
    emits 78296 bytes. Through the old pipe form it took the full 300s and
    returned 124; through a file it takes 45s and returns 101, its real verdict.
    Every current caller stayed under 64 KiB, which is why this never fired —
    the bound was on the INPUTS, not on the harness.

    A file has no such bound, cannot block the writer, and cannot be held open
    by a descendant in a way that stalls the reader.
    """
    merged = dict(os.environ)
    if env:
        merged.update(env)
    sink = tempfile.TemporaryFile()
    try:
        try:
            proc = subprocess.Popen(
                list(argv),
                cwd=cwd,
                env=merged,
                stdout=sink,
                stderr=subprocess.STDOUT,
                start_new_session=True,
            )
        except OSError as exc:
            # FileNotFoundError, PermissionError, ENOEXEC, IsADirectoryError ...
            return Run(argv, 126, f"cannot execute: {exc}")
        except Exception as exc:  # noqa: BLE001
            return Run(argv, 125, f"launch failed: {exc!r}")

        # start_new_session makes the child a process-group leader, so its pid is
        # the pgid and stays usable after the child itself is reaped.
        pgid = proc.pid
        timed_out = False
        try:
            proc.wait(timeout=TIMEOUT_S)
        except subprocess.TimeoutExpired:
            timed_out = True

        # Kill the group whatever happened: a descendant still writing would
        # otherwise keep appending to the file we are about to read.
        try:
            os.killpg(pgid, signal.SIGKILL)
        except OSError:
            pass
        try:
            proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            pass

        # THE CAPTURE IS EVIDENCE, AND A CAPTURE THAT FAILED IS NOT EVIDENCE.
        # This used to swallow the OSError, substitute b"", and hand back the
        # PRODUCER'S exit code — so a producer that exited 0 whose output could
        # not be read became `Concluded("")`: a semantic verdict built on
        # evidence nobody ever saw, inside the module that exists to stop
        # exactly that. It is now a capture error, and `classify()` turns it
        # into a Malfunction whatever the producer did.
        out, capture_error = b"", None
        try:
            sink.seek(0)
            out = sink.read()
        except OSError as exc:
            capture_error = f"{type(exc).__name__}: {exc}"
        else:
            # Read what is there, and confirm it is all of it. A short read
            # leaves a plausible prefix, which is worse than no output at all:
            # a prefix parses.
            try:
                size = os.fstat(sink.fileno()).st_size
            except OSError as exc:
                capture_error = f"cannot size the capture: {exc}"
            else:
                if len(out) != size:
                    capture_error = (f"short read: {len(out)} of {size} byte(s); "
                                     f"a prefix of a producer's output parses "
                                     f"and is not its output")
    finally:
        try:
            sink.close()
        except OSError:
            pass

    text = out.decode("utf-8", "replace") if isinstance(out, bytes) else str(out)
    if timed_out:
        return Run(argv, 124, f"timed out after {TIMEOUT_S}s\n{text}",
                   capture_error)
    return Run(argv, proc.returncode, text, capture_error)


def classify(r: Run, reject_codes=(1,)):
    """Concluded | Malfunction — the intended route from a status to a verdict.

    A `Concluded` carries `.text`; a `Malfunction` has no such attribute, so
    reading the output of a producer that did not conclude cannot happen BY
    ACCIDENT. It can still happen on purpose, via `Run._out`; that is a naming
    convention plus the grep in scripts/test-gate-probe.sh, not a barrier.

    TWO WAYS TO HAVE NO EVIDENCE, AND THEY ARE THE SAME VERDICT. A producer
    that did not conclude has no readable text; a capture that did not conclude
    has no readable text either, and the producer's exit code says nothing
    about the second. So the capture is checked FIRST: `Concluded` means the
    producer finished at a pinned code AND its output was read in full.
    """
    if r.capture_error is not None:
        return Malfunction(r.describe(), Withheld(r._out))
    if r.rc == 0:
        return Concluded(r._out, r.rc, True)
    if r.signal_number is None and r.rc in reject_codes:
        return Concluded(r._out, r.rc, False)
    return Malfunction(r.describe(), Withheld(r._out))


ANSI = re.compile(r"\033\[[0-9;]*m")


def strip_ansi(s: str) -> str:
    return ANSI.sub("", s)


def error_lines(text: str):
    return [ln.strip() for ln in strip_ansi(text).splitlines() if "error" in ln]


def emit(**fields):
    for k, v in fields.items():
        print(f"{k.upper()} {v}")


def report_malfunction(what: str, m: Malfunction, spill_to=None) -> int:
    """Announce a malfunction WITHOUT republishing any producer text.

    `WITHHELD_AT <path>` is a promise that the output is at that path, so it is
    printed only when the write is CONFIRMED. When the spill fails, the
    distinct token `WITHHELD_LOST` is printed with the reason — never the
    bytes — so an operator is told the diagnostic is gone rather than sent to a
    file that is not there.
    """
    emit(outcome="malfunction", reason=f"{what} {m.how}")
    if spill_to:
        path, err = m.withheld.spill(Path(spill_to))
        if err is None:
            print(f"WITHHELD_AT {path}")
        else:
            # NOT "it is gone" — the spill is written to a temporary and renamed,
            # so a failure leaves whatever was at `path` BEFORE this run, which
            # may be a stale file from an earlier one. Telling an operator the
            # output is gone while a plausible file sits there is the same class
            # of lie this whole path was fixed for.
            print(f"WITHHELD_LOST the withheld output was NOT written to "
                  f"{path}: {err}. Anything at that path is from an earlier "
                  f"run, not this one; re-run the producer to see its output.")
    else:
        print("WITHHELD output of a process that did not conclude is not evidence "
              "and is not reproduced here")
    return EXIT_MALFUNCTION


LOC_RE = re.compile(r"--> ([^ :]+):(\d+):(\d+)")


def classify_blocker(path: str, text: str) -> str:
    """WHY a file was rejected, as a stable CATEGORY rather than a wording.

    Runs only on `Concluded.text`, so only on a proven rejection.
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
def cmd_calibrate(args) -> int:
    """Re-measure the producer's exit contract instead of trusting the docstring."""
    tmp = Path(args.scratch)
    try:
        tmp.mkdir(parents=True, exist_ok=True)
        good = tmp / "cal_ok.pd"
        good.write_text('fn main() { print("ok"); }\n')
        bad = tmp / "cal_bad.pd"
        bad.write_text("use nope::nope;\nfn main() { }\n")
    except OSError as exc:
        emit(outcome="malfunction", reason=f"cannot prepare calibration inputs: {exc}")
        return EXIT_MALFUNCTION

    r_ok = run([args.pdc, "compile", str(good), "-o", "cal_ok"])
    r_bad = run([args.pdc, "compile", str(bad), "-o", "cal_bad"])
    problems = []
    if r_ok.rc != 0:
        problems.append(f"a VALID program did not exit 0 ({r_ok.describe()})")
    if r_bad.rc == 0:
        problems.append(
            "an INVALID program exited 0 — on this platform a rejection is "
            "indistinguishable from success, so every verdict below would be wrong")
    elif r_bad.signal_number is not None or r_bad.rc != 1:
        problems.append(
            f"a rejection gave {r_bad.describe()}, not the pinned exit 1 — update the "
            "pinned reject code, or this platform is unsupported")
    if problems:
        emit(outcome="malfunction", reason="pdc exit contract does not hold here")
        for p in problems:
            print(f"WITHHELD_NOTE {p}")
        return EXIT_MALFUNCTION
    emit(outcome="success", success_exit=r_ok.rc, reject_exit=r_bad.rc, platform=sys.platform)
    return EXIT_OK


def cmd_pdc_verdict(args) -> int:
    r = run([args.pdc, "compile", args.file, "-o", args.out])
    res = classify(r)
    if isinstance(res, Malfunction):
        return report_malfunction(f"pdc on {args.file}", res, args.spill)
    if res.succeeded:
        emit(outcome="success", verdict="COMPILE_OK")
        print("BLOCKER -")
        return EXIT_OK

    errs = error_lines(res.text)
    no_main = [e for e in errs if "No main function found" in e]
    others = [e for e in errs if "No main function found" not in e]
    if no_main and not others:
        verdict = "ACCEPTED_NO_MAIN"
    elif "gcc compilation failed" in strip_ansi(res.text):
        verdict = "LINK_FAIL"
    else:
        verdict = "COMPILE_FAIL"
    emit(outcome="rejected", verdict=verdict)
    print("BLOCKER " + ("-" if verdict == "ACCEPTED_NO_MAIN"
                        else classify_blocker(args.file, res.text)))
    first = next(iter(errs), "")
    if first:
        print(f"DIAG {first}")
    return EXIT_OK


def cmd_pdc_reject(args) -> int:
    env = {}
    for pair in args.env or []:
        k, _, v = pair.partition("=")
        env[k] = v
    r = run([args.pdc, "compile", args.file, "-o", args.out], cwd=args.cwd, env=env)
    res = classify(r)
    if isinstance(res, Malfunction):
        return report_malfunction(f"pdc on {args.file}", res, args.spill)
    if res.succeeded:
        emit(outcome="accepted")
        return EXIT_OK

    plain = strip_ansi(res.text)
    stage = "link" if "gcc compilation failed" in plain else "compile"
    first = next(iter(error_lines(res.text)), "")
    if args.expect_stage and stage != args.expect_stage:
        emit(outcome="rejected-other", stage=stage,
             reason=f"expected rejection at {args.expect_stage}, got {stage}")
        if first:
            print(f"DIAG {first}")
        return EXIT_OK
    if args.require and args.require not in plain:
        emit(outcome="rejected-other", stage=stage,
             reason="rejected at the expected stage but not with the expected diagnostic")
        if first:
            print(f"DIAG {first}")
        return EXIT_OK
    emit(outcome="rejected-as-expected", stage=stage)
    return EXIT_OK


def _load_net_a():
    """Import the structural analyser. Any failure RAISES, and main() maps it."""
    path = HERE / "check-c-returns.py"
    if not path.is_file():
        raise RuntimeError(f"{path} is missing")
    spec = importlib.util.spec_from_file_location("c_returns", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"{path} is not importable")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    if not hasattr(mod, "check_file"):
        raise RuntimeError(f"{path} defines no check_file()")
    return mod


RETURN_TYPE_DIAG = re.compile(
    r"error: .*(non-void function does not return a value"
    r"|control reaches end of non-void function"
    r"|\[-Werror?=?return-type\])"
)


def cmd_generated_c(args) -> int:
    try:
        net_a = _load_net_a()
    except Exception as exc:  # noqa: BLE001
        emit(outcome="malfunction", reason=f"Net A could not be loaded: {exc}")
        return EXIT_MALFUNCTION

    violations = harness = recognised_total = 0
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
        try:
            v, h, recognised = net_a.check_file(path)
        except Exception as exc:  # noqa: BLE001
            print(f"HARNESS {path}: Net A raised: {exc!r}")
            harness += 1
            continue
        recognised_total += recognised
        if h:
            harness += 1
            continue
        violations += v

        rb = run([args.cc, "-fsyntax-only", "-Werror=return-type", "-I", args.runtime, path])
        res = classify(rb)
        if isinstance(res, Malfunction):
            print(f"HARNESS {path}: Net B — {args.cc} {res.how}, so it proves nothing here")
            harness += 1
            continue
        if res.succeeded:
            continue
        errs = [e for e in strip_ansi(res.text).splitlines() if "error:" in e]
        if not errs:
            print(f"HARNESS {path}: Net B — {args.cc} failed with no error diagnostic to classify")
            harness += 1
            continue
        unrelated = [e for e in errs if not RETURN_TYPE_DIAG.search(e)]
        if unrelated:
            print(f"HARNESS {path}: Net B — {len(unrelated)} of {len(errs)} diagnostics are NOT "
                  f"return-type errors, so {args.cc} failed for another reason")
            for e in unrelated[:3]:
                print(f"HARNESS   {e.strip()}")
            harness += 1
            continue
        for e in errs[:5]:
            print(f"FINDING {e.strip()}")
        violations += len(errs)

    print(f"ANALYSED {recognised_total} function definition(s) in {len(args.files)} file(s)")
    if harness:
        emit(outcome="malfunction", reason=f"{harness} input(s) could not be analysed")
        return EXIT_MALFUNCTION
    if violations:
        emit(outcome="finding", count=violations)
        return EXIT_FINDING
    emit(outcome="success", analysed=recognised_total)
    return EXIT_OK


def cmd_reconcile(args) -> int:
    """Cross-check the builtin registry's unsupported set against our manifest.

    Inside the boundary because it, too, was an unstructured `exit 1` that a
    shell mapped onto a semantic result: a traceback carrying no MISSING lines
    was read as "reconciled".
    """
    try:
        src = Path(args.src).read_text(errors="replace")
    except OSError as exc:
        emit(outcome="malfunction", reason=f"cannot read {args.src}: {exc}")
        return EXIT_MALFUNCTION
    try:
        manifest_lines = Path(args.manifest).read_text(errors="replace").splitlines()
    except OSError as exc:
        emit(outcome="malfunction", reason=f"cannot read {args.manifest}: {exc}")
        return EXIT_MALFUNCTION

    # NOTE: substring activation is a known-weak contract — an unrelated
    # occurrence of "Support" activates it and a rename returns it to dormant.
    # The durable fix is for the registry to emit a machine-readable list both
    # gates read; raised as a follow-up unit now that branch has merged.
    if "Support" not in src:
        emit(outcome="dormant",
             reason=f"no Support type in {args.src}; the sibling registry has not landed")
        return EXIT_OK

    names = set()
    for block in re.findall(r"Builtin\s*\{.*?\n    \}", src, re.S):
        if "Support::Unsupported" in block:
            m = re.search(r'name:\s*"([a-z_0-9]+)"', block)
            if m:
                names.add(m.group(1))
    if not names:
        arr = re.search(r"PRELUDE_TYPE_MISMATCHES[^=]*=\s*&\[(.*?)\];", src, re.S)
        if arr:
            names.update(re.findall(r'"([a-z_0-9]+) (?:param|return)', arr.group(1)))
    if not names:
        emit(outcome="malfunction",
             reason=f"{args.src} has the Support type but no unsupported builtin could be "
                    "extracted — the parsing contract broke")
        return EXIT_MALFUNCTION

    recorded = {}
    for line in manifest_lines:
        if line.strip() and not line.startswith("#"):
            c = line.split("\t")
            if len(c) >= 2:
                recorded[c[0]] = c[1]
    missing = sorted(n for n in names if recorded.get(n) != "UNUSABLE")
    for n in missing:
        print(f"FINDING {n} is marked unsupported in {args.src} but recorded "
              f"'{recorded.get(n, '<absent>')}' in {args.manifest}")
    if missing:
        emit(outcome="finding", count=len(missing))
        return EXIT_FINDING
    emit(outcome="success", checked=len(names))
    return EXIT_OK


def build_parser():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = ap.add_subparsers(dest="cmd", required=True)

    cal = sub.add_parser("calibrate", help="re-measure pdc's exit contract on this platform")
    cal.add_argument("--pdc", default="./target/release/pdc")
    cal.add_argument("--scratch", default="build_output/gate_calibrate")
    cal.set_defaults(fn=cmd_calibrate)

    a = sub.add_parser("pdc-verdict", help="classify how pdc treats one file")
    a.add_argument("file")
    a.add_argument("--pdc", default="./target/release/pdc")
    a.add_argument("--out", required=True)
    a.add_argument("--spill")
    a.set_defaults(fn=cmd_pdc_verdict)

    b = sub.add_parser("pdc-reject", help="require pdc to refuse a file in a specific way")
    b.add_argument("file")
    b.add_argument("--pdc", default="./target/release/pdc")
    b.add_argument("--out", required=True)
    b.add_argument("--expect-stage", choices=["compile", "link"])
    b.add_argument("--require")
    b.add_argument("--cwd")
    b.add_argument("--env", action="append")
    b.add_argument("--spill")
    b.set_defaults(fn=cmd_pdc_reject)

    c = sub.add_parser("generated-c", help="structural invariant on emitted C")
    c.add_argument("files", nargs="+")
    c.add_argument("--cc", default=os.environ.get("CC", "gcc"))
    c.add_argument("--runtime", default="runtime")
    c.set_defaults(fn=cmd_generated_c)

    d = sub.add_parser("reconcile", help="registry unsupported set vs our manifest")
    d.add_argument("--src", default="src/builtins.rs")
    d.add_argument("--manifest", default="tests/stdlib/BUILTINS.tsv")
    d.set_defaults(fn=cmd_reconcile)
    return ap


def main(argv=None) -> int:
    # TOTALITY. Any escape from a subcommand is a malfunction, never a finding:
    # an uncaught exception would otherwise exit 1, the code reserved for a
    # genuine finding.
    try:
        args = build_parser().parse_args(argv)
    except SystemExit as e:
        return EXIT_OK if e.code == 0 else EXIT_MALFUNCTION
    try:
        return args.fn(args)
    except Exception:  # noqa: BLE001
        emit(outcome="malfunction", reason="gate_probe raised; nothing was established")
        for line in traceback.format_exc().rstrip().splitlines()[-8:]:
            print(f"WITHHELD_NOTE {line}")
        return EXIT_MALFUNCTION


if __name__ == "__main__":
    sys.exit(main())
