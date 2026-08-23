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

WHAT `Concluded` MEANS — AT ITS FINAL WIDTH, after three attempts to state it
    The producer exited at a pinned code, AND the output stream reached EOF
    BEFORE this harness killed anything.

    That second clause is the whole of what the capture establishes, and it is
    narrower than it has twice been written here. EOF is the event "the last
    write handle closed" — nothing more. It does not mean the producer finished
    saying what it had to say: a descendant can emit a prefix, then crash or be
    killed by something outside this process, close the final handle, and EOF
    arrives on a truncated capture. Neither this nor any observer of a pipe can
    tell that from an orderly finish; the only thing that could is a
    completion protocol owned by the producer, which these producers (pdc, gcc,
    cargo) do not have.

    So the claim is deliberately about PROVENANCE, not completeness: the close
    was the writers' doing and not the harness's. That excludes the defect that
    was actually here — `killpg` ran first, so EOF was manufactured by cleanup
    and proved "every writer is closed after I killed them", which is true of
    every run.

    WHAT WOULD FALSIFY IT: a run reported `Concluded` whose EOF arrived only
    after `killpg`. scripts/test-gate-probe.sh drives exactly that, with a
    descendant that stays in the process group so the kill reaches it, and
    requires a Malfunction. Measured at 8839613, before the order was fixed:
    exit 0.
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
`._b` and `.withheld._`, outside this module and the enforcer itself, exempting
a mention only where it sits wholly inside a backticked bare dotted name. A
comment is NOT skipped: `# the _out slot` written without backticks is a
finding, and the fix is the backticks the convention asks for. (This paragraph
claimed the opposite for two rounds, as did `Withheld`'s docstring below, while
scripts/test-xfail.py described the real mechanism — three statements of one
rule, drifting. Both spellings of the false one are now checked phrases.)
That is the whole of it. It is a LEXICAL CONVENTION GUARD:
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
import stat
import subprocess
import sys
import tempfile
import threading
import traceback
from dataclasses import dataclass
from pathlib import Path

EXIT_OK = 0
EXIT_FINDING = 1
EXIT_MALFUNCTION = 2

HERE = Path(__file__).resolve().parent
TIMEOUT_S = 300
# How long to wait for EOF after the producer has exited. Reaching it means a
# writer outlived the process group, so the capture is not known to be complete.
DRAIN_JOIN_S = 5
# A capture that never reaches EOF leaves its descriptor with the reader thread
# that still owns it (see run()), so each one costs one descriptor for the life
# of the process. That cost was written down as "a handful of runs" and NOT
# enforced — while `generated-c` takes an unbounded file list and continues past
# every malfunction, so a compiler leaving an escaped writer per invocation
# would accumulate one blocked reader per file until the process ran out. An
# accepted cost has to be an enforced one, or it is a hope in a comment.
MAX_ABANDONED_CAPTURES = 8
_abandoned_captures = 0
# Reproduce the bound (the escapee producer forks, setsid()s, holds stdout):
#   python3 - <<'EOF'
#   import sys, textwrap, tempfile, pathlib
#   sys.path.insert(0, "scripts"); import gate_probe as gp
#   src = textwrap.dedent("""
#       import os, sys, time
#       if os.fork() == 0:
#           os.setsid(); time.sleep(30); os._exit(0)
#       sys.stdout.write("prefix\n"); sys.stdout.flush(); os._exit(0)
#   """)
#   f = pathlib.Path(tempfile.mkdtemp()) / "escapee.py"; f.write_text(src)
#   for i in range(gp.MAX_ABANDONED_CAPTURES + 1):
#       v = gp.run_and_classify([sys.executable, str(f)])
#       print(i, type(v).__name__, gp._abandoned_captures)
#   print(v.how)
#   EOF
# Measured 2026-08-22: eight Malfunctions, the counter reaching 8, then
# "refusing to run: 8 earlier capture(s) ... never reached EOF". The gate itself
# checks the enforcement directly rather than spending 45s reproducing it — see
# scripts/test-gate-probe.sh, "the abandoned-capture bound is enforced".


