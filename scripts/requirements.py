#!/usr/bin/env python3
"""THE THIRD OWNER INVENTORY: docs/contributing/1.0-requirements.tsv, as a command.

WHY THIS FILE EXISTS (GI-08). `make m1-exit` reads three inventories — .pd
fixtures, the Rust debt manifest, and the ordinary Rust suite — and every one of
them is a register of *declared failures*. That is a real check and it is not a
milestone's exit criterion, because a declared failure is a PROXY: it exists only
where somebody already wrote a red test. A requirement nobody has started on
produces no xfail row, no `#[ignore]`, and no failing test, so all three
inventories are clean about it and the milestone looks finished.

The requirement manifest is the inventory that does not have that hole: it
enumerates what the milestone OWES rather than what has already been observed to
break. GI-08 states the obligation in one line — "Every milestone exit reads BOTH
debt inventories and this manifest" — and this is the "and this manifest" half.

WHAT THIS IS NOT. It is not `make thesis-exit`, and it must never be read as a
completeness criterion for 1.0; see the header of the manifest itself, which says
at length why drawing the line on an inventory is the disease this repository
spent M1 burning out. This answers exactly one question: **does milestone
<REQ_MILESTONE> still owe a row of its own manifest.**

MACHINE CONTRACT — the exit code is three-valued, as scripts/thesis-exit.sh is,
and for the same reason: a gate that cannot distinguish "the answer is no" from
"I could not answer" has already lied once.

    0  CLEAR       the milestone owes nothing AND everything needed to say that ran
    1  OWED        it owes rows, or the manifest itself is malformed
    2  NO_VERDICT  the gate would not measure; nothing may be inferred

A milestone with no owed rows does NOT get a 0 today. Steps 3 and 4 of the
specification in docs/contributing/MILESTONES.md (resolve each evidence locator
and RUN it; reconcile both debt inventories by `req:` id) are NOT implemented
here, and "no row says owed" without them is a statement about the status column,
not about the code. So an all-satisfied milestone exits 2 and says which step is
missing. That is deliberate: this gate is allowed to be RED and allowed to
abstain, and is not allowed to be green for a reason it did not establish.

Env:
  REQ_MILESTONE          the milestone under test (M1..M9, `M3-start`, `P1`, or
                         `unscheduled`). REQUIRED — unset is NO_VERDICT, never
                         green, because a filter with no subject clears
                         everything.
  REQUIREMENTS_MANIFEST  path to the manifest. Exists for
                         scripts/test-requirements-runner.sh, which has to plant
                         rows in a copy; `make m2-exit` never sets it, and the
                         self-test asserts that.

Usage: scripts/requirements.py [--self-test]
"""

import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_MANIFEST = "docs/contributing/1.0-requirements.tsv"

EXIT_CLEAR = 0
EXIT_OWED = 1
EXIT_NO_VERDICT = 2

# The column vocabulary, from the manifest's own header. Held here as well so
# that a value nobody described cannot enter by being typed: an unknown status is
# a manifest error, not a row that silently matches no filter.
KINDS = {"fixture", "reject", "skip", "observable", "gate", "decision"}
STATUSES = {"satisfied", "owed", "blocked"}
DISPOSITIONS = {"thesis", "1.0", "post-1.0"}
COLUMNS = 9

# `-` is a real value in this column: 31 rows are owed to no milestone because
# they are already satisfied. `M3-start` and `P1` are in the manifest today.
MILESTONE_RE = re.compile(r"\A(M[1-9](?:-start)?|P1|unscheduled|-)\Z")

# The steps of the specification in docs/contributing/MILESTONES.md that this
# file does NOT do. Named in the output of every run, because a gate that is
# silent about its own scope is how "make m1-exit exits 0" came to mean
# something it did not.
UNIMPLEMENTED = [
    "step 3 — resolve each evidence locator by kind and RUN it "
    "(fixture/reject/skip via the conformance runner, observable via cargo, "
    "gate via make, decision against MILESTONES.md)",
    "step 4 — reconcile the Rust debt inventory by `req: <id>` tag; the "
    "`#[ignore]` reasons carry a milestone and not a requirement id, so that "
    "half is by review and not by command",
]


class ManifestError(Exception):
    pass


