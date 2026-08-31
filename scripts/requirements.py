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

THE EXIT CODE OF THIS FILE IS NOT THE EXIT CODE OF `make m2-exit`, and Make is
why: it maps every nonzero recipe status to 2. `scripts/m2-exit.sh` aggregates
the four inventories, keeps the tri-state, and prints it on its last line as

    M2_EXIT_RESULT <code> <name>

which survives the Make layer intact. Measured before that script existed:
`REQ_MILESTONE=M2 python3 scripts/requirements.py` exited 1 (OWED) while
`make m2-exit` exited 2 — which in this repo's own vocabulary says NO_VERDICT.
That is not lossy, it is wrong: the truth was a measurement and the code said
"nothing may be inferred". See the consumer contract in scripts/m2-exit.sh.

Env:
  REQ_MILESTONE   the milestone under test (M1..M9, `M3-start`, `P1`, or
                  `unscheduled`). REQUIRED — unset is NO_VERDICT, never green,
                  because a filter with no subject clears everything.

Usage:
  scripts/requirements.py                    # REQ_MILESTONE=<m>, the gate
  scripts/requirements.py --manifest PATH    # read PATH instead (self-test only;
                                             # an ARGUMENT, so an exported
                                             # variable cannot redirect the real
                                             # milestone exit)
  scripts/requirements.py --check-ledger     # MILESTONES.md's counts vs this file
  scripts/requirements.py --self-test
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