def _describe_target(path: Path) -> str:
    """What is at `path` after a spill that did not happen.

    The message used to say "anything at that path is from an earlier run",
    which is one guess out of several: a pre-existing DIRECTORY is the most
    likely reason `os.replace` refused, and it is not an earlier-run artifact.
    An operator being told the wrong thing about the file in front of them is
    the same class of defect as being told a file exists when it does not.
    """
    try:
        st = os.lstat(path)
    except FileNotFoundError:
        return "nothing is at that path"
    except OSError as exc:
        return f"and that path cannot be inspected either ({exc})"
    if stat.S_ISDIR(st.st_mode):
        return "a DIRECTORY is at that path, which is why the move was refused"
    if stat.S_ISLNK(st.st_mode):
        return "a symbolic link is at that path"
    if stat.S_ISREG(st.st_mode):
        return (f"a {st.st_size}-byte file is at that path and this invocation "
                f"did not write it")
    return "a non-regular file is at that path"


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
    this module and the enforcer, exempting a mention only where it lies wholly
    inside a backticked bare dotted name — a comment is searched like any other
    line. That is a convention guard, not access control — see the module
    docstring for exactly what walks past it.

    `complete` records whether the capture reached EOF. It is FALSE exactly when
    `Run.capture_error` was set, and it is the difference between "here is what
    the producer wrote" and "here is some of what the producer wrote": without
    EOF the bytes are a prefix of unknown length. `report_malfunction` prints
    `WITHHELD_AT` only for a complete one and `WITHHELD_PARTIAL` otherwise,
    because announcing a prefix under the token that promises the whole output
    is the same lie as announcing a file that is not there.
    """

    __slots__ = ("_b", "complete")

    def __init__(self, b, complete: bool = True) -> None:
        self._b = b
        self.complete = complete

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

        AND WHAT IS WRITTEN IS BYTES. `Withheld` holds the producer's raw
        output; this writes it verbatim. It used to hold a
        UTF-8-with-replacement DECODING which this method re-encoded, so a
        producer that emitted `b"A\xffB"` was spilled as `b"A\xef\xbf\xbdB"`
        while `WITHHELD_AT` promised the run's complete output. An operator
        debugging a producer that emits binary was being handed a different
        file and told it was the same one.

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
        data = self._b if isinstance(self._b, bytes) else str(self._b).encode(
            "utf-8", "surrogateescape")
        # EXCLUSIVE CREATION, in the target's own directory. A predictable
        # `.partial.<pid>` opened with ordinary write semantics can be
        # pre-created as a SYMLINK by anyone who can write that directory, and
        # the write would then follow it and truncate whatever it points at.
        # mkstemp() uses O_CREAT|O_EXCL|O_NOFOLLOW semantics and an unguessable
        # name; the same directory keeps the later rename on one filesystem, so
        # it stays atomic.
        try:
            fd, tmpname = tempfile.mkstemp(dir=str(path.parent),
                                           prefix=f".{path.name}.partial.",
                                           suffix=".tmp")
        except OSError as exc:
            return path, f"{type(exc).__name__}: {exc}"
        tmp = Path(tmpname)
        try:
            with os.fdopen(fd, "wb") as fh:
                fh.write(data)
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
    """The experiment finished at a pinned exit code. `text` IS evidence.

    `text` is a UTF-8-with-replacement rendering of what the producer wrote,
    because every consumer of it greps for diagnostics. For a producer that
    emitted bytes which are not valid UTF-8 it is therefore LOSSY, and it is the
    only lossy thing here: the raw bytes survive on the `Run` and, for a
    malfunction, in `Withheld` — which is what `spill()` writes and what
    `WITHHELD_AT` promises.
    """

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

    `_out` holds RAW BYTES. It used to hold a UTF-8-with-replacement decoding,
    which silently destroys any byte a producer emitted that is not valid UTF-8
    — and `spill()` then re-encoded that string, so the "designed channel" for a
    malfunction's output delivered a lossy rendering while `WITHHELD_AT` promised
    this run's complete output. Measured: a producer emitting `b"A\xffB"` spilled
    as `b"A\xef\xbf\xbdB"`. Bytes in, bytes out; the decoding happens once, at
    `classify()`, and only for `Concluded.text`, which is what callers grep.

    `_rc` is private for the same reason `_out` is, and it was made private
    BECAUSE it was read outside the boundary: `cmd_calibrate` compared raw exit
    codes and never looked at `capture_error`, so two producers whose captures
    failed while their statuses matched still printed `OUTCOME success`. A raw
    status read outside `classify()` is how a verdict gets built on evidence
    nobody established.
    """

    __slots__ = ("argv", "_rc", "_out", "capture_error")

    def __init__(self, argv, rc: int, out, capture_error=None) -> None:
        self.argv = list(argv)
        self._rc = rc
        # Callers (and self-tests) may hand a str; store bytes either way, so
        # there is exactly one representation to reason about.
        self._out = out if isinstance(out, bytes) else str(out).encode("utf-8", "surrogateescape")
        self.capture_error = capture_error

    @property
    def signal_number(self):
        """Signal that killed the process, else None.

        TWO CONVENTIONS, and getting this wrong is how a signal check silently
        never fires: subprocess reports a signal-killed child as NEGATIVE (-9),
        a POSIX shell reports 128+signum (137). Both are recognised.
        """
        if self._rc < 0:
            return -self._rc
        if self._rc > 128:
            return self._rc - 128
        return None

    def describe(self) -> str:
        if self.capture_error is not None:
            # The producer's own status is reported too, because "it exited 0"
            # is exactly the fact that would otherwise have been mistaken for a
            # verdict. It is context, not a conclusion.
            s = self.signal_number
            how = f"killed by signal {s}" if s is not None else f"exit {self._rc}"
            return (f"{how}, but its output could not be captured "
                    f"({self.capture_error}), so what it printed is not "
                    f"established")
        s = self.signal_number
        return f"killed by signal {s}" if s is not None else f"exit {self._rc}"