def read_manifest(path):
    """-> (rows, errors). A row is a dict; errors are strings.

    Both are returned: a malformed row does not stop the scan, because reporting
    the first defect and exiting costs a round trip to discover the rest.
    """
    try:
        text = Path(path).read_text(errors="replace")
    except OSError as exc:
        raise ManifestError("cannot read %s: %s" % (path, exc))

    rows, errors, seen = [], [], {}
    for lineno, line in enumerate(text.split("\n"), 1):
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        cols = line.split("\t")
        if len(cols) != COLUMNS:
            errors.append("%s:%d: %d tab-separated columns, expected %d: %.60s"
                          % (path, lineno, len(cols), COLUMNS, line))
            continue
        rid, milestone, source, req, kind, evidence, status, disp, fp = cols
        row = {"line": lineno, "id": rid, "milestone": milestone,
               "source": source, "requirement": req, "kind": kind,
               "evidence": evidence, "status": status, "disposition": disp,
               "fingerprint": fp}
        if not rid.strip():
            errors.append("%s:%d: row has no id" % (path, lineno))
        elif rid in seen:
            errors.append("%s:%d: duplicate id %s, first declared at line %d"
                          % (path, lineno, rid, seen[rid]))
        else:
            seen[rid] = lineno
        if not MILESTONE_RE.match(milestone):
            errors.append("%s:%d: %s has milestone %r, which is not M1..M9, "
                          "M3-start, P1, unscheduled or -"
                          % (path, lineno, rid, milestone))
        if kind not in KINDS:
            errors.append("%s:%d: %s has evidence-kind %r; known kinds are %s"
                          % (path, lineno, rid, kind, ", ".join(sorted(KINDS))))
        if status not in STATUSES:
            errors.append("%s:%d: %s has status %r; known statuses are %s"
                          % (path, lineno, rid, status, ", ".join(sorted(STATUSES))))
        if disp not in DISPOSITIONS:
            errors.append("%s:%d: %s has disposition %r; known dispositions are %s"
                          % (path, lineno, rid, disp, ", ".join(sorted(DISPOSITIONS))))
        if not evidence.strip():
            errors.append("%s:%d: %s names no evidence" % (path, lineno, rid))
        # A thesis reject row with no fingerprint is satisfied by ANY rejection,
        # including one for incidental unsupported syntax. The manifest's own
        # header says so; this is that sentence as a check.
        if kind == "reject" and disp == "thesis" and fp.strip() == "-":
            errors.append("%s:%d: %s is a thesis `reject` row with no diagnostic "
                          "fingerprint — any rejection would satisfy it"
                          % (path, lineno, rid))
        rows.append(row)
    if not rows and not errors:
        raise ManifestError("%s parsed to zero rows — the manifest is empty or "
                            "its shape changed; nothing was established" % path)
    return rows, errors


def report(rows, errors, milestone, manifest, out=sys.stdout):
    """-> exit code. Prints the whole finding, never just the first."""
    w = out.write

    owed = [r for r in rows if r["milestone"] == milestone
            and r["status"] != "satisfied"]
    mine = [r for r in rows if r["milestone"] == milestone]

    w("==============================================\n")
    w("requirement inventory (%s): %d row(s)\n" % (manifest, len(rows)))
    w("milestone gate: REQ_MILESTONE=%s -> %d of %d row(s) owned by %s are not "
      "satisfied\n" % (milestone, len(owed), len(mine), milestone))
    w("NOT CHECKED HERE, and therefore not established by a green run:\n")
    for item in UNIMPLEMENTED:
        w("  - %s\n" % item)
    w("==============================================\n")

    if errors:
        w("\nthe manifest itself is malformed — the closed inventory cannot be "
          "read:\n")
        for e in errors:
            w("  %s\n" % e)
        w("\nREQUIREMENTS_RESULT 1 OWED\n")
        return EXIT_OWED

    if not mine:
        w("\nNO_VERDICT: no row of %s is owned by %s. A filter whose subject "
          "matches nothing clears everything, so this is refused rather than "
          "reported as 'nothing owed'.\n" % (manifest, milestone))
        w("\nREQUIREMENTS_RESULT 2 NO_VERDICT\n")
        return EXIT_NO_VERDICT

    if owed:
        w("\nOWED_TO_%s — %s is not finished; these rows of its own manifest "
          "are not satisfied:\n" % (milestone, milestone))
        for r in owed:
            w("  OWED_TO_%s %s [%s] %s\n"
              % (milestone, r["id"], r["status"], r["requirement"][:110]))
            w("      evidence: %s %s\n" % (r["kind"], r["evidence"]))
        w("\nREQUIREMENTS_RESULT 1 OWED\n")
        return EXIT_OWED

    w("\nNO_VERDICT: every %s row is `satisfied`, but the steps listed above "
      "did not run, so no evidence was resolved. 'No row says owed' is a "
      "statement about the status column, not about the compiler.\n" % milestone)
    w("\nREQUIREMENTS_RESULT 2 NO_VERDICT\n")
    return EXIT_NO_VERDICT