# THE OWNERSHIP ROSTER, PINNED. Deleting a row, or retagging it to `-` or to a
# milestone that has already shipped, removes it from every milestone filter —
# and the three declared-failure inventories stay clean, because a requirement
# nobody started produces no red test to notice its absence. That is the exact
# hole inventory four exists to close, and without this pin it existed INSIDE
# inventory four.
#
# NOTHING ELSE PINNED IT. `scripts/thesis_exit.py::EXPECTED_THESIS_CONTRACT`
# pins the 26 `thesis` rows and pins (kind, evidence, fingerprint) — not the
# milestone column, and not the other 167 rows. Measured WHEN THIS PIN WAS
# WRITTEN: all 46 M2 rows carried `disposition = 1.0`, so every one of them was
# unpinned. Both halves of that sentence have moved since — M2 owns 49 rows now
# and N3-13 left for M5 — and it is left in the past tense rather than
# re-measured into the comment, because the ARGUMENT is about what the thesis
# contract does not cover and that has not changed. The live count is whatever
# `MILESTONE_ROSTER` below says, which the self-test reconciles against the
# manifest on every run; a number in a comment is exactly the thing this file
# exists to stop people trusting.
#
# This is the same reviewed cross-check `thesis-exit.sh` describes about its own
# contract copy, and it buys the same thing: moving a requirement between
# milestones is a TWO-FILE edit that a reviewer can see, rather than one word in
# one column. `scripts/test-xfail.py` already states that principle for the
# `#[ignore]` owner; the requirement manifest had no second place to agree with.
MILESTONE_ROSTER = {
    "-": (
        "GI-01", "GI-02", "GI-03", "GI-04", "GI-05", "GI-07", "N12-01",
        "N12-02", "N13-01", "N13-02", "N2-01", "N2-02", "N2-05", "N2-06",
        "N2-07", "N3-01", "N3-04", "N3-07", "N4-01", "N4-03", "N4-05",
        "N4-06", "N4-07", "N4-09", "N4-20", "N5-01", "N5-10", "N5-11",
        "N6-01", "N6-04", "SH-01",
    ),
    "M2": (
        "GI-06", "GI-08", "GI-09", "GI-12", "N13-03", "N14-01", "N14-02",
        "N14-04", "N2-03", "N2-04", "N2-08", "N2-09", "N2-10", "N2-11",
        "N3-02", "N3-03", "N3-05", "N3-09", "N3-10", "N3-12",
        "N14-17", "N3-14", "N3-15", "N4-02", "N4-04", "N4-10", "N4-12",
        "N4-22", "N4-23", "N5-03", "N5-04",
        "N5-05", "N5-06", "N5-07", "N5-12", "N5-13", "N5-14", "N5-15",
        "N5-16", "N5-17", "N6-02", "N6-03", "N6-05", "N6-07", "N6-08",
        "N6-09", "N6-10", "N6-11", "WT-01",
    ),
    "M3": (
        "N10-01", "N10-02", "N10-03", "N10-04", "N10-05", "N10-06", "N10-07",
        "N10-08", "N10-09", "N10-10", "N10-11", "N14-03", "N3-06", "N3-08",
        "N4-11", "N4-14", "N4-15", "N4-16", "N4-18", "N4-19", "N4-21",
        "N5-08", "N5-09", "N6-06",
    ),
    "M3-start": (
        "GI-11",
    ),
    "M4": (
        "N11-01", "N11-02", "N11-03", "N11-04", "N11-05", "N11-06", "N11-07",
        "N3-11",
    ),
    "M5": (
        # N3-13 (macro hygiene) arrived here from M2 on 2026-08-26. The move is
        # a two-file edit ON PURPOSE — see the manifest header for the two
        # capture measurements that decided it — and this half is the one that
        # makes it visible to a reviewer reading code rather than a column.
        "N3-13",
        "N12-07", "N14-05", "N5-02", "N7-01", "N7-02", "N7-03", "N7-04",
        "N7-05", "N7-06", "N7-07", "N7-08", "N7-09", "N7-10", "N7-11",
        "N7-12",
    ),
    "M6": (
        "N8-01", "N8-02", "N8-03", "N8-04", "N8-05", "N8-06", "N8-07",
        "N8-08", "N8-09", "N8-10", "N8-11", "N8-12",
    ),
    "M7": (
        "D3-01", "D4-01", "N12-03", "N12-04", "N12-05", "N12-06", "N12-08",
        "N12-09", "N4-08", "N4-13", "N9-01", "N9-02", "N9-03", "N9-04",
        "N9-05", "N9-06", "N9-07", "N9-08", "N9-09",
    ),
    "M8": (
        "FFI-01", "FFI-02", "FFI-03", "N14-06", "N14-07", "N14-08", "N14-09",
        "N14-10", "N14-11", "N14-12", "N14-13", "N14-14", "N14-15", "N14-16",
        "N4-17",
    ),
    "M9": (
        "D1-01", "GI-10", "N1-01", "N1-02", "N1-03", "SH-02", "SH-03",
        "SH-04", "SH-05", "TH-01", "TH-02", "TH-03", "TH-04", "TH-05",
        "TH-06", "WT-02",
    ),
    "P1": (
        "D2-01", "N7-13", "N7-14", "N7-15", "N7-16", "N7-17",
    ),
}

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

        # ALL NINE COLUMNS ARE MANDATORY, AND `-` IS HOW YOU SAY "N/A". The
        # manifest's header calls them mandatory; the first version of this
        # parser checked the SHAPE (nine fields) and then only looked at four of
        # them, so a blank `source`, `requirement` or `fingerprint` passed. A
        # nine-column row three of whose columns are empty is not a row that
        # satisfies a mandatory-column rule — it is a row that was never filled
        # in, and the rule existed to catch exactly that.
        for col_no, (col_name, value) in enumerate(
                (("id", rid), ("milestone", milestone), ("source", source),
                 ("requirement", req), ("evidence-kind", kind),
                 ("evidence", evidence), ("status", status),
                 ("disposition", disp), ("fingerprint", fp)), 1):
            if not value.strip():
                errors.append("%s:%d: %s has an EMPTY %s (column %d); every "
                              "column is mandatory and `-` is how a row says "
                              "N/A" % (path, lineno, rid or "<no id>",
                                       col_name, col_no))

        # A thesis reject row with no fingerprint is satisfied by ANY rejection,
        # including one for incidental unsupported syntax. The manifest's own
        # header says so; this is that sentence as a check.
        #
        # `in ("", "-")` AND NOT `== "-"`: the first version tested for the
        # SENTINEL and not for ABSENCE, so a thesis reject row with an empty
        # fingerprint — the same defect, spelled with nothing instead of with a
        # dash — walked straight through the check written to stop it.
        if kind == "reject" and disp == "thesis" and fp.strip() in ("", "-"):
            errors.append("%s:%d: %s is a thesis `reject` row with no diagnostic "
                          "fingerprint — any rejection would satisfy it"
                          % (path, lineno, rid))

        # A row owed to nobody. `-` in the milestone column means "no milestone
        # owes this", and that is only true of a row already `satisfied`: an
        # `owed` or `blocked` row tagged `-` matches NO milestone filter, so it
        # vanishes from every milestone exit — while the three declared-failure
        # inventories stay clean, because a requirement nobody started produces
        # no red test. That is the unowned-requirement hole inventory four exists
        # to close, and it was open inside inventory four.
        if milestone == "-" and status in STATUSES and status != "satisfied":
            errors.append("%s:%d: %s is `%s` but owed to NOBODY (milestone `-`). "
                          "An unsatisfied row with no milestone matches no exit "
                          "criterion and can never be reported as outstanding; "
                          "give it an owner, or say why it is satisfied"
                          % (path, lineno, rid, status))
        rows.append(row)
    if not rows and not errors:
        raise ManifestError("%s parsed to zero rows — the manifest is empty or "
                            "its shape changed; nothing was established" % path)
    return rows, errors