def run(argv, cwd=None, env=None) -> Run:
    """Run a process. Every failure mode becomes a Run, never an exception.

    "READ IN FULL" IS ESTABLISHED BY EOF, NOT BY A SIZE COMPARISON.
    Three designs have stood here and the first two each traded one hole for
    another:

      1. a pipe, read AFTER waiting on the child. A producer writing more than
         one pipe buffer (64 KiB) blocks in write(2) forever while the parent
         blocks in wait(): the harness manufactured its own `timed out` verdict.
         Measured on a 78 KiB producer: 300s, exit 124.
      2. a shared temporary FILE, sized with fstat() and compared against the
         length read. That removed the deadlock and introduced a TOCTOU:
         `killpg` delivery is ASYNCHRONOUS, descendants are never reaped, and a
         descendant that calls setsid() escapes the group kill entirely while
         still holding the inherited fd. Both the size and the bytes are sampled
         from a file somebody else may still be writing, so agreement between
         them establishes nothing.

    So: a pipe again, but DRAINED CONCURRENTLY by a reader thread. EOF on that
    pipe is not a sample — it is the operating system reporting that every write
    handle has closed. There is no buffer to overflow because the drain never
    stops, and there is nothing to compare because nothing is sampled.

    AND EOF IS EXACTLY THAT EVENT, NO MORE. It says the last writer closed; it
    does not say the writer had finished, or was even alive. A descendant that
    emits a prefix and is then killed by somebody else closes the last handle
    too, and this cannot tell that apart from an orderly finish. What the join
    below DOES establish is that the close was not caused by THIS harness —
    which is the defect that was here, and is worth having, and is all that is
    claimed. See the module docstring, "WHAT `Concluded` MEANS".

    And when EOF does NOT arrive — the escaped-descendant case — the drain
    thread is still running after the bounded join below, and that becomes a
    `capture_error`. That is the honest answer: a capture that cannot be known
    to be complete is not evidence, and `classify()` turns it into a
    Malfunction. The old design answered `Concluded` there.
    """
    global _abandoned_captures
    if _abandoned_captures >= MAX_ABANDONED_CAPTURES:
        # Refuse to start another one. This is itself a malfunction — nothing is
        # established about this producer — and it is the enforcement of the
        # bound above rather than a diagnosis of this particular input.
        return Run(argv, 125, b"",
                   f"refusing to run: {_abandoned_captures} earlier capture(s) "
                   f"in this process never reached EOF and still own their "
                   f"descriptors (limit {MAX_ABANDONED_CAPTURES}). Something is "
                   f"leaving writers behind; fix that rather than raising this")

    merged = dict(os.environ)
    if env:
        merged.update(env)
    try:
        proc = subprocess.Popen(
            list(argv),
            cwd=cwd,
            env=merged,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            start_new_session=True,
            # UNBUFFERED. With Python's default buffering `proc.stdout` is a
            # BufferedReader, which can pull bytes off the pipe into its own
            # buffer and hold them there while `read()` is blocked. Those bytes
            # are off the pipe and not in `chunks`: a prefix that exists nowhere
            # the verdict can see. bufsize=0 makes `stdout` a raw FileIO, so
            # what is read is what is appended.
            bufsize=0,
        )
    except OSError as exc:
        # FileNotFoundError, PermissionError, ENOEXEC, IsADirectoryError ...
        return Run(argv, 126, b"", f"cannot execute: {exc}")
    except Exception as exc:  # noqa: BLE001
        return Run(argv, 125, b"", f"launch failed: {exc!r}")

    chunks: list = []
    drain_error: list = []

    def drain() -> None:
        try:
            while True:
                block = proc.stdout.read(65536)
                if not block:
                    return                      # EOF: every writer has closed
                chunks.append(block)
        except (OSError, ValueError) as exc:    # ValueError: fd closed under us
            drain_error.append(f"{type(exc).__name__}: {exc}")

    reader = threading.Thread(target=drain, daemon=True)
    reader.start()

    # start_new_session makes the child a process-group leader, so its pid is
    # the pgid and stays usable after the child itself is reaped.
    pgid = proc.pid
    timed_out = False
    try:
        proc.wait(timeout=TIMEOUT_S)
    except subprocess.TimeoutExpired:
        timed_out = True

    # THE ORDER IS THE WHOLE POINT, AND IT WAS WRONG.
    # This used to `killpg` FIRST and ask about EOF afterwards. Killing the
    # writers closes their pipe handles, so EOF then arrives BECAUSE OF THE
    # CLEANUP — it proved "all writers are closed after I killed them", which is
    # true of every run and establishes nothing about the producer having
    # finished writing. An exit-0 parent with an in-group descendant still
    # mid-write was reported `Concluded` with a truncated capture.
    #
    # So EOF is awaited BEFORE anything is killed. Now it can only mean the
    # writers closed of their own accord, which is the fact completeness needs.
    # If it does not arrive, the capture is marked incomplete HERE, and the kill
    # below is cleanup only — it can no longer create the evidence it is meant
    # to be judged against.
    capture_error = None
    reader.join(DRAIN_JOIN_S)
    if reader.is_alive():
        capture_error = (
            f"the output stream did not reach EOF within {DRAIN_JOIN_S}s of the "
            f"producer exiting, and before this harness killed anything: a "
            f"write handle is still open, so what was read is a prefix of "
            f"unknown length")
    elif drain_error:
        capture_error = drain_error[0]

    # Cleanup, and ONLY cleanup. Best-effort by construction: delivery is
    # asynchronous and a descendant in its own session is not in the group at
    # all. Nothing below may change the verdict decided above.
    try:
        os.killpg(pgid, signal.SIGKILL)
    except OSError:
        pass
    try:
        proc.wait(timeout=10)
    except subprocess.TimeoutExpired:
        pass

    if capture_error is None:
        # The reader is already finished; closing from here owns no race.
        try:
            proc.stdout.close()
        except (OSError, ValueError):
            pass
    else:
        # Give the reader a moment to notice the kill and exit on its own, and
        # then LEAVE THE DESCRIPTOR ALONE.
        #
        # Closing it from this thread is not cancellation: the daemon thread and
        # its stream still own that descriptor, a blocked read can outlive the
        # close, and the number can be reused underneath it — a later read or a
        # double close would then touch an unrelated file. There is no portable
        # way to cancel a blocked read from another thread, so the honest thing
        # is not to try. The descriptor stays with its reader and is reclaimed
        # when the process exits.
        #
        # The bound this accepts is now COUNTED and ENFORCED: see
        # MAX_ABANDONED_CAPTURES at the top of the file, checked before the next
        # process is started.
        reader.join(1.0)
        if reader.is_alive():
            _abandoned_captures += 1

    out = b"".join(chunks)
    if timed_out:
        return Run(argv, 124, b"timed out after %ds\n" % TIMEOUT_S + out,
                   capture_error)
    return Run(argv, proc.returncode, out, capture_error)