# --------------------------------------------------------------------------
# Self-test: the parser and the verdict map, on planted inputs
#
# scripts/test-requirements-runner.sh drives the whole command end to end and is
# what GI-09 names. This is the layer below it: the pieces a shell test would
# have to reconstruct from output, checked directly.
# --------------------------------------------------------------------------

def self_test():
    import io
    import tempfile

    failures = []

    def check(label, got, want):
        if got != want:
            failures.append("%s\n     got:  %r\n     want: %r" % (label, got, want))

    HEAD = "# a planted manifest\n"
    GOOD = "X-01\tM2\tN1\tsomething\tfixture\ttests/x.pd\t%s\t1.0\t-\n"

    def write(body):
        fd, path = tempfile.mkstemp(suffix=".tsv")
        with os.fdopen(fd, "w") as f:
            f.write(HEAD + body)
        return path

    def verdict(body, milestone="M2"):
        path = write(body)
        try:
            rows, errors = read_manifest(path)
            buf = io.StringIO()
            rc = report(rows, errors, milestone, path, out=buf)
            return rc, buf.getvalue()
        finally:
            os.unlink(path)

    # 1. A planted owed row is RED and is NAMED.
    rc, out = verdict(GOOD % "owed")
    check("owed row is RED", (rc, "OWED_TO_M2 X-01" in out), (EXIT_OWED, True))

    # 2. `blocked` is not `satisfied`. A row that is a question is still owed.
    rc, out = verdict(GOOD % "blocked")
    check("blocked row is RED", (rc, "OWED_TO_M2 X-01" in out), (EXIT_OWED, True))

    # 3. The filter is a FILTER: another milestone's owed row does not trip it,
    #    and with no row of its own the answer is NO_VERDICT, not green.
    rc, out = verdict(GOOD % "owed", milestone="M5")
    check("another milestone does not trip it",
          (rc, "OWED_TO_M5" in out), (EXIT_NO_VERDICT, False))

    # 4. All satisfied is NOT green, because steps 3 and 4 did not run.
    rc, out = verdict(GOOD % "satisfied")
    check("all-satisfied is NO_VERDICT, not green",
          (rc, "NO_VERDICT" in out), (EXIT_NO_VERDICT, True))

    # 5. Every run says what it did not check, green or red.
    check("scope is always printed", "step 3 —" in out, True)

    # 6. Structural defects, one per check, all reported as manifest errors.
    for label, body, needle in [
        ("wrong column count", "X-01\tM2\tN1\tr\tfixture\te\towed\t1.0\n",
         "tab-separated columns"),
        ("unknown status", GOOD % "probably", "has status 'probably'"),
        ("unknown kind", "X-01\tM2\tN1\tr\tvibes\te\towed\t1.0\t-\n",
         "has evidence-kind 'vibes'"),
        ("unknown disposition", "X-01\tM2\tN1\tr\tfixture\te\towed\tsoon\t-\n",
         "has disposition 'soon'"),
        ("unknown milestone", "X-01\tM99\tN1\tr\tfixture\te\towed\t1.0\t-\n",
         "which is not M1..M9"),
        ("duplicate id", (GOOD % "owed") + (GOOD % "satisfied"), "duplicate id X-01"),
        ("empty evidence", "X-01\tM2\tN1\tr\tfixture\t \towed\t1.0\t-\n",
         "names no evidence"),
        ("thesis reject with no fingerprint",
         "X-01\tM2\tN1\tr\treject\te\towed\tthesis\t-\n",
         "no diagnostic fingerprint"),
    ]:
        rc, out = verdict(body)
        check("malformed: " + label, (rc, needle in out), (EXIT_OWED, True))

    # 7. A non-thesis reject row may leave the fingerprint at `-`: the rule is
    #    about thesis rows, and a check that fired on every reject row would be
    #    a different, unreviewed contract.
    rc, out = verdict("X-01\tM2\tN1\tr\treject\te\towed\t1.0\t-\n")
    check("non-thesis reject with `-` is not a manifest error",
          "no diagnostic fingerprint" in out, False)

    # 8. An empty manifest establishes nothing and must say so rather than
    #    reporting a clean scan of zero rows.
    path = write("")
    try:
        raised = None
        try:
            read_manifest(path)
        except ManifestError as exc:
            raised = str(exc)
    finally:
        os.unlink(path)
    check("empty manifest is refused", raised is not None and "zero rows" in raised, True)

    # 9. An unreadable manifest is a malfunction, not "no rows owed".
    raised = None
    try:
        read_manifest("/nonexistent/1.0-requirements.tsv")
    except ManifestError as exc:
        raised = str(exc)
    check("absent manifest is refused", raised is not None and "cannot read" in raised, True)

    # 10. THE REAL MANIFEST PARSES. Every check above runs on planted input, so
    #     all ten could be green while the file this gate exists to read has
    #     drifted out of the shape the parser assumes.
    rows, errors = read_manifest(ROOT / DEFAULT_MANIFEST)
    check("the real manifest is well-formed", errors, [])
    check("the real manifest has rows", len(rows) > 100, True)

    # 11. AND IT IS WIRED. A gate reachable only by someone who already knows to
    #     name it is a document — the argument this repo has now made three times
    #     (test-xfail, version-source-gate, test-honest). `make -n` is used
    #     rather than reading the Makefile as text: what matters is what Make
    #     would run.
    try:
        dry = subprocess.run(["make", "-n", "m2-exit"], cwd=str(ROOT),
                             stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                             text=True, timeout=120)
        text = dry.stdout
    except (OSError, subprocess.SubprocessError) as exc:
        text = ""
        failures.append("make -n m2-exit did not run: %s" % exc)
    check("make m2-exit runs scripts/requirements.py",
          "scripts/requirements.py" in text, True)
    check("make m2-exit names M2 as the subject", "REQ_MILESTONE=M2" in text, True)
    # The other three inventories, so that weakening m2-exit to the one this
    # file owns is caught HERE and not by a reviewer's memory.
    check("make m2-exit reads inventory one (.pd fixtures)",
          "CONFORMANCE_FORBID_OWNER=M2" in text, True)
    check("make m2-exit reads inventory two (Rust debt)",
          "TEST_XFAIL_FORBID_OWNER=M2" in text, True)
    check("make m2-exit reads inventory three (the ordinary Rust suite)",
          "test --release --no-fail-fast" in text, True)
    # And it must read the REAL manifest: REQUIREMENTS_MANIFEST is the
    # self-test's plant hook, and a milestone exit that set it would be
    # measuring a file of its own choosing.
    check("make m2-exit does not redirect the manifest",
          "REQUIREMENTS_MANIFEST" in text, False)

    if failures:
        print("self-test FAILED:", file=sys.stderr)
        for f in failures:
            print("  " + f, file=sys.stderr)
        return False
    print("self-test: 24 checks green (owed and blocked are RED and named, the "
          "owner filter is a filter, all-satisfied is NO_VERDICT rather than "
          "green because steps 3 and 4 did not run, every structural column "
          "check incl. the thesis-reject fingerprint rule and its non-thesis "
          "control, empty and absent manifests refused, the real manifest "
          "parses clean, and `make -n m2-exit` still reads all four "
          "inventories against M2 with no manifest redirect)")
    return True