def roster_drift(rows):
    """-> list of strings naming every id whose ownership left the pin.

    Named rather than digested. A hash would say "something moved" and leave the
    reviewer to find it, and the whole point of a two-file edit is that the diff
    is legible.
    """
    actual = {}
    for r in rows:
        actual[r["id"]] = r["milestone"]
    pinned = {rid: m for m, ids in MILESTONE_ROSTER.items() for rid in ids}

    drift = []
    for rid in sorted(set(pinned) - set(actual)):
        drift.append("%s is pinned to %s and is NOT IN THE MANIFEST — a row that "
                     "leaves the inventory leaves every milestone filter with it"
                     % (rid, pinned[rid]))
    for rid in sorted(set(actual) - set(pinned)):
        drift.append("%s is in the manifest owned by %s and is NOT IN THE ROSTER "
                     "— a new requirement needs an owner recorded in two places"
                     % (rid, actual[rid]))
    for rid in sorted(set(actual) & set(pinned)):
        if actual[rid] != pinned[rid]:
            drift.append("%s is RETAGGED: roster says %s, manifest says %s"
                         % (rid, pinned[rid], actual[rid]))
    return drift


# ---------------------------------------------------------------------------
# The status ledger (--check-ledger)
#
# docs/contributing/MILESTONES.md states counts over this manifest in prose and
# in a table, and every one of them was stale: the disposition table totalled 192
# against 193 rows, and the status line had been hand-corrected once already this
# round. A hand-written count in a document about anti-drift instrumentation is
# the drift, and there is no reason for it to be hand-written now that the file
# it counts is machine-readable.
#
# So the numbers are DERIVED here and the document is required to agree. What
# this does NOT cover is named in `report_ledger`: figures that are receipts of
# other gates (how many Rust tests passed) cannot be derived from a file, and
# saying so is cheaper than implying they are checked.
# ---------------------------------------------------------------------------

LEDGER = ROOT / "docs/contributing/MILESTONES.md"

# The three ids that carry `disposition = thesis` without being SCORED rows: the
# aggregate and the two preconditions. Held here because MILESTONES.md states the
# scored count in prose and nothing could otherwise derive it.
THESIS_UNSCORED = ("D1-01", "GI-11", "GI-12")