def run_and_classify(argv, cwd=None, env=None, reject_codes=(1,)):
    """Execute and judge in one call. -> Concluded | Malfunction.

    THE ONLY ENTRY POINT PRODUCTION CODE SHOULD USE, and the reason it exists:
    `Run` carries a raw exit status, and twice now something read that status
    without asking whether the output had been captured — `cmd_calibrate` did it
    inside this module, which is exactly where the private-name grep does not
    look. Renaming `rc` to `_rc` moved the spelling; it did not remove the
    reader. Fusing does: a caller of this function never holds a `Run`, so there
    is no raw status for it to read.

    `run()` and `classify()` remain separate and public because the fault
    injections in scripts/test-gate-probe.sh must be able to build a `Run` with
    a chosen status and a broken capture. That is a test surface, and the
    intra-module allowlist there is what keeps it from becoming a production
    one.
    """
    return classify(run(argv, cwd=cwd, env=env), reject_codes=reject_codes)


def classify(r: Run, reject_codes=(1,)):
    """Concluded | Malfunction — the intended route from a status to a verdict.

    THE DEFAULT `(1,)` IS A FRONT-END CONTRACT AND NOT A UNIVERSAL ONE. Any code
    outside it becomes a Malfunction, i.e. "the producer did not conclude its
    experiment" — which is the wrong sentence for a producer that concluded at a
    code this tuple has not been told about. `fix/gcc-diagnostics-discarded`
    (aa63982, src/linker.rs:247-261) makes pdc exit 3 (gcc refused the
    translation unit), 4 (pdc emitted ill-typed C) and 5 (gcc never reached a
    verdict); all three are CONCLUSIONS. Callers that run `pdc` therefore pass
    PDC_REJECT_CODES, and the two that do are cmd_pdc_verdict and cmd_pdc_reject.
    `cmd_calibrate` deliberately keeps the default: its whole job is to MEASURE
    that a front-end rejection is exit 1 on this platform, so widening what it
    accepts would make the measurement vacuous.

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
        return Malfunction(r.describe(), Withheld(r._out, complete=False))
    # The ONE decoding in this module, and it is lossy by design: callers grep
    # `Concluded.text` for diagnostics, which must be a str. The exact bytes are
    # never destroyed — `Withheld` keeps them, and `spill()` writes them
    # verbatim — so the byte-for-byte guarantee lives where it is promised.
    text = r._out.decode("utf-8", "replace")
    if r._rc == 0:
        return Concluded(text, r._rc, True)
    if r.signal_number is None and r._rc in reject_codes:
        return Concluded(text, r._rc, False)
    return Malfunction(r.describe(), Withheld(r._out, complete=True))


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

    THREE TOKENS, BECAUSE THERE ARE THREE OUTCOMES:
      WITHHELD_AT       the complete output is at that path, written byte for
                        byte and confirmed. Requires BOTH a confirmed write and
                        a capture that reached EOF.
      WITHHELD_PARTIAL  a prefix is at that path. The write succeeded; the
                        capture did not complete, so how much is missing is
                        unknown.
      WITHHELD_LOST     nothing was written, with the reason and a description
                        of what is at that path instead.
    None of them prints the bytes.
    """
    emit(outcome="malfunction", reason=f"{what} {m.how}")
    if spill_to:
        path, err = m.withheld.spill(Path(spill_to))
        if err is None and m.withheld.complete:
            print(f"WITHHELD_AT {path}")
        elif err is None:
            # Written, byte for byte — but only of what arrived. Without EOF
            # there is no way to know how much the producer still had to say, so
            # this must not borrow the token that promises the whole output.
            print(f"WITHHELD_PARTIAL {path} holds the {len(m.withheld._b)} "
                  f"byte(s) that had arrived when the capture was abandoned; "
                  f"the producer's output never reached EOF, so this is a "
                  f"prefix of unknown completeness, not its output")
        else:
            # NOT "it is gone" — the spill is written to a temporary and renamed,
            # so a failure leaves whatever was at `path` BEFORE this run, which
            # may be a stale file from an earlier one. Telling an operator the
            # output is gone while a plausible file sits there is the same class
            # of lie this whole path was fixed for.
            print(f"WITHHELD_LOST the withheld output was NOT written to "
                  f"{path}: {err}. {_describe_target(path)}; re-run the "
                  f"producer to see its output.")
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
    # THREE CATEGORIES WERE DELETED HERE, NOT DISABLED: ATTRIBUTE
    # ("Unexpected character '#'"), CHAR_ESCAPE ("Unexpected character '\\'")
    # and FLOAT_LITERAL. N2-08..N2-11 made all three diagnostics unreachable, so
    # the rules could never fire again and the manifest rows that named them
    # moved to the blocker each was masking.
    #
    # FLOAT_LITERAL had to go rather than merely stop firing, and that is the
    # part worth reading: it did not test the DIAGNOSTIC, it tested whether the
    # offending SOURCE LINE contained `[0-9]+\.[0-9]+`. That was sound only
    # while no float could lex. It is now a false positive waiting to happen --
    # `pub const PI: f64 = 3.14159...;` fails for `pub const`, and the old rule
    # would still have called it a float-literal blocker. A classifier that
    # names the wrong cause is worse than one that says OTHER, because OTHER is
    # visibly a question and a wrong category reads as an answer.
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
    # Both added when the lexical blockers above stopped masking them.
    if re.search(r"\*\s*(mut|const)\s", src):
        return "RAW_POINTER"
    if "Unexpected character '^'" in first:
        return "BITWISE_XOR"
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

    # THROUGH `classify()`, like everything else. This used to compare raw
    # `Run.rc` values and never look at `capture_error`, so two producers whose
    # OUTPUT could not be read still calibrated as `OUTCOME success` provided
    # their exit codes matched — the module's own rule ("an unreadable capture
    # is a Malfunction") broken by the subcommand whose job is to establish that
    # the platform behaves as the module assumes.
    v_ok = run_and_classify([args.pdc, "compile", str(good), "-o", "cal_ok"])
    v_bad = run_and_classify([args.pdc, "compile", str(bad), "-o", "cal_bad"])
    problems = []
    if isinstance(v_ok, Malfunction):
        problems.append(f"a VALID program did not conclude ({v_ok.how})")
    elif not v_ok.succeeded:
        problems.append(f"a VALID program did not exit 0 (exit {v_ok.rc})")
    if isinstance(v_bad, Malfunction):
        problems.append(
            f"a rejection did not conclude at the pinned exit 1 ({v_bad.how}) — "
            "update the pinned reject code, or this platform is unsupported")
    elif v_bad.succeeded:
        problems.append(
            "an INVALID program exited 0 — on this platform a rejection is "
            "indistinguishable from success, so every verdict below would be wrong")
    if problems:
        emit(outcome="malfunction", reason="pdc exit contract does not hold here")
        for p in problems:
            print(f"WITHHELD_NOTE {p}")
        return EXIT_MALFUNCTION
    emit(outcome="success", success_exit=v_ok.rc, reject_exit=v_bad.rc,
         platform=sys.platform)
    return EXIT_OK