def main():
    os.chdir(str(ROOT))

    if not self_test():
        print("\nthe gate's own parser or wiring is broken; not running it",
              file=sys.stderr)
        return EXIT_NO_VERDICT
    if "--self-test" in sys.argv:
        return EXIT_CLEAR

    manifest = os.environ.get("REQUIREMENTS_MANIFEST", "").strip() or DEFAULT_MANIFEST
    milestone = os.environ.get("REQ_MILESTONE", "").strip()

    if not milestone:
        print("NO_VERDICT: REQ_MILESTONE is unset. This gate answers 'does "
              "milestone X still owe a row', and with no X it would clear "
              "everything.", file=sys.stderr)
        print("REQUIREMENTS_RESULT 2 NO_VERDICT")
        return EXIT_NO_VERDICT
    if not MILESTONE_RE.match(milestone) or milestone == "-":
        # Fail closed, exactly as TEST_XFAIL_FORBID_OWNER does: a typo'd
        # milestone matches no row, and "no rows owed" would be a green run that
        # established nothing.
        print("NO_VERDICT: REQ_MILESTONE=%r is not a milestone (M1..M9, "
              "M3-start, P1, unscheduled)." % milestone, file=sys.stderr)
        print("REQUIREMENTS_RESULT 2 NO_VERDICT")
        return EXIT_NO_VERDICT

    try:
        rows, errors = read_manifest(manifest)
    except ManifestError as exc:
        print("NO_VERDICT: %s" % exc, file=sys.stderr)
        print("REQUIREMENTS_RESULT 2 NO_VERDICT")
        return EXIT_NO_VERDICT

    return report(rows, errors, milestone, manifest)


if __name__ == "__main__":
    sys.exit(main())