def ledger_claims(rows):
    """-> list of (label, regex, expected) the ledger must state.

    EVERY figure in MILESTONES.md that is a count over this manifest is here.
    The first version governed five patterns chosen by hand, and the reviewer's
    finding was exact: the document went on saying "25 `thesis` rows / 22 scored"
    in two other places while the gated disposition table said 26 / 23, and a
    checker for derived figures that leaves contradictory derived figures green
    is not yet a checker. The governed set is now the whole class, and the check
    prints it.

    Each regex must match exactly once. A regex that stops matching is a FAILURE
    and never a skip: the sentence being rewritten is precisely when the number
    inside it goes unchecked.
    """
    total = len(rows)
    status = {s: sum(1 for r in rows if r["status"] == s) for s in STATUSES}
    disp = {d: sum(1 for r in rows if r["disposition"] == d) for d in DISPOSITIONS}
    scored = sum(1 for r in rows
                 if r["disposition"] == "thesis" and r["id"] not in THESIS_UNSCORED)
    owned = lambda m: sum(1 for r in rows if r["milestone"] == m)
    owed_by = lambda m: sum(1 for r in rows
                            if r["milestone"] == m and r["status"] != "satisfied")

    claims = [
        ("row count + status breakdown (prose)",
         r"\*\*(\d+) rows, (\d+) satisfied · (\d+) owed · (\d+) blocked\*\*",
         (total, status["satisfied"], status["owed"], status["blocked"])),
        ("row count + status breakdown (status table)",
         r"\| (\d+) satisfied · (\d+) owed · (\d+) blocked, over (\d+) rows \|",
         (status["satisfied"], status["owed"], status["blocked"], total)),
        ("disposition `thesis` (table)",
         r"\| `thesis` \| (\d+) = (\d+) scored ", (disp["thesis"], scored)),
        ("disposition `1.0`", r"\| `1\.0` \| (\d+) \|", (disp["1.0"],)),
        ("disposition `post-1.0`",
         r"\| `post-1\.0` \| (\d+) \|", (disp["post-1.0"],)),
        # The two sites the first version of this check did not govern, and which
        # contradicted the table above for a full round.
        ("thesis rows, stated in the opening section",
         r"two of its (\d+) `thesis` rows", (disp["thesis"],)),
        ("thesis rows + scored, stated under M9",
         r"(\d+) rows across the manifest carry `disposition = thesis` —\n(\d+) scored,",
         (disp["thesis"], scored)),
        ("evaluated rows, stated in the opening section",
         r"\(1 of (\d+) evaluated rows would pass\)", (scored,)),
        # ADDED AFTER IT DRIFTED. This sentence is a count over this manifest and
        # was not governed, so it went on saying 8 while the manifest held 3 and
        # then 2 — the exact class the docstring above says the governed set is
        # supposed to be the whole of. Found while closing GI-08; the number it
        # states is `m2-exit`'s own OWED_TO_M2 line count, which is owed_by(M2).
        ("M2's outstanding row count, stated in item 8",
         r"It reports (\d+) rows `OWED_TO_M2`", (owed_by("M2"),)),
    ]
    # Per-milestone ownership, every milestone that states one. These were never
    # governed and three of them were already wrong.
    for m, pattern in (
        ("M2", r"\*\*Owns (\d+) requirement rows, (\d+) of them still owed\*\*"),
        ("M3", r"\*\*Owns (\d+) requirement rows\*\* and the 18 `#\[ignore\]` rows"),
        ("M4", r"\*\*Owns (\d+) requirement rows\*\* — N3-11 and N11-01"),
        ("M5", r"\*\*Owns (\d+) requirement rows\*\*, the five `#\[ignore\]` rows"),
        ("M6", r"\*\*Owns (\d+) requirement rows\.\*\* It owns no `#\[ignore\]` row"),
        ("M7", r"\*\*Owns (\d+) requirement rows\*\* and two of the owner's decisions"),
        ("M8", r"\*\*Owns (\d+) requirement rows\.\*\* What the library needs"),
        ("M9", r"\*\*Owns (\d+) requirement rows\*\*; \d+ rows across the manifest"),
    ):
        want = (owned(m), owed_by(m)) if m == "M2" else (owned(m),)
        claims.append(("%s ownership" % m, pattern, want))
    return claims