# --------------------------------------------------------------------------
# The backend rejecting its own output is not a verdict a caller may declare
# --------------------------------------------------------------------------
# There is no valid Palladium program for which pdc accepts the source and gcc
# then refuses the C codegen emitted. If the front end said yes, C that will not
# build is a defect in pdc — never a property of the input, and never something
# a caller may pin as an expectation. scripts/conformance.sh reached that
# conclusion for .pd fixtures; this module carried the SAME two defects for the
# stdlib corpus and is brought onto the same footing here:
#
#   * it classified the outcome by grepping the producer's prose for
#     `gcc compilation failed` — a string a fixture's own text can put in the
#     log, which is the forgeable-classifier defect this whole file exists to
#     refuse ("diagnostic text is never sufficient evidence of a verdict"), and
#   * `--expect-stage` offered `link` as a CHOICE, so a caller could declare the
#     defect and be told `rejected-as-expected`. Measured in use on 2026-08-22:
#     six UNUSABLE builtins were pinned at stage `link` and all six went red
#     with "expected rejection at link, got compile" when the repair made the
#     TYPE CHECKER refuse them (tests/stdlib/BUILTINS.tsv:40-54). The rule was
#     already being applied by hand; only the spelling survived.
#
# WHO refused is decided by the FILESYSTEM, not the log: codegen is the last
# phase, so the translation unit exists if and only if the front end accepted.
# No producer text can reach that.