def report_ledger(rows, out=sys.stdout):
    w = out.write
    try:
        text = LEDGER.read_text(errors="replace")
    except OSError as exc:
        w("NO_VERDICT: cannot read %s: %s\n" % (LEDGER, exc))
        w("\nREQUIREMENTS_RESULT 2 NO_VERDICT\n")
        return EXIT_NO_VERDICT

    problems = []
    checked = 0
    for label, pattern, expected in ledger_claims(rows):
        found = re.findall(pattern, text)
        if len(found) != 1:
            problems.append(
                "%s: the sentence this is checked in matched %d times, not once "
                "(/%s/). A rewritten sentence is when the number inside it stops "
                "being checked, so this is a failure and not a skip."
                % (label, len(found), pattern))
            continue
        got = tuple(int(g) for g in
                    (found[0] if isinstance(found[0], tuple) else (found[0],)))
        checked += 1
        if got != expected:
            problems.append("%s: %s states %s, the manifest has %s"
                            % (label, LEDGER.name, got, expected))

    w("==============================================\n")
    w("status ledger: %d of %d derived claim(s) in %s agree with %s\n"
      % (checked - len([p for p in problems if ": " in p and "states" in p]),
         len(ledger_claims(rows)), LEDGER.name, DEFAULT_MANIFEST))
    w("NOT CHECKED HERE: every figure in that document that is a RECEIPT of\n")
    w("another gate rather than a count over this file — how many Rust tests\n")
    w("passed, how many self-test cases ran, what conformance verified. Those\n")
    w("cannot be derived from a tracked file and are not covered by this check.\n")
    w("==============================================\n")

    if problems:
        w("\nthe status ledger disagrees with the manifest it describes:\n")
        for p in problems:
            w("  %s\n" % p)
        w("\nREQUIREMENTS_RESULT 1 OWED\n")
        return EXIT_OWED
    w("\nREQUIREMENTS_RESULT 0 CLEAR\n")
    return EXIT_CLEAR


def report(rows, errors, milestone, manifest, out=sys.stdout, check_roster=False):
    """-> exit code. Prints the whole finding, never just the first."""
    w = out.write

    owed = [r for r in rows if r["milestone"] == milestone
            and r["status"] != "satisfied"]
    mine = [r for r in rows if r["milestone"] == milestone]
    # Only the real manifest has a roster. A planted one is a fixture, and
    # comparing it against the roster would test the fixture.
    drift = roster_drift(rows) if check_roster else []

    w("==============================================\n")
    w("requirement inventory (%s): %d row(s)\n" % (manifest, len(rows)))
    w("milestone gate: REQ_MILESTONE=%s -> %d of %d row(s) owned by %s are not "
      "satisfied\n" % (milestone, len(owed), len(mine), milestone))
    if check_roster:
        w("ownership roster: %d id(s) pinned in scripts/requirements.py, %d "
          "drift(s)\n"
          % (sum(len(v) for v in MILESTONE_ROSTER.values()), len(drift)))
    w("NOT CHECKED HERE, and therefore not established by a green run:\n")
    for item in UNIMPLEMENTED:
        w("  - %s\n" % item)
    w("==============================================\n")

    if drift:
        w("\nOWNERSHIP DRIFT — the manifest and the pinned roster disagree about "
          "who owns what. Moving a requirement between milestones is a two-file "
          "edit on purpose:\n")
        for d in drift:
            w("  ROSTER %s\n" % d)

    if errors:
        w("\nthe manifest itself is malformed — the closed inventory cannot be "
          "read:\n")
        for e in errors:
            w("  %s\n" % e)

    if drift or errors:
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
        # EVERY column is mandatory. The first parser checked the nine-field
        # SHAPE and then read four of them, so three of these five passed.
        ("empty evidence", "X-01\tM2\tN1\tr\tfixture\t \towed\t1.0\t-\n",
         "has an EMPTY evidence (column 6)"),
        ("empty source", "X-01\tM2\t \tr\tfixture\te\towed\t1.0\t-\n",
         "has an EMPTY source (column 3)"),
        ("empty requirement text", "X-01\tM2\tN1\t\tfixture\te\towed\t1.0\t-\n",
         "has an EMPTY requirement (column 4)"),
        ("empty fingerprint", "X-01\tM2\tN1\tr\tfixture\te\towed\t1.0\t\n",
         "has an EMPTY fingerprint (column 9)"),
        ("empty id", "\tM2\tN1\tr\tfixture\te\towed\t1.0\t-\n", "row has no id"),
        ("thesis reject with `-` fingerprint",
         "X-01\tM2\tN1\tr\treject\te\towed\tthesis\t-\n",
         "no diagnostic fingerprint"),
        # THE SENTINEL WAS NOT THE RULE. `fp == "-"` tested for the dash and not
        # for absence, so the same defect spelled with nothing walked through the
        # check written to stop it.
        ("thesis reject with a BLANK fingerprint",
         "X-01\tM2\tN1\tr\treject\te\towed\tthesis\t \n",
         "no diagnostic fingerprint"),
        # A row owed to nobody vanishes from every milestone filter.
        ("owed row tagged `-`", "X-01\t-\tN1\tr\tfixture\te\towed\t1.0\t-\n",
         "owed to NOBODY"),
        ("blocked row tagged `-`",
         "X-01\t-\tN1\tr\tdecision\te\tblocked\t1.0\t-\n", "owed to NOBODY"),
    ]:
        rc, out = verdict(body)
        check("malformed: " + label, (rc, needle in out), (EXIT_OWED, True))

    # 6b. ...and the control for the two rules above: the legitimate shapes.
    rc, out = verdict("X-01\t-\tN1\tr\tfixture\te\tsatisfied\t1.0\t-\n", milestone="M5")
    check("a SATISFIED row tagged `-` is fine — that is what `-` is for",
          "owed to NOBODY" in out, False)
    rc, out = verdict("X-01\tM2\tN1\tr\treject\te\towed\t1.0\t \n")
    check("a blank fingerprint is still an empty mandatory column",
          "has an EMPTY fingerprint" in out, True)

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

    # 11. THE OWNERSHIP ROSTER AGREES WITH THE REAL MANIFEST, id by id. Item 10
    #     only says the file parses; a deleted or retagged row parses perfectly.
    check("the pinned roster and the manifest agree, id by id",
          roster_drift(rows), [])
    check("the roster covers every row",
          sum(len(v) for v in MILESTONE_ROSTER.values()), len(rows))

    # 12. AND THE ROSTER CAN FAIL, in each of its three directions. A pin nobody
    #     has watched fail is the thing this whole file exists to complain about.
    _fake = [dict(r) for r in rows]
    _m2 = next(r for r in _fake if r["milestone"] == "M2" and r["status"] == "owed")
    _dropped = [r for r in _fake if r["id"] != _m2["id"]]
    check("deleting an owed M2 row is ROSTER drift",
          any(_m2["id"] in d and "NOT IN THE MANIFEST" in d
              for d in roster_drift(_dropped)), True)
    _retagged = [dict(r, milestone="-") if r["id"] == _m2["id"] else r for r in _fake]
    check("retagging an owed M2 row to `-` is ROSTER drift",
          any(_m2["id"] in d and "RETAGGED" in d
              for d in roster_drift(_retagged)), True)
    check("a brand-new id is ROSTER drift",
          any("ZZ-99" in d and "NOT IN THE ROSTER" in d for d in
              roster_drift(_fake + [dict(_m2, id="ZZ-99")])), True)

    # 13. THE STATUS LEDGER. Derived, and the document has to agree.
    import io as _io
    _buf = _io.StringIO()
    check("the status ledger agrees with the manifest",
          report_ledger(rows, out=_buf), EXIT_CLEAR)
    check("...and it can fail: a wrong count is named",
          report_ledger([r for r in rows if r["id"] != _m2["id"]],
                        out=_io.StringIO()), EXIT_OWED)

    # 14. WIRING, AND THIS IS A SHAPE CHECK — SAID OUT LOUD. `make -n` proves a
    #     recipe exists and names this file; it CANNOT prove the file ran, and a
    #     recipe of `echo 'python3 scripts/requirements.py'` would satisfy it.
    #     The effect-level control is scripts/test-requirements-runner.sh, which
    #     runs `make m2-exit` for real and requires each inventory's own output,
    #     with counts it recomputes independently. Both layers are kept for
    #     src/builtins.rs's reason: the shape check localises the fault, the
    #     behavioural one proves the fault would be visible.
    try:
        dry = subprocess.run(["make", "-n", "m2-exit"], cwd=str(ROOT),
                             stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                             text=True, timeout=120)
        text = dry.stdout
    except (OSError, subprocess.SubprocessError) as exc:
        text = ""
        failures.append("make -n m2-exit did not run: %s" % exc)
    check("make m2-exit has a recipe and it runs the aggregator (SHAPE ONLY)",
          "scripts/m2-exit.sh" in text, True)

    if failures:
        print("self-test FAILED:", file=sys.stderr)
        for f in failures:
            print("  " + f, file=sys.stderr)
        return False
    print("self-test: 31 checks green (owed and blocked are RED and named, the "
          "owner filter is a filter, all-satisfied is NO_VERDICT rather than "
          "green because steps 3 and 4 did not run, every structural column "
          "check incl. mandatory-column emptiness, the thesis-reject fingerprint "
          "rule against BOTH `-` and blank, and a row owed to nobody; empty and "
          "absent manifests refused; the real manifest parses clean AND matches "
          "the pinned ownership roster, whose three drift directions are each "
          "proved to fail; the derived status ledger agrees with MILESTONES.md "
          "and is proved able to disagree; and `make m2-exit` has a recipe that "
          "names the aggregator — a SHAPE check whose effect-level control is "
          "scripts/test-requirements-runner.sh)")
    return True