def emitted_c_path(file: str, cwd=None) -> Path:
    """Where codegen writes the translation unit for `file`.

    `build_output/<file_stem>.c`, relative to the process's working directory
    (src/codegen/mod.rs:3650-3655). Derived from the BASENAME, so two inputs
    with the same stem share it — every caller must clear it first, or a
    previous run's C answers for this one.
    """
    base = Path(cwd) if cwd else Path.cwd()
    return base / "build_output" / (Path(file).stem + ".c")


def clear_emitted_c(file: str, cwd=None) -> Path:
    tu = emitted_c_path(file, cwd)
    try:
        tu.unlink()
    except FileNotFoundError:
        pass
    return tu


# pdc's exit code, on `fix/gcc-diagnostics-discarded` (aa63982). Read from that
# branch's src/linker.rs:247-261 and its LinkError variants, not from a summary:
#
#   3 EXIT_BACKEND_REJECT      LinkError::GccFailed  — gcc ran to completion and
#                              exited nonzero: it REFUSED the translation unit.
#   4 EXIT_BACKEND_ILL_TYPED   LinkError::IllTypedC  — gcc exited 0 and diagnosed
#                              C that pdc generated. An ICE; no Palladium program
#                              asks for ill-typed C. Also a compiler defect.
#   5 EXIT_TOOLCHAIN           LinkError::Toolchain | GccDied — gcc could not be
#                              spawned, or was killed by a signal. It never
#                              reached a verdict, so nothing is established
#                              about the C and nothing may be claimed about it.
#
# AN EXIT CODE, NOT A STRING, and that is the whole point. `gcc compilation
# failed` was emitted for every unsuccessful gcc status, so it could not separate
# "gcc refused our C" from "gcc died" — and a fixture's own text can reach a log,
# where an exit code has no route from fixture text at all. Today's pdc exits 1
# for all of these, which is UNRESOLVED and under-claims by design.
BACKEND_REJECT_CODES = (3, 4)
BACKEND_TOOLCHAIN_CODE = 5
# Structured rejections are CONCLUDED experiments, not malfunctions. Without
# them in reject_codes the first fixture to exit 3 would be reported by
# `make stdlib-gate` as "pdc MALFUNCTIONED", turning a real backend defect into
# an accusation that the harness broke — this repo's recurring defect in a new
# place. Nothing in the corpus reaches these codes today (measured on the
# sibling branch: 191 .pd files, zero verdict changes), so this is forward
# compatibility, not a live change.
PDC_REJECT_CODES = (1, 3, 4, 5)


BACKEND_VERDICTS = ("BACKEND_REJECT", "BACKEND_UNRESOLVED")