def main():
    os.chdir(str(ROOT))

    if not self_test():
        print("\nthe gate's own parser or wiring is broken; not running it",
              file=sys.stderr)
        return EXIT_NO_VERDICT
    if "--self-test" in sys.argv:
        return EXIT_CLEAR

    if "--check-ledger" in sys.argv:
        try:
            rows, _ = read_manifest(DEFAULT_MANIFEST)
        except ManifestError as exc:
            print("NO_VERDICT: %s" % exc, file=sys.stderr)
            print("REQUIREMENTS_RESULT 2 NO_VERDICT")
            return EXIT_NO_VERDICT
        return report_ledger(rows)

    # THE MANIFEST PATH IS AN ARGUMENT AND NOT AN ENVIRONMENT VARIABLE, and that
    # is the fix for a hole the first version had: with `REQUIREMENTS_MANIFEST`
    # read from the environment, `REQUIREMENTS_MANIFEST=/dev/null make m2-exit`
    # redirected the milestone's own exit criterion at a file of the caller's
    # choosing, and the assertion that the Makefile does not set it could not see
    # that. An argument cannot be injected by an exported variable, so "m2-exit
    # reads the real manifest" is now structural rather than asserted.
    manifest = DEFAULT_MANIFEST
    for i, arg in enumerate(sys.argv[1:], 1):
        if arg == "--manifest":
            if i + 1 >= len(sys.argv):
                print("NO_VERDICT: --manifest needs a path", file=sys.stderr)
                print("REQUIREMENTS_RESULT 2 NO_VERDICT")
                return EXIT_NO_VERDICT
            manifest = sys.argv[i + 1]
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

    return report(rows, errors, milestone, manifest,
                  check_roster=(manifest == DEFAULT_MANIFEST))


if __name__ == "__main__":
    sys.exit(main())