def backend_verdict(rc: int) -> str:
    """BACKEND_REJECT only when the exit code says gcc reached a judgement."""
    return "BACKEND_REJECT" if rc in BACKEND_REJECT_CODES else "BACKEND_UNRESOLVED"


def cmd_pdc_verdict(args) -> int:
    tu = clear_emitted_c(args.file)
    res = run_and_classify([args.pdc, "compile", args.file, "-o", args.out],
                           reject_codes=PDC_REJECT_CODES)
    if isinstance(res, Malfunction):
        return report_malfunction(f"pdc on {args.file}", res, args.spill)
    if res.succeeded:
        emit(outcome="success", verdict="COMPILE_OK")
        print("BLOCKER -")
        return EXIT_OK

    errs = error_lines(res.text)
    no_main = [e for e in errs if "No main function found" in e]
    others = [e for e in errs if "No main function found" not in e]
    if tu.is_file():
        # The front end ACCEPTED this file and the build failed anyway. Was
        # LINK_FAIL, a verdict stdlib/MANIFEST.tsv could pin as a file's
        # expected state. It is now one of BACKEND_VERDICTS, which no manifest
        # row can equal, so the outcome fails whatever is declared.
        verdict = backend_verdict(res.rc)
    elif no_main and not others:
        verdict = "ACCEPTED_NO_MAIN"
    elif "gcc compilation failed" in strip_ansi(res.text):
        # No translation unit, yet the log says gcc ran. Those cannot both be
        # true; refusing beats filing it under COMPILE_FAIL, which IS pinnable.
        verdict = "BACKEND_UNRESOLVED"
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
    tu = clear_emitted_c(args.file, args.cwd)
    res = run_and_classify([args.pdc, "compile", args.file, "-o", args.out],
                           cwd=args.cwd, env=env, reject_codes=PDC_REJECT_CODES)
    if isinstance(res, Malfunction):
        return report_malfunction(f"pdc on {args.file}", res, args.spill)
    if res.succeeded:
        emit(outcome="accepted")
        return EXIT_OK

    plain = strip_ansi(res.text)
    first = next(iter(error_lines(res.text)), "")
    if tu.is_file():
        # Never `rejected-as-expected`, whatever --expect-stage says: the caller
        # is not permitted to declare that the C this compiler emits does not
        # compile. `link` is no longer an offered choice either, so there is no
        # spelling that reaches this outcome deliberately.
        emit(outcome="backend-reject", stage="backend",
             verdict=backend_verdict(res.rc),
             reason=("pdc accepted this file and the build then failed: the C at "
                     f"{tu} is the compiler's own output. A caller may not pin this "
                     "as an expectation — fix the backend."))
        if first:
            print(f"DIAG {first}")
        return EXIT_OK
    if "gcc compilation failed" in plain:
        emit(outcome="backend-reject", stage="backend", verdict="BACKEND_UNRESOLVED",
             reason=(f"the log says gcc ran, but no translation unit is at {tu}; "
                     "those cannot both be true"))
        if first:
            print(f"DIAG {first}")
        return EXIT_OK
    stage = "compile"
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
    """Import the structural analyser. Any failure RAISES, and main() maps it.

    `GATE_PROBE_NET_A` overrides the path, and exists for ONE reason: the fault
    injections in scripts/test-gate-probe.sh must be able to point this at a
    missing, unparsable or entry-point-less analyser. They used to do that by
    MOVING the tracked scripts/check-c-returns.py into a temporary directory and
    overwriting its path — so an interrupt inside that window left the trap to
    delete the temporary directory AND the only copy of the analyser, with the
    tracked file missing or a two-line stub. Injecting a path touches nothing
    that is checked in.
    """
    path = Path(os.environ.get("GATE_PROBE_NET_A") or (HERE / "check-c-returns.py"))
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
    visited = 0
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
        visited += 1
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

        res = run_and_classify(
            [args.cc, "-fsyntax-only", "-Werror=return-type", "-I", args.runtime, path])
        if isinstance(res, Malfunction):
            print(f"HARNESS {path}: Net B — {args.cc} {res.how}, so it proves nothing here")
            harness += 1
            # A capture that never reached EOF costs a descriptor that this
            # process cannot reclaim. Marching on through an unbounded file list
            # accumulates one per file, so this stops: the run is already a
            # malfunction, and the remaining files would be analysed by a
            # harness in a worse state than the one that just failed.
            if "did not reach EOF" in res.how:
                print(f"HARNESS {path}: stopping — a capture was abandoned, and "
                      f"each one costs a descriptor this process cannot reclaim")
                break
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

    # REQUESTED, VISITED, UNVISITED — because the loop can stop early (an
    # abandoned capture costs a descriptor this process cannot reclaim, so it
    # does not march on through the rest of the list). Reporting the requested
    # count as though every file had been analysed is a denominator that
    # overstates coverage, which is the defect this whole reader was rebuilt to
    # remove. The malfunction already prevents a false green; this makes the
    # number honest as well.
    unvisited = len(args.files) - visited
    print(f"ANALYSED {recognised_total} function definition(s) in {visited} of "
          f"{len(args.files)} requested file(s)"
          + (f"; {unvisited} NOT VISITED" if unvisited else ""))
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

    # ZERO UNSUPPORTED BUILTINS IS A LEGITIMATE STATE, AND USED NOT TO BE.
    # This read "no unsupported builtin could be extracted -> the parsing contract
    # broke", which was true while the registry always had some. On 2026-08-23 the
    # last two were repaired and the set went empty, and the gate reported a
    # MALFUNCTION about a tree that was in the best state it has ever been in. The
    # discriminator is not "did we find names" but "did we find BLOCKS": a parser
    # that cannot see the table finds no blocks at all, and that is still a
    # malfunction.
    blocks = re.findall(r"Builtin\s*\{.*?\n    \}", src, re.S)
    if not blocks:
        emit(outcome="malfunction",
             reason=f"{args.src} has the Support type but NO `Builtin {{ … }}` entry could be "
                    "parsed at all — the parsing contract broke")
        return EXIT_MALFUNCTION

    names = set()
    unsupported_blocks = 0
    for block in blocks:
        if "Support::Unsupported" in block:
            unsupported_blocks += 1
            m = re.search(r'name:\s*"([a-z_0-9]+)"', block)
            if m:
                names.add(m.group(1))
    # A block SAYS it is unsupported and we could not read its name: the reader is
    # out of step with the registry, which is a malfunction and not an empty set.
    # This is the half of the discriminator that survives once every builtin is
    # callable — "no names" alone stopped meaning "parser broke" on 2026-08-23.
    # EQUALITY, NOT "AT LEAST ONE". Every block that says `Support::Unsupported`
    # must yield exactly one name; if the counts differ, some entry was silently
    # omitted and the manifest was reconciled against a SUBSET with nobody told.
    #
    # This read `if unsupported_blocks and not names`, which one readable entry
    # satisfies beside any number of malformed ones. That is the same EXISTENCE
    # shape as the bug it replaced — "no names means the parser broke", written
    # when the registry always had unsupported builtins, which reported a
    # malfunction on 2026-08-23 when the set legitimately went empty. Twice now a
    # completeness question has been answered with an existence check.
    if unsupported_blocks != len(names):
        emit(outcome="malfunction",
             reason=f"{args.src} has {unsupported_blocks} `Support::Unsupported` "
                    f"entr{'y' if unsupported_blocks == 1 else 'ies'} but "
                    f"{len(names)} name(s) could be parsed from them — the "
                    "reconciliation would have run against a subset")
        return EXIT_MALFUNCTION
    if not names:
        arr = re.search(r"PRELUDE_TYPE_MISMATCHES[^=]*=\s*&\[(.*?)\];", src, re.S)
        if arr:
            names.update(re.findall(r'"([a-z_0-9]+) (?:param|return)', arr.group(1)))
    if not names:
        # Nothing to reconcile, and that is the finding rather than a failure: the
        # manifest cannot disagree with an empty set. Reported as its own outcome
        # so a green line never says "N unsupported builtin(s) checked" about zero.
        emit(outcome="none-unsupported", analysed=len(blocks))
        return EXIT_OK

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
    # `link` is gone as a CHOICE, not merely unused: it was the spelling that
    # let a caller declare "gcc is expected to refuse the C we emit" and be
    # answered `rejected-as-expected`. argparse now refuses it at parse time,
    # which is the same shape as scripts/conformance.sh refusing stage=link in
    # its manifest — the exemption cannot be reopened by writing a value.
    # Enumerated before removal, so the compatibility cost is measured and not
    # assumed. Every caller in the tree at fb12f6f:
    #   scripts/stdlib-gate.sh:268        literal `compile`
    #   scripts/stdlib-gate.sh:473        `$stage` from tests/stdlib/BUILTINS.tsv
    #                                     column 3, for status=UNUSABLE rows —
    #                                     of which there are ZERO today, so no
    #                                     live value flows through it
    #   scripts/test-gate-probe.sh:117    literal `compile`
    #   scripts/test-gate-probe.sh:122    literal `link` — the one real user, a
    #                                     control whose assertion is about the
    #                                     SIGKILL malfunction path, which
    #                                     short-circuits before any stage is
    #                                     classified. Updated with this change.
    b.add_argument("--expect-stage", choices=["compile"])
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
