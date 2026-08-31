#!/usr/bin/env python3
"""Evidence gate for the documentation. See scripts/check-doc-evidence.sh for why.

Three checks: citation pins, the no-compile allowlist, and feature-index evidence tags.

Design note on what a pin can and cannot prove. A fingerprint proves the cited range has
not MOVED. It cannot prove the range SUPPORTS the claim made about it — that is a reading,
and no machine does it. A review of 24 pinned targets found 21 supporting their claims and
3 not, so the gap is real and measured, not theoretical. Two things narrow it: the pin file
stores a readable excerpt beside the hash so a spot-check needs no source checkout, and
`--update` prints the old and new excerpt for every changed pin, so a reviewer judges
meaning rather than a hex string.

HOW TO RELOCATE PINS AFTER AN EDIT SHIFTS A CITED FILE — the procedure, so a reviewer can
re-derive it instead of trusting that it was done. `--update` on its own is a LAUNDERING
MACHINE: it will happily record a fingerprint for whatever now sits at `path:line`, and
"0 MOVED" afterwards proves only that the pins agree with the docs, never that the docs
point at the right code. So the doc citations are corrected FIRST, by content, and only
then is `--update` run.

  1. Run the check. It names every citation whose fingerprint no longer matches.
  2. For each, take the OLD text of the cited range (`git show <base>:<path>`, lines
     start..end) and search the WORKING TREE file for that exact line sequence.
       * exactly one match  -> rewrite the citation in the citing doc to the new line
         numbers. Content equality is what justifies the move, not proximity.
       * zero or several matches -> the text is not unique (`}`, `true`, `ty,`). Do NOT
         guess. Establish a uniform offset instead: prove that every line from the first
         unedited one onwards satisfies `old[i] == new[i + delta]` for a single delta,
         which makes the relocation an identity rather than a search, then apply it and
         re-verify each moved range by FINGERPRINT EQUALITY against the pin file.
  3. Re-run the check. It should now report only "unpinned (run --update)" — i.e. new
     keys — with no MOVED.
  4. Run `--update` and read its MOVED list. It should be empty.

MOVED > 0 AFTER STEP 4 IS NOT AUTOMATICALLY A DEFECT, and here is the one benign case,
recorded because it happened: pin keys are `(path, lines, doc)`, so when citation A shifts
onto the line number citation B used to occupy, the key survives with different content and
is reported as MOVED. That is a KEY COLLISION, not a laundering. Resolve it by reading both
citations and confirming each names the code its prose describes — and say in the commit
message which keys collided, so the reviewer checks the same two lines you did.

THE HOLE MOVEMENT DETECTION CANNOT SEE, AND THE RULE THAT CLOSES IT.
Everything above is about a pin whose CONTENT changed. It says nothing about a citation
that changes its LINE NUMBERS, because the pin key contains them: edit `foo.rs:100` to
`foo.rs:120` in a doc, run `--update`, and the old key is dropped while a new one is added.
Neither is a MOVED. So `--update` reported `MOVED 0` while two citations in this repository
had come to rest on an EMPTY LINE and on a bare `}` — fingerprint-stable, content-free, and
therefore invisible to every check here. A sweep for that shape found 25 of them across 220
citations, several carrying `src:` evidence in the feature index whose prose quoted code the
cited line did not contain (`src/ownership/borrow_checker.rs:519` was claimed to be
`let call_lifetime = self.context.new_lifetime();` and was `}`).

So: A PIN WHOSE TARGET CARRIES NO CONTENT IS NOT A CITATION. A cited range whose text
contains no alphanumeric or underscore character at all — blank, `}`, `};`, `)));` — is
reported as NON-SEMANTIC and fails the gate, in the check AND in `--update`, which is what
stops the laundering being recorded rather than merely noticed. The rule is deliberately
the narrowest one that covers the measured shape: `true`, `ty,` and `Ok(())` are all real
one-token citations and all pass. It does not make a citation CORRECT — `}` was simply the
one wrongness a machine can name without reading.
"""
from __future__ import annotations

import hashlib
import os
import re
import shlex
import shutil
import signal
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
# The typed-result boundary, reused rather than re-implemented: `classify()` returns
# `Malfunction` for a signal or an unlisted status, and `Malfunction` carries no text
# attribute, so reading a non-concluding producer's output takes a deliberate step.
# It is DISCOURAGED BY CONVENTION, not prevented — the bytes remain reachable through
# `Run._out`. An earlier version of this comment called the ordering unexpressible; that
# was overstated. scripts/gate_probe.py is owned by another branch and is not edited here.
import gate_probe  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
PINS = ROOT / "docs" / "citation-pins.tsv"
ALLOW = ROOT / "docs" / "no-compile-allowlist.txt"
INDEX = ROOT / "docs" / "reference" / "features" / "feature-index.toml"
MANIFEST = ROOT / "tests" / "conformance-manifest.txt"

CITED_ROOTS = ("src/", "scripts/", "tests/", "examples/", "stdlib/", "benchmarks/",
               "runtime/", "bootstrap/", "docs/")
CITED_FILES = ("Cargo.toml", "Makefile")

CITATION = re.compile(
    r"\b((?:src|scripts|tests|examples|stdlib|benchmarks|runtime|bootstrap|docs)/[\w./-]+?"
    r"|Cargo\.toml|Makefile|grammar\.ebnf):(\d+)(?:-(\d+))?\b"
    r"(?!:\d)"  # `path:line:col` is compiler output, not a citation
)

# CommonMark allows a fence to be indented up to three spaces. Matching only column zero
# let an indented fence escape the pinned count entirely.
FENCE = re.compile(r"^ {0,3}```")
NO_COMPILE_FENCE = re.compile(r"^ {0,3}```palladium\b[^\n]*\bno-compile\b")

# Per-tag evidence rules. A textual prefix is NOT enough: the failure this file exists to
# prevent was fifteen rows citing docs/specification/grammar.ebnf, which is documentation,
# not the compiler. Prefix-only validation leaves that hole open, so `src:` is resolved.
# `src:` may cite any NON-DOCUMENTATION repository file: compiler source, the runtime,
# a gate script, a build manifest. It may not cite anything under docs/ — that was the
# original circular-evidence failure, where fifteen rows rested on grammar.ebnf.
SRC_EVIDENCE = re.compile(
    r"^src:\s+((?:src|runtime|scripts|bootstrap|stdlib|benchmarks|tests|examples)/[\w./-]+"
    r"|Cargo\.toml|Makefile):(\d+)(?:-(\d+))?\s+\S")
# `cmd:` is EXECUTED. It used to be a SHAPE check — the string had to look like
# `cmd: X -> Y`, and X was never run — so the one evidence class that exists to prove an
# ABSENCE was the one nothing checked. Measured at 2ef170f, 9 of the 53 `cmd:` items in
# feature-index.toml were false, among them
#   `grep -rn 'effects::' src/ --include='*.rs' | grep -v '^src/effects' -> 1 line`
# which produces 8, and
#   `grep -rniE 'socket|TcpStream|BufReader' src/runtime/ -> exit 1, 0 lines`
# which produces 95, over a whole src/runtime/net.rs the row claimed did not exist.
#
# So the result is a machine-readable contract rather than prose:
#     cmd: <command> -> exit <N>, <M> lines[ -- <prose>]
# and the gate runs <command> and compares BOTH numbers. `exit 1, 0 lines` is the
# dominant shape and is a SUCCESS for an absence proof, which is why the exit status is
# declared and compared rather than asserted to be zero.
#
# Prose after the em dash is a READING of the output and carries exactly the status a
# `src:` claim carries about its cited range. So it gets the same rule: anything quoted
# in it must actually appear in the output.
CMD_EVIDENCE = re.compile(
    r"^cmd:\s+(?P<cmd>.+)\s+->\s+exit\s+(?P<rc>\d+),\s+(?P<n>\d+)\s+lines?\b"
    r"(?P<rest>.*)$")

# Commands a `cmd:` item may run. Deliberately short: a `cmd:` item is a hermetic
# observation of the checked-in source tree and nothing else. Anything needing a build,
# a compiler, or a gate belongs to the gate that already owns it — see CMD_REFERRED.
CMD_ALLOWED = {"grep", "ls", "find", "sort", "wc"}

# Programs that must never appear in a `cmd:`, and where the question actually lives.
# Building a second executor for these here would give the repository two engines that
# can disagree about whether a program is rejected — the class of bug this file's own
# header describes for the YAML parser: "a second parser can always disagree with the
# first". It is also how the corpus of programs stops being enumerable: an inline program
# in a TOML string is in nobody's closed inventory, so it can neither go MISSING nor be
# UNDECLARED, which is the whole protection tests/conformance-manifest.txt provides.
CMD_REFERRED = {
    "pdc": "a program's behaviour is proved by a conformance fixture: add it under "
           "tests/, declare it in tests/conformance-manifest.txt, and cite it as "
           "`conformance: <path>.pd <class>`",
    "pdm": "as for pdc — declare a fixture and cite `conformance:`",
    "pls": "as for pdc — declare a fixture and cite `conformance:`",
    "cargo": "a build is a gate, not an observation — cite it as `gate: cargo ... -> ...`",
    "make": "a gate is proved by running it — cite it as `gate: make <target> -> ...`",
    "bash": "a script is a gate — cite it as `gate: make <target> -> ...`",
    "sh": "a script is a gate — cite it as `gate: make <target> -> ...`",
}

# Paths that only exist after a build. A command reading one is not reproducible from a
# checkout, and a `cmd:` item that cannot be reproduced is the same unexecuted text this
# whole change removes. Measured: `grep -c '#line' build_output/01_lexical_comments.c`
# exited 2 ("No such file or directory") on a clean tree and had been recorded as
# "0, exit 1".
CMD_BUILD_ARTIFACT_ROOTS = {"target", "build_output"}   # compared case-folded

# Shell control operators. shlex(punctuation_chars=True) surfaces these as their own
# tokens ONLY when unquoted, so `grep -nE '#\[token\("(with|effect)"\)\]' f` keeps its
# parenthesised alternation while `grep x src/; rm -rf y` is refused.
CMD_OPERATORS = {";", "&", "&&", "||", "<", ">", ">>", "<<", "(", ")"}

CONF_CLASSES = ("run", "untranscribed", "vacuous", "xfail", "reject", "skip")
# The manifest's column count, named because conformance_counts() recounts the file and a
# row of the wrong width is not a fixture declaration. scripts/conformance.sh owns the
# refusal; this reader only declines to count what it cannot read.
MANIFEST_COLUMNS = 6
# The verdict vocabulary is the MANIFEST's class vocabulary, and the row is looked up
# there. It used to be PASS|COMPILE_FAIL|LINK_FAIL|RUN_FAIL|SKIP_NO_MAIN, checked against
# nothing but the fixture's existence — so six rows read `PASS (placeholder: only prints
# that the feature is unimplemented)`, which is the manifest's `vacuous` class wearing the
# word PASS. Separating those two is the entire reason the class exists.
CONF_EVIDENCE = re.compile(r"^conformance:\s+([\w./-]+\.pd)\s+(" +
                           "|".join(CONF_CLASSES) + r")\b")
GATE_EVIDENCE = re.compile(r"^gate:\s+(?:make\s+([\w-]+)|cargo\s+[^\n]+?)\s*->\s*\S")
TAGGED = re.compile(r"^(src|cmd|conformance|gate):")

# A bare `:1330` continuation carries no path, so CITATION cannot match it and it gets no
# pin and no movement check. One such form escaped four rounds of review. Authors must write
# the full path; the shorthand is rejected wherever a citation could live.
CONTINUATION = re.compile(r"(?<![\w/.]):(\d+)(?:-\d+)?`")

# Text a `src:` evidence item quotes must actually appear in the range it cites. This is the
# mechanical form of "a pin whose excerpt does not contain the thing being claimed".
QUOTED = re.compile(r"`([^`]{6,})`|\"([^\"]{6,})\"|'([^']{8,})'")

# A cited range that cannot support any claim: it contains no word character at all.
#
# THIS USED TO BE A LIST OF SIX EXACT STRINGS -- "", "}", "{", "};", ")", "*/" -- and the
# list is the defect, not the threshold. `]`, `];`, `),`, `);`, `},`, `,`, `|---|---|` and
# every other punctuation-only line are mechanically identical to the six and were all
# accepted, so the check named six examples of a class instead of testing the class.
# Enumerating members of a set the world can extend is the same shape as the citation
# fingerprints this whole gate exists to replace.
#
# The property is "there is nothing here to read": after whitespace normalisation, no word
# character. `\w` is Unicode-aware in Python 3, so a prose line in any script counts as
# substantive, and a line of only digits (`42`) does too -- it carries a value a claim can
# be about. What is left is punctuation and whitespace, which cannot.
#
# See the check in `main` for why this is a hard failure and not a warning.
def is_delimiter_only(text: str) -> bool:
    return re.search(r"\w", norm(text)) is None


def norm(text: str) -> str:
    return " ".join(text.split())


# A cited range has to CARRY something. Blank lines and pure punctuation are
# fingerprint-stable and content-free, which is what makes a citation that has
# come to rest on one invisible to the movement check — see the module
# docstring. One alphanumeric or underscore character anywhere in the range is
# enough; the rule names the measured shape and nothing wider.
SEMANTIC = re.compile(r"[A-Za-z0-9_]")


def is_semantic(text: str) -> bool:
    return bool(SEMANTIC.search(text))


def fingerprint(text: str) -> str:
    return hashlib.sha256(norm(text).encode()).hexdigest()[:12]


def excerpt(text: str, width: int = 160) -> str:
    """A bounded excerpt that shows BOTH ends of a cited range.

    Showing only the first 100 characters made the load-bearing later lines of a range
    citation invisible, which is how `src/typeck/mod.rs:365-540` looked plausible while
    naming nothing relevant. A reviewer needs to see where a range starts and where it ends.
    """
    t = norm(text).replace("\t", " ")
    if len(t) <= width:
        return t
    head = width // 2 - 3
    return t[:head] + " ... " + t[-(width - head - 5):]


def resolve(path_str: str) -> Path | None:
    if path_str == "grammar.ebnf":
        return ROOT / "docs" / "specification" / "grammar.ebnf"
    if path_str.startswith(CITED_ROOTS) or path_str in CITED_FILES:
        return ROOT / path_str
    return None


def strip_fenced(text: str) -> str:
    """Blank out fenced code blocks.

    A `path:line` inside a fence is sample output — an illustrative compiler diagnostic —
    not a claim about this repository.
    """
    out, fenced = [], False
    for line in text.split("\n"):
        if FENCE.match(line):
            fenced = not fenced
            out.append("")
            continue
        out.append("" if fenced else line)
    return "\n".join(out)


def citing_sources():
    """Every file whose `path:line` citations are pinned. -> [(rel, text)]

    THE ROOTS ARE THE SAME ON BOTH SIDES, and until this function existed they were not.
    `CITED_ROOTS` has always named src/, tests/, scripts/, stdlib/ … as legal TARGETS of a
    citation, and the scan only ever read docs/. So a citation written in a Rust doc
    comment or a test's header was checked by NOTHING, and `make check-docs` going green
    said nothing about it — while those are exactly the files where this repository puts
    its `file:line` evidence. Measured when the scan was widened: 96 citations in 18 files
    under src/ and tests/, none of them ever pinned.

    A pin proves a cited range has not MOVED. That is worth more in source than in prose,
    not less: a comment citing `src/codegen/mod.rs:2811` is read by whoever is editing the
    file next to it, and inserting twenty lines above the target silently repoints it at
    something unrelated. Editing source is how citations move; docs/ is where they were
    being checked.

    Fences are stripped from Markdown only. In a Rust doc comment a fence is written
    `/// ```text`, which `FENCE` does not match, so nothing here depends on stripping —
    and leaving it unstripped is the conservative direction: a citation inside a doc fence
    gets pinned rather than exempted.

    ROOT-LEVEL `*.md` IS IN THE SET, and that is the half of the asymmetry that was missing
    longest. `CITED_ROOTS` has always allowed src/, tests/, scripts/ … as legal TARGETS of a
    citation, so the gate has always checked where a claim POINTS; it did not check the files
    where claims are MADE unless they lived under docs/. `CLAUDE.md` — the file every agent
    working on this repository is told to read first — was therefore ungated, and its single
    `path:line` citation was WRONG: it named `check_stmt` while claiming to name the call path
    that mints and ends a per-call lifetime. Nobody could say when it broke, because nothing
    had ever looked.

    Measured before widening: root `.md` holds 1 citation in total (CLAUDE.md's), 0 of which
    fail, plus 1 unpinnable shorthand which was fixed first. README, CONTRIBUTING, FEATURES and
    README-crate carry none, so they enter with zero cost today and are governed from now on.
    Full accounting, including what this does NOT buy, is in
    `docs/contributing/claude-md-coverage.md`.
    """
    out = []
    # KNOWN BLIND SPOT: `scripts/` IS NOT IN THIS TUPLE.
    #
    # Every `path:line` citation written inside a gate script is therefore
    # unpinned and unchecked, and a green check-docs says nothing about any of
    # them. Measured 2026-08-23: 40 distinct citations across scripts/*.py and
    # scripts/*.sh, the largest groups being src/codegen/mod.rs (9),
    # src/linker.rs (6), scripts/conformance.sh (6) and src/main.rs (4). Eight of
    # them broke when fix/gcc-stage-invariant shifted scripts/conformance.sh and
    # were repaired by hand; nothing would have caught them.
    #
    # SIZING, so whoever takes this does not underestimate it: adding
    # `scripts/**/*.py` and `scripts/**/*.sh` here does not pin eight citations,
    # it pins ALL FORTY at once, and every one must first be reconciled by
    # CONTENT — the ones nobody has touched are exactly the ones most likely to
    # have drifted onto an unrelated line, which is the defect this module
    # already found 25 times across 220 citations (see the NON-SEMANTIC rule
    # above). Budget for that reconciliation, not for the glob edit. The
    # alternative, pinning these citations individually, is smaller but leaves
    # the surface open for the next one written.
    for doc in (sorted(ROOT.glob("*.md")) + sorted(ROOT.glob("docs/**/*.md"))
                + sorted(ROOT.glob("docs/**/*.toml"))
                + sorted(ROOT.glob("src/**/*.rs")) + sorted(ROOT.glob("tests/**/*.rs"))):
        text = doc.read_text(encoding="utf-8", errors="replace")
        if doc.suffix == ".md":
            text = strip_fenced(text)
        out.append((str(doc.relative_to(ROOT)), text))
    return out


def collect_citations() -> list[tuple[str, str, str, str, str]]:
    """-> (path, 'start-end', citing-doc, fingerprint-of-the-WHOLE-range, excerpt)

    The fingerprint covers the entire cited range and the endpoint is part of the key.
    Fingerprinting only the first line let everything in `path:49-228` change while green.
    """
    out = []
    for rel, text in citing_sources():
        for m in CITATION.finditer(text):
            path_str, start = m.group(1), int(m.group(2))
            end = int(m.group(3)) if m.group(3) else start
            span = f"{start}-{end}"
            target = resolve(path_str)
            if target is None:
                continue
            if not target.exists():
                out.append((path_str, span, rel, "MISSING-FILE", ""))
                continue
            lines = target.read_text(encoding="utf-8", errors="replace").split("\n")
            if start < 1 or end > len(lines) or end < start:
                out.append((path_str, span, rel, "OUT-OF-RANGE", ""))
                continue
            body = "\n".join(lines[start - 1:end])
            if not is_semantic(body):
                out.append((path_str, span, rel, "NON-SEMANTIC", excerpt(body)))
                continue
            out.append((path_str, span, rel, fingerprint(body), excerpt(body)))
    return sorted(set(out))


# Two citations to ONE range inside ONE enumeration.
#
# A `0 MOVED` gate cannot see this: it compares each pin to its own target, and two
# citations rewritten to the same place agree with their pins perfectly. The failure it
# misses is a REPAIR that collapsed two claims onto one location -- which is what a
# key-based relocation does whenever its key is not injective. Measured on this branch:
# a merge repair keyed on (target, citing-doc) collapsed 170 pairs, and the ledger was
# `0 MOVED, 0 NON-SEMANTIC` the whole time.
#
# The rule is decidable without reading any claim: a document that lists several
# citations in one breath is asserting several DISTINCT sites, so if two of them are
# equal at most one is right. "In one breath" is a 240-character window, which is the
# span of a citation list that reads as one sentence.
#
# WHY NOT THE OBVIOUS CHECK. "The same document cites the same range twice" is far too
# coarse: measured over this corpus it flags 41 groups, EVERY ONE of them legitimate --
# a claim stated in prose and restated in an `#[ignore]` reason, hundreds of characters
# apart. Narrowing to one enumeration takes the false positives to zero over 400
# citations while still catching both real cases, including one that a diff against
# every earlier revision of this branch did not.
ENUM_WINDOW = 240


def collect_enumeration_repeats() -> list[tuple[str, int, str]]:
    """-> (citing-doc, line, citation) for a citation repeated inside one enumeration."""
    out = []
    for rel, text in citing_sources():
        ms = [(m.start(), m.group(0)) for m in CITATION.finditer(text)]
        for i in range(len(ms)):
            for j in range(i + 1, len(ms)):
                if ms[j][0] - ms[i][0] > ENUM_WINDOW:
                    break
                if ms[i][1] == ms[j][1]:
                    out.append((rel, text[: ms[i][0]].count("\n") + 1, ms[i][1]))
    return out


def collect_continuations() -> list[tuple[str, str]]:
    """Citation shorthands that cannot be pinned. -> (citing-doc, matched-text)

    UNCONDITIONAL. An earlier version required a recognizable filename within a
    90-character lookbehind, which is a heuristic: a longer continuation, or a different
    formatting, recreated an unpinnable citation with the gate green. There is now no
    window and no filename requirement — any `:NNN` backtick shorthand outside a fenced
    block fails, and the author writes the full path. The corpus was swept to zero first,
    so this costs nothing and closes the hole rather than narrowing it.

    SAME CORPUS AS THE PINS (see citing_sources): source and tests are scanned too, and
    the sweep to zero was repeated there — 21 shorthands in 6 files, all rewritten to full
    paths. A `(`:1193`, `:1278`)` written in a codegen comment is exactly the form this
    rule exists to refuse: it is unpinnable, so it cannot be told from a citation that has
    silently drifted, and every one of those six files is a file whose line numbers move.
    """
    out = []
    for rel, text in citing_sources():
        for m in CONTINUATION.finditer(text):
            out.append((rel, text[max(0, m.start() - 60):m.end()].strip()))
    return out


# Spellings the specification has REPLACED. A document carrying the normative banner is
# defining the language, so it may not use a syntax N-something rules out. This is the
# mechanical form of a class that was fixed three times by hand and reappeared each time:
# a normative rule stated in one document and violated in another.
#   token, replacement, the normative section that decides it
FORBIDDEN_IN_NORMATIVE = [
    (re.compile(r"&mut\s+\w"), "`ref mut T`", "N9"),
    (re.compile(r"(?<![&\w])&(?!mut)(?!&)[A-Za-z_]\w*\s*[,)>;]"), "`ref T`", "N9"),
    (re.compile(r"\bmacro_rules!"), "the unified macro system", "N3"),
    (re.compile(r"#\[total\(decreases"), "`#[decreases(expr)]`", "N8"),
    (re.compile(r"\.await\b"), "nothing — `.await` is not in the language", "N7"),
    (re.compile(r"\basync\s+fn\b"), "nothing — there is no `async` keyword", "N7"),
]
NORMATIVE_BANNER = re.compile(r"^>\s*\*\*NORMATIVE", re.M)


def collect_normative_violations() -> list[tuple[str, int, str, str, str]]:
    """-> (doc, line, matched token, replacement, deciding section)

    USE, not MENTION. Only Palladium code blocks and syntax-defining table rows are
    scanned. Prose must be able to say "there is no `.await`" without failing its own gate,
    and a comparison block written in Rust is the whole point of these documents. What is
    checked is the syntax the document PRESENTS AS PALLADIUM.

    Scanning stops at a heading that marks itself non-normative ("Open design questions",
    "Relocated:"), because material below it defines nothing.
    """
    out = []
    for doc in sorted(ROOT.glob("docs/**/*.md")):
        text = doc.read_text(encoding="utf-8")
        if not NORMATIVE_BANNER.search(text):
            continue
        rel = str(doc.relative_to(ROOT))
        fence_lang = None
        for n, line in enumerate(text.split("\n"), 1):
            if FENCE.match(line):
                info = line.strip().lstrip("`").strip()
                fence_lang = None if fence_lang is not None else (info or "text")
                continue
            if re.match(r"^#+\s+(Open design questions|Relocated:)", line):
                break
            in_palladium = bool(fence_lang) and fence_lang.startswith("palladium")
            # A syntax-defining table row: `| `ref T` | A shared borrow ... |`
            defines_syntax = (fence_lang is None and line.startswith("|")
                              and line.count("`") >= 2)
            if not (in_palladium or defines_syntax):
                continue
            # A comment, or a sentence explaining what the spelling REPLACES, is a
            # mention. "Replaces Rust's `&mut T`" must not fail its own rule.
            code = line.split("//")[0]
            if re.search(r"Rust'?s?\b|Replaces|instead of|rather than|not\s+`", line):
                continue
            for pat, repl, sec in FORBIDDEN_IN_NORMATIVE:
                m = pat.search(code)
                if m:
                    out.append((rel, n, m.group(0).strip(), repl, sec))
    return out


def collect_fences() -> list[tuple[str, int]]:
    out = []
    for doc in sorted(ROOT.glob("docs/**/*.md")) + [ROOT / "README.md"]:
        if not doc.exists():
            continue
        n = sum(1 for l in doc.read_text(encoding="utf-8").split("\n")
                if NO_COMPILE_FENCE.match(l))
        if n:
            out.append((str(doc.relative_to(ROOT)), n))
    return sorted(out)

# --- running a `cmd:` item ------------------------------------------------------------
#
# THE PROPERTY THIS SECTION ENFORCES, STATED POSITIVELY:
#
#     An absence claim must be shown CAPABLE OF PRODUCING OUTPUT.
#
# The first version of this executor stated the property negatively — "a path a command
# names must exist" — and review walked in through two other doors of the same room:
#
#     grep -rn 'zzz'                  no path operand at all: grep reads stdin, is handed
#                                     DEVNULL, and returns exit 1, 0 lines. The canonical
#                                     absence proof, measured over nothing.
#     grep -r --regexp=zzz src/       the pattern is consumed by the option, so the one
#                                     real path is what the "drop the pattern" slice threw
#                                     away. Zero existence checks, exit 1, 0 lines.
#
# and two more found while confirming those: `grep -rne 'zzz'` (a clustered short option
# smuggling -e past a check that only looked at the whole token), and `wc -l` alone.
#
# Every one of them is the same bug, and no list of parse repairs closes a room. So the
# rule is now measured rather than parsed, in three layers, of which only the third is a
# proof:
#
#   L1 STRUCTURE   the FIRST segment must name at least one path operand; later segments
#                  must name none, because their input is the pipe. Pattern-bearing
#                  options (-e/-f/--regexp/--file, in every spelling including clustered)
#                  are refused outright, because if the pattern can come from anywhere but
#                  the first operand then "which operand is a path" has no total answer.
#   L2 FILESYSTEM  every named path resolves — through symlinks — to somewhere inside this
#                  repository, and exists.
#   L3 MEASUREMENT for any item claiming 0 lines, the first segment is re-run with its
#                  pattern replaced by one that matches EVERY line of every stream. If
#                  that run finds nothing, the command reads nothing, and its emptiness
#                  measures nothing.
#
# L3 is what makes this a property rather than a patch list. It subsumes all four doors
# above and closes ones nobody has thought of — a live directory filtered to nothing by
# --include, an empty scope, a path that exists but holds no files — because it asks the
# stream, not the argv. It also VALIDATES L1's parse: the probe can only be built by
# identifying the pattern, so a mis-identified pattern makes the probe fail loudly instead
# of silently checking the wrong thing.

# The tools a `cmd:` may run, and the statuses each may CONCLUDE at. Anything else — a
# signal, a status not listed — is a MALFUNCTION: nothing was established, so no verdict
# may be read out of it. grep's 1 is "did not match"; its 2 is "could not look".
CMD_OK_STATUS = {"grep": (0, 1), "ls": (0,), "find": (0,), "sort": (0,), "wc": (0,)}

# PATH is pinned rather than inherited. Without this, which binary answers `grep` is
# decided by the caller's environment, and the "no shell" property is a claim about a
# string rather than about the process that ran.
SAFE_PATH = "/usr/bin:/bin:/usr/sbin:/sbin"

# grep options that supply the PATTERN. Refused, not handled: if the pattern can arrive
# through an option then the first operand may be a path, and the gate has no total way to
# tell operands apart. `grep -r --regexp=zzz src/` is exactly that hole, and it read as a
# perfect absence proof over zero checked paths.
GREP_PATTERN_OPTS = {"-e", "-f", "--regexp", "--file"}
GREP_PATTERN_SHORTS = "ef"
# Options taking a separate argument that is NOT a pattern. Also refused, for the same
# reason in reverse: their argument would be counted as a path.
GREP_OPTS_WITH_ARG = {"-m", "-A", "-B", "-C", "-d", "--include", "--exclude",
                      "--max-count", "--after-context", "--before-context", "--context"}
GREP_SHORTS_WITH_ARG = "mABCd"
# Inverting the match makes "matched nothing" mean the opposite thing, so the L3 probe
# must not inherit it. Handled as a whole token AND as a letter inside a short cluster:
# stripping only the token `-v` left `-vn` inverted, so a legitimate item written that way
# was falsely REJECTED. Fail-closed, but it made an honest observation unwritable.
GREP_DEREF_RECURSIVE = {"-R", "--dereference-recursive"}
GREP_INVERT = {"-v", "--invert-match"}
GREP_INVERT_SHORT = "v"

# find's expression, ENUMERATED. Everything a `cmd:` item needs in order to observe the
# tree, and nothing else. Until this existed the expression was forwarded to find
# unchecked, so
#     find src -name '*.rs' -exec /bin/sh -c id {} +
# was accepted — a declared hermetic observation that runs an interpreter of the
# document's choosing — and `-delete` would have altered the checkout. Neither is caught
# by the head or tool checks, because the executable being run really is `find`.
#
# The set is closed for a second reason. The L3 probe below neutralises the MATCHING
# predicates and preserves the TRAVERSAL ones, and that rewrite can only be total over a
# set this small. Negation is refused for the same reason: under `-not`, neutralising
# `-name X` to `-name '*'` would match nothing, and the probe would accuse an honest item.
FIND_MATCH_PREDICATES = {"-name", "-iname", "-path", "-ipath"}    # at most ONE, neutralised
FIND_TRAVERSAL_PREDICATES = {"-type", "-maxdepth", "-mindepth"}   # kept: they decide what is READ
FIND_TYPE_ARGS = {"f", "d", "l", "p", "s", "b", "c"}

# 4 MiB. A broad recursive grep can emit far more than the gate needs, and an unbounded
# read is an out-of-memory failure dressed as a doc check. The count is all that is
# compared, so a command this loud is a badly scoped item, and saying so is better than
# buffering it.
CMD_MAX_BYTES = 4 * 1024 * 1024
CMD_TIMEOUT_S = 120
CMD_CLEANUP_S = 10      # for ALL of cleanup together, after the kill
CMD_MAX_SEGMENTS = 8    # a `cmd:` is an observation, not a shell script

_TOOLS: dict = {}


def resolve_tool(name: str):
    """A bare command name -> the absolute path of the binary. -> (path, error).

    Two things are enforced here that "no shell" only claimed before. A token containing
    a slash is refused, so a checked-in `scripts/grep` cannot be the thing that runs; and
    the lookup happens on SAFE_PATH rather than the inherited one, so the answer does not
    depend on who invoked make. The resolved path is what Popen is given.
    """
    if "/" in name:
        return None, (f"names the executable by path ({name!r}); a `cmd:` item may only "
                      f"use a bare tool name, which is resolved on a pinned PATH. A path "
                      f"here would let a file in this repository be the thing that runs")
    if name not in _TOOLS:
        _TOOLS[name] = shutil.which(name, path=SAFE_PATH)
    if _TOOLS[name] is None:
        return None, f"needs `{name}`, which is not on the pinned PATH ({SAFE_PATH})"
    return _TOOLS[name], None


def contained(rel: str):
    """Resolve a path operand and require it to stay inside the repository.

    `Path.resolve()` follows symlinks, which a lexical check cannot: a link committed
    inside the repo can point anywhere, and the gate would then be measuring unversioned
    content while reporting on this tree. scripts/conformance.sh:600-612 refuses the same
    thing for the same reason.
    """
    if rel.startswith(("/", "~")):
        return None, f"names {rel!r}, which is absolute; a `cmd:` observes this tree only"
    p = (ROOT / rel)
    if not p.exists():
        return None, (f"reads {rel!r}, which does not exist. An absence measured over a "
                      f"path that is not there is not an absence: BSD grep with --include "
                      f"exits 1 and prints nothing for a missing directory, which is "
                      f"exactly what a true absence proof looks like")
    real = p.resolve()
    if real != ROOT and ROOT not in real.parents:
        return None, (f"names {rel!r}, which resolves to {real} — outside {ROOT}. The gate "
                      f"would be measuring unversioned content and reporting it as this "
                      f"repository's state")
    # BUILD ARTIFACTS, DECIDED AFTER RESOLUTION. This used to be a lexical
    # startswith(("target/", "./target/", ...)) over the raw token, which `target` without
    # a slash and `docs/../target` both walked straight past — and CI creates target/
    # before the documentation-evidence step, so generated state could validate
    # documentation. The question is which directory the path IS IN once resolved, so it
    # is asked of the resolved, checkout-relative first component.
    # A RECURSIVE ROOT THAT CONTAINS AN EXCLUDED DIRECTORY READS IT. The alias repair
    # fixed EXPLICIT operands: `target`, `docs/../target`. It did nothing about
    # `grep -r pattern .`, which resolves to the repository root, passes containment, and
    # then reads target/ and build_output/ anyway. The hermetic claim is about what is
    # READ, not how the path was spelled -- so an operand that is an ANCESTOR of an
    # excluded directory is refused, exactly as one inside it is. That narrows the grammar
    # instead of validating it, the move the find expression and the five-command list
    # both needed.
    if any((real / d).exists() for d in CMD_BUILD_ARTIFACT_ROOTS):
        return None, (f"names {rel!r}, which CONTAINS build output; a recursive read from "
                      f"here would descend into it. Name the subdirectory the observation "
                      f"is actually about")
    # Case-folded: on a case-insensitive checkout `TARGET` is the same directory as
    # `target`, and a case-sensitive comparison would let the alias through.
    first = real.relative_to(ROOT).parts[0].casefold() if real != ROOT else ""
    if first in CMD_BUILD_ARTIFACT_ROOTS:
        return None, (f"reads {rel!r}, which resolves into {first}/ — a build artifact. A "
                      f"`cmd:` item must be reproducible from a clean checkout, so "
                      f"generated state is evidence only through the gate that generates "
                      f"it")
    return real, None


def parse_segment(argv):
    """Split one pipeline segment into (options, pattern, paths). -> (parsed, error)

    Total for the five allowed tools, which is why the pattern-bearing options above are
    refused rather than handled. `parsed` is a dict so the L3 probe can rebuild the argv
    with a different pattern; that rebuild is the check on this parse.
    """
    head = argv[0]
    rest, opts, operands, after_ddash = argv[1:], [], [], False
    # find's grammar is `find <path>... <expression>`, and the expression contains bare
    # words: `-name '*.v' -o -name '*.thy'`. Reading those as further paths made the gate
    # demand a file literally called `*.v`. Once the expression starts it never stops.
    in_find_expr = False
    for tok in rest:
        if in_find_expr:
            opts.append(tok)
            continue
        if not after_ddash and tok == "--":
            after_ddash = True
            opts.append(tok)
            continue
        if not after_ddash and tok.startswith("-") and tok != "-":
            if head == "find" and operands:
                in_find_expr = True         # paths are read; the rest is the expression
                opts.append(tok)
                continue
            if head == "find":
                return None, (f"puts {tok!r} before any path. find's grammar is "
                              f"`find <path>... <expression>`, and a `cmd:` item must name "
                              f"the scope it observes first")
            base = tok.split("=", 1)[0]
            if base in GREP_DEREF_RECURSIVE:
                return None, ("uses -R, which follows symlinks while descending, so a link "
                              "inside the tree can lead the read outside it. Containment "
                              "is checked on the operand, and only -r keeps that check "
                              "meaning what it says")
            if base in GREP_PATTERN_OPTS:
                return None, (f"supplies its pattern through {base!r}. That is refused: if "
                              f"the pattern can arrive through an option, the first operand "
                              f"may be a path, and there is no total rule for telling "
                              f"operands apart. Write the pattern as the first operand")
            if base in GREP_OPTS_WITH_ARG and "=" not in tok:
                return None, (f"uses {tok!r}, which takes a separate argument the gate "
                              f"would count as a path. Use the inline `--opt=value` form")
            # Clustered short options: `-rne` is `-r -n -e`, and a check that only looked
            # at the whole token let the `e` through with its argument counted as a path.
            if not tok.startswith("--"):
                for ch in tok[1:]:
                    if ch == "R":
                        return None, (f"clusters -R inside {tok!r}, which follows symlinks "
                                      f"while descending; use -r")
                    if ch in GREP_PATTERN_SHORTS:
                        return None, (f"clusters -{ch} inside {tok!r}, which supplies the "
                                      f"pattern; see above. Write the pattern as the first "
                                      f"operand")
                    if ch in GREP_SHORTS_WITH_ARG:
                        return None, (f"clusters -{ch} inside {tok!r}, which takes a "
                                      f"separate argument the gate would count as a path")
            opts.append(tok)
            continue
        operands.append(tok)
    pattern = None
    if head == "grep":
        if not operands:
            return None, "is a grep with no pattern at all"
        pattern, operands = operands[0], operands[1:]
    if head == "find":
        err = check_find_expression(opts)
        if err:
            return None, err
    return {"head": head, "opts": opts, "pattern": pattern, "paths": operands}, None


def check_find_expression(expr):
    """The find expression this gate permits. -> error-or-None.

        <traversal>*  <match>?

    A conjunction of traversal predicates and AT MOST ONE matching predicate. No `-o`, no
    `-a`, no action, no negation, no parentheses.

    WHY IT IS THIS SMALL. Three review rounds produced three defects in this one
    construct, each time in the reduction that turns a command into its L3 probe:
      round 1  the probe deleted the expression outright, so `find empty-dir -type f`
               probed as `find empty-dir` and printed the directory itself;
      round 2  `-type` was preserved but `-o` is Boolean, so `-type f -o -name '*.zzz'`
               reads as (type f) OR (name) and one vacuous branch removed the -type bound;
      round 3  `-a` binds tighter than `-o`, so `-name '*.x' -o -name '*.y' -print` reads
               as name(x) OR (name(y) AND print) — measured, the command printed 0 lines
               and its probe printed 3.
    Each fix was to the instance. The construct is find's Boolean expression grammar, and
    a gate does not need one: the corpus had exactly ONE item using `-o`, and it splits
    into three items that are each trivially checkable. So the grammar is cut down to the
    fragment where the reduction is obviously sound, rather than made cleverer again.

    THE GUARANTEE, IN ONE SENTENCE: because the expression is a conjunction of traversal
    predicates and at most one matching predicate, replacing that predicate's argument
    with `*` yields a command whose results are a SUPERSET of the original's, so an empty
    probe proves the scope held nothing for the command to match.
    """
    i, seen_match = 0, None
    while i < len(expr):
        tok = expr[i]
        if tok in FIND_TRAVERSAL_PREDICATES:
            if seen_match:
                return (f"puts the traversal predicate {tok!r} after the matching predicate "
                        f"{seen_match!r}. Traversal bounds WHAT IS READ and must come "
                        f"first, so the probe can keep it while neutralising the match")
            if i + 1 >= len(expr):
                return f"has {tok!r} with no argument"
            if tok == "-type" and expr[i + 1] not in FIND_TYPE_ARGS:
                return (f"has -type {expr[i + 1]!r}; the argument must be one of "
                        f"{', '.join(sorted(FIND_TYPE_ARGS))}")
            i += 2
            continue
        if tok in FIND_MATCH_PREDICATES:
            if seen_match:
                return (f"uses two matching predicates ({seen_match} and {tok}). This gate "
                        f"permits at most one, because the probe replaces it with a "
                        f"match-everything argument and that reduction is only obviously "
                        f"sound when there is nothing to combine it with. Write one item "
                        f"per pattern")
            if i + 1 >= len(expr):
                return f"has {tok!r} with no argument"
            seen_match = tok
            i += 2
            continue
        allowed = sorted(FIND_MATCH_PREDICATES | FIND_TRAVERSAL_PREDICATES)
        extra = ""
        if tok in ("-o", "-a", "-not", "!", "(", ")", "-print", "-print0"):
            extra = (" find's Boolean expression grammar is deliberately outside this "
                     "gate: `-o` and `-a` have precedence, and every attempt to reduce an "
                     "expression containing them to a sound probe has been wrong. Write "
                     "one `cmd:` item per pattern instead; the default action already "
                     "prints, so `-print` is never needed.")
        return (f"uses the find predicate {tok!r}, which is not in the observation set "
                f"({', '.join(allowed)}).{extra} A find expression is forwarded to a real "
                f"process, so `-exec` would run a program of this document's choosing and "
                f"`-delete` would alter the checkout — neither is caught by the tool "
                f"checks, because the program being run really is find")
    return None


def build_probe(parsed):
    """An argv for the same scope that MUST match, if one can be built. -> argv or None.

    grep: the same options and paths, with a pattern matching every line of every stream.
    The empty pattern does that in BRE, ERE and -F alike, so the probe does not depend on
    the dialect. Inversion is removed — as a token and as a letter in a short cluster —
    because with it "matched everything" becomes "printed nothing".

    find: the traversal predicates are kept verbatim and the single matching predicate has
    its argument replaced by `*`. See check_find_expression for why the permitted grammar
    is only `<traversal>* <match>?` — three rounds of defects in the reduction of find's
    Boolean expressions, fixed each time at the instance, until the construct itself was
    removed.
    """
    head = parsed["head"]
    if head == "grep":
        opts = []
        for o in parsed["opts"]:
            if o in GREP_INVERT:
                continue
            if o.startswith("-") and not o.startswith("--"):
                o = "-" + "".join(c for c in o[1:] if c != GREP_INVERT_SHORT)
                if o == "-":
                    continue
            opts.append(o)
        return [head] + opts + [""] + parsed["paths"]
    if head == "find":
        expr, i = [], 0
        src = parsed["opts"]
        while i < len(src):
            tok = src[i]
            if tok in FIND_MATCH_PREDICATES:
                expr += [tok, "*"]                  # the one match, widened to everything
                i += 2
            else:
                expr += [tok, src[i + 1]]           # a traversal predicate, kept verbatim
                i += 2
        return [head] + parsed["paths"] + expr
    return None


def split_pipeline(cmd: str):
    """Split a `cmd:` command into checked argv segments. -> (segments, error-or-None)

    There is NO SHELL anywhere in this path. A `cmd:` item is argv, not a script: parsed
    with shlex, resolved on a pinned PATH, run with Popen, piped by file descriptor. That
    is not only a safety property — it is what makes the allowlist mean anything, because
    with `shell=True` a quoted string could still smuggle a substitution past any token
    inspection.

    Returns segments as {"argv": [...], "parsed": {...}}.
    """
    try:
        lex = shlex.shlex(cmd, posix=True, punctuation_chars=True)
        lex.whitespace_split = True
        tokens = list(lex)
    except ValueError as exc:               # unbalanced quote
        return None, f"could not be parsed as a command ({exc})"
    if not tokens:
        return None, "is empty"
    if "`" in cmd or "$(" in cmd or "${" in cmd:
        return None, ("uses command or variable substitution; a `cmd:` item must be a "
                      "fixed observation, not a computed one")

    raw, current = [], []
    for tok in tokens:
        if tok == "|":
            raw.append(current)
            current = []
        elif tok in CMD_OPERATORS:
            return None, (f"uses the shell operator {tok!r}; only a plain pipeline of "
                          f"allowed commands may be a `cmd:` item")
        else:
            current.append(tok)
    raw.append(current)

    segments = []
    for n, seg in enumerate(raw):
        if not seg:
            return None, "has an empty pipeline segment"
        head = seg[0]
        if os.path.basename(head) in CMD_REFERRED:
            return None, (f"runs `{os.path.basename(head)}`, which is not an observation "
                          f"— {CMD_REFERRED[os.path.basename(head)]}")
        # Before the allowlist, so the diagnosis names the real objection. `scripts/grep`
        # is not "an unknown tool" — it is a file in this repository being asked to be the
        # thing that runs, and whose interpreter could be a shell.
        if "/" in head:
            return None, (f"names the executable by path ({head!r}); a `cmd:` item may "
                          f"only use a bare tool name, resolved on a pinned PATH. A path "
                          f"here would let a file in this repository be what runs")
        if head not in CMD_ALLOWED:
            return None, (f"runs `{head}`, which is not in the `cmd:` allowlist "
                          f"({', '.join(sorted(CMD_ALLOWED))}). A `cmd:` item is a "
                          f"hermetic observation of the checked-in tree; anything else "
                          f"belongs to the gate that owns it")
        exe, err = resolve_tool(head)
        if err:
            return None, err
        parsed, err = parse_segment(seg)
        if err:
            return None, err
        # L1. The first segment is the one that reads the tree, so it must name something
        # to read; every later segment's input is the pipe, so naming a file there would
        # mean the pipeline measured two different things and reported one number.
        if n == 0 and not parsed["paths"]:
            return None, (f"names no path to read. Its input would be the gate's empty "
                          f"stdin, so `exit 1, 0 lines` would be produced by reading "
                          f"nothing at all — which is what a true absence proof looks "
                          f"like, and is why this is refused rather than measured")
        if n > 0 and parsed["paths"]:
            return None, (f"is downstream of a pipe but also names the path(s) "
                          f"{parsed['paths']}. Its input would be the file, not the pipe, "
                          f"so the pipeline's result would not be what it appears to be")
        # L2.
        for rel in parsed["paths"]:
            _, err = contained(rel)
            if err:
                return None, err
        segments.append({"argv": [exe] + seg[1:], "parsed": parsed})
    return segments, None


def _classify(argv, rc: int, text: str, ok_status):
    """gate_probe's typed boundary, applied to one pipeline segment.

    Reused rather than re-implemented. `Malfunction` has no text attribute, so reading a
    non-concluding segment's output takes a deliberate step rather than being the default
    — a convention with a shape, not a barrier: the bytes are still reachable via
    `Run._out`. What IS structural here is that every caller of this function gets
    `(None, "", <reason>)` for a malfunction and has no output to misread.
    """
    return gate_probe.classify(gate_probe.Run(argv, rc, text), reject_codes=ok_status)


def run_pipeline(segments, timeout: int = CMD_TIMEOUT_S):
    """Run the pipeline. -> (rc, stdout, harness-error-or-None)

    EVERY SEGMENT'S STATUS IS CHECKED, not just the last one. An upstream segment killed
    by SIGPIPE, or failing on an unreadable file, truncates the stream while the
    downstream command cheerfully returns the declared absence. Each segment is classified
    through gate_probe, so a signal or an unlisted status is a MALFUNCTION and the run
    establishes nothing.

    This is REACHABLE on the allowlist, which an earlier version of this comment denied:
    `grep -q` terminates at its first match, so `grep -rn e src/ | grep -q fn` kills the
    upstream with SIGPIPE while the downstream exits 0 having printed nothing — a perfect
    absence, from a pipeline that broke in the middle. scripts/test-doc-evidence.sh
    exercises exactly that command through the ordinary allowlist.

    STDERR FROM ANY SEGMENT IS ALSO A HARNESS ERROR. grep answers three questions, not
    two — 0 matched, 1 did not match, 2 COULD NOT LOOK — and collapsing the third into
    "did not match" turns an unreadable path into a proof of absence. That is exactly how
    a probe for the LLVM backend's CLI flag was recorded as `exit 1, 0 lines`: the pattern
    began with two dashes, grep read it as an option, exited 2, and printed its usage to
    stderr. Nothing was ever searched. (That probe is also why no comment here may spell
    the flag out: it greps scripts/, so a mention would BE a hit.)
    scripts/conformance.sh:199-211 draws the same distinction for the same reason.

    stderr goes to a temporary FILE, never a pipe, so a chatty segment cannot deadlock the
    gate against a full pipe buffer while nobody is reading it. There is exactly one
    cleanup path, and it kills and reaps every process whatever happened, so an upstream
    hang after the downstream has been terminated becomes a controlled harness error
    rather than a traceback or a leaked process.
    """
    env = {"PATH": SAFE_PATH, "LC_ALL": "C"}   # pinned, not inherited
    # ONE DEADLINE FOR THE WHOLE PIPELINE. `timeout` used to be spent again in full by the
    # reader join, by the final wait, and by EVERY upstream wait, and then cleanup added
    # up to 10s per process on top — so a two-segment pipeline under a 120s "timeout"
    # could take past 360s and the number bounded nothing. Every wait below gets what is
    # LEFT of this, and cleanup gets a small documented budget of its own because it runs
    # after the deadline has already been declared blown.
    deadline = time.monotonic() + timeout
    def remaining():
        return max(0.0, deadline - time.monotonic())
    # Spawning is inside the deadline too, and the pipeline has a length. Every process
    # used to be started before any clock was consulted and nothing capped the segment
    # count, so "the deadline is total" was a statement about the waits only.
    if len(segments) > CMD_MAX_SEGMENTS:
        return None, "", (f"has {len(segments)} pipeline segments; at most "
                          f"{CMD_MAX_SEGMENTS} may be spawned under one deadline")
    procs, errfiles, prev = [], [], subprocess.DEVNULL
    try:
        for seg in segments:
            if remaining() <= 0:
                return None, "", f"did not finish within {timeout}s (while starting it)"
            errf = tempfile.TemporaryFile()
            errfiles.append(errf)
            try:
                # Its own session, so the cleanup below can kill DESCENDANTS. Measured
                # without it: a segment whose grandchild held stdout open made the whole
                # pipeline take 30s under a 3s timeout — the timeout returned the right
                # answer and the process kept the gate waiting anyway.
                p = subprocess.Popen(seg["argv"], cwd=ROOT, env=env, stdin=prev,
                                     stdout=subprocess.PIPE, stderr=errf,
                                     start_new_session=True)
            except OSError as exc:
                return None, "", f"could not start `{seg['argv'][0]}`: {exc}"
            if prev is not subprocess.DEVNULL:
                prev.close()
            prev = p.stdout
            procs.append(p)

        # THE READ IS BOUNDED BY THE TIMEOUT, WHICH IT WAS NOT. `stdout.read()` ran BEFORE
        # the timeout-bearing `wait()`, so a final process that held stdout open without
        # finishing blocked forever and CMD_TIMEOUT_S bounded nothing. The read happens on
        # a daemon thread with a bounded join; if it is still going the pipeline is killed
        # by the `finally` below, which unblocks it, and the daemon flag means a wedged
        # reader cannot keep the interpreter alive.
        box: dict = {}

        def _drain():
            try:
                box["out"] = procs[-1].stdout.read(CMD_MAX_BYTES + 1)
            except OSError as exc:                       # pipe torn down by the kill
                box["err"] = exc

        reader = threading.Thread(target=_drain, daemon=True)
        reader.start()
        reader.join(remaining())
        if reader.is_alive():
            return None, "", (f"did not finish within {timeout}s (its output stream was "
                              f"still open)")
        if "err" in box:
            return None, "", f"could not read the pipeline's output: {box['err']}"
        out = box.get("out", b"")
        if len(out) > CMD_MAX_BYTES:
            return None, "", (f"produced more than {CMD_MAX_BYTES} bytes. A `cmd:` "
                              f"item is an observation, not a dump; narrow its scope")
        try:
            procs[-1].wait(timeout=remaining())
        except subprocess.TimeoutExpired:
            return None, "", f"did not finish within {timeout}s"

        for p in procs[:-1]:
            try:
                p.wait(timeout=remaining())
            except subprocess.TimeoutExpired:
                return None, "", (f"upstream segment `{p.args[0]}` did not finish within "
                                  f"{timeout}s after the pipeline was drained")

        for p, seg, errf in zip(procs, segments, errfiles):
            errf.seek(0)
            err = errf.read().decode("utf-8", "replace").strip()
            if err:
                return None, "", (f"`{seg['parsed']['head']}` wrote to stderr, so it did "
                                  f"not answer the question — a command that could not "
                                  f"look is not a proof of absence: "
                                  f"{err.splitlines()[0][:160]}")
            verdict = _classify(seg["argv"], p.returncode, "",
                                CMD_OK_STATUS[seg["parsed"]["head"]])
            if isinstance(verdict, gate_probe.Malfunction):
                return None, "", (f"segment `{' '.join(seg['argv'][1:][:3])}...` "
                                  f"MALFUNCTIONED ({verdict.how}). Nothing was "
                                  f"established, so its output is not a result: a "
                                  f"pipeline that broke in the middle still hands the "
                                  f"last command an empty stream")
        return procs[-1].returncode, out.decode("utf-8", "replace"), None
    finally:
        cleanup_deadline = time.monotonic() + CMD_CLEANUP_S
        # One cleanup path, unconditional: kill then reap. A process left running would
        # hold a pipe open for whoever comes next.
        for p in procs:
            # The budget covers every cleanup operation, not just wait(): killpg and
            # close() are syscalls that can block on a wedged descendant too.
            if time.monotonic() >= cleanup_deadline:
                break
            # The GROUP, not the process: a grandchild is what holds the pipe open.
            try:
                os.killpg(p.pid, signal.SIGKILL)
            except OSError:
                pass
            if p.poll() is None:
                try:
                    p.kill()
                except OSError:
                    pass
            try:
                if p.stdout:
                    p.stdout.close()
            except OSError:
                pass
            try:
                # CMD_CLEANUP_S total, not per process: this runs after the pipeline has
                # already been killed, so it is a reaping formality, not a wait for work.
                p.wait(timeout=max(0.0, cleanup_deadline - time.monotonic()))
            except Exception:               # noqa: BLE001 — cleanup may not raise
                pass
        for errf in errfiles:
            errf.close()


def probe_reads_something(segments):
    """L3. Show the first segment CAN produce output. -> error-or-None.

    Required of every item claiming 0 lines, and of nothing else: an item claiming N > 0
    has already demonstrated that it read something.
    """
    first = segments[0]
    probe = build_probe(first["parsed"])
    if probe is None:
        return (f"claims an empty result, but `{first['parsed']['head']}` has no "
                f"match-everything form the gate can use to show it read anything. Only "
                f"grep and find can carry an absence claim here; for anything else, state "
                f"the observation as a non-empty result")
    exe, err = resolve_tool(first["parsed"]["head"])
    if err:
        return err
    rc, out, herr = run_pipeline([{"argv": [exe] + probe[1:], "parsed": first["parsed"]}])
    if herr:
        return f"could not be shown to read anything: the control run {herr}"
    if rc != 0 or not out:
        return (f"claims an empty result over a scope that produces NOTHING even when "
                f"asked to match everything ({' '.join(probe[1:])} -> exit {rc}, "
                f"{len(out.splitlines())} lines). The command reads nothing, so its "
                f"emptiness measures nothing. An absence proof must be shown capable of "
                f"producing output")
    return None


def check_cmd(name: str, item: str, cache: dict) -> list[str]:
    """Check one `cmd:` evidence item by RUNNING it. -> list of problems."""
    m = CMD_EVIDENCE.match(item)
    if not m:
        return [f"{name}: `cmd:` must be `<command> -> exit <N>, <M> lines[ -- <prose>]`, "
                f"so the gate can run the command and compare both numbers; an absence "
                f"is proved by `exit 1, 0 lines`, not by prose -> {item[:80]!r}"]
    cmd = m.group("cmd").strip()
    want_rc, want_n = int(m.group("rc")), int(m.group("n"))
    rest = m.group("rest")

    key = (cmd, want_n == 0)
    if key not in cache:
        segments, err = split_pipeline(cmd)
        if err:
            cache[key] = (None, "", f"`cmd:` {err}")
        elif want_n == 0 and (perr := probe_reads_something(segments)):
            cache[key] = (None, "", f"`cmd:` {perr}")
        else:
            cache[key] = run_pipeline(segments)
    rc, out, err = cache[key]
    if err:
        return [f"{name}: {err}\n      command: {cmd}"]

    lines = out.splitlines()
    problems = []
    if rc != want_rc or len(lines) != want_n:
        shown = "\n".join(f"        | {l[:150]}" for l in lines[:6]) or "        | (no output)"
        if len(lines) > 6:
            shown += f"\n        | ... and {len(lines) - 6} more line(s)"
        problems.append(
            f"{name}: `cmd:` claims a result the command does not produce\n"
            f"      command: {cmd}\n"
            f"      claimed: exit {want_rc}, {want_n} line(s)\n"
            f"      actual:  exit {rc}, {len(lines)} line(s)\n"
            f"      output:\n{shown}\n"
            f"      Re-derive the row against what the command produces now. If the true "
            f"result undermines the claim, the claim is what changes.")
        return problems

    # The prose is a reading of the output, so whatever it quotes must be in the output.
    # Same rule, same reason, as the `src:` quoted-claim check above.
    body = norm(out)
    for q in QUOTED.finditer(rest):
        text = q.group(1) or q.group(2) or q.group(3)
        if norm(text) not in body:
            problems.append(
                f"{name}: `cmd:` quotes {text[:60]!r} but the command's output does not "
                f"contain it\n      command: {cmd}\n"
                f"      Quote from the output, or move the remark to `note`, which is "
                f"where the schema puts prose that is not load-bearing.")
    return problems


# --- `gate:` evidence: a receipt from THIS run, never a transcription -------------------
#
# `gate:` was the last unexecuted evidence class: eight outcomes validated only as "a Make
# target by that name exists". Two of them still carried the pre-2026-08-22 conformance
# output shape, so the file was already carrying stale numbers under a green gate.
#
# Running these from inside the doc lint is not possible — `make gates` runs `check-docs`,
# and the gates cited here are `make conformance` and `make selfhost`, so the lint would
# recurse into its own caller. The recursion is an artefact of putting both jobs in one
# target, so they are split: scripts/gate-receipts.sh executes each DISTINCT referenced
# gate exactly once (three rows cite `make selfhost`; it runs once), records what it
# printed, and then asks this checker to validate the index against those receipts.
#
# WHAT VALIDATION MEANS, and why it is not just "the gate passed": every checkable token
# in the declared result — a `key=value`, or a backtick/double-quoted span — must be borne
# out by what the gate printed in this run. That is what stops `-> fixtures=65 ...
# verified=45` from rotting the way `-> total=42 pass=39` did. A result with no checkable
# token at all is prose and is rejected, for the same reason a `cmd:` result may not be.
#
# KEY=VALUE IS COMPARED BY VALUE, NOT BY CONTAINMENT. The first version asked whether the
# claimed token appeared as a SUBSTRING of the output, so a row claiming `verified=4`
# validated against a run that printed `verified=46`. A number could drift downward by
# truncation and still pass — in the one mechanism whose entire purpose is that a number
# cannot drift. Keys are now parsed out of the receipt and the values compared exactly,
# and a mismatch reports both.
#
# A QUOTED SPAN IS STILL A SUBSTRING, AND THAT IS A REVIEWED BOUNDARY. Requiring a quoted
# span to be a whole output line would make many honest rows unwritable. So a row can
# satisfy validation with a ubiquitous quoted span and then put an unsupported conclusion
# in the unchecked prose beside it. That is a limit of this mechanism, recorded here and
# in the index's schema header rather than papered over; the prose next to a validated
# token is read by a human, exactly like the excerpt beside a citation pin.
GATE_KV = re.compile(r"(?<![\w.=-])([a-z_][a-z0-9_]*)=([^\s,;]+)")


def gate_tokens(result: str):
    """-> (key/value pairs, quoted spans). The parts a machine can disagree with."""
    kv = [(m.group(1), m.group(2)) for m in GATE_KV.finditer(result)]
    quoted = [q.group(1) or q.group(2) or q.group(3) for q in QUOTED.finditer(result)]
    return kv, [q for q in quoted if q]


def gate_mismatches(kv, quoted, output: str):
    """What the run does not bear out. -> list of human-readable strings."""
    seen: dict = {}
    for m in GATE_KV.finditer(output):
        seen.setdefault(m.group(1), set()).add(m.group(2))
    out, body = [], norm(output)
    for k, v in kv:
        if k not in seen:
            out.append(f"{k}={v} (the run printed no {k}= at all)")
        elif seen[k] != {v}:
            # MEMBERSHIP IS NOT AGREEMENT. Asking only whether the claim is one of the
            # values observed let an output containing both `verified=46` and
            # `verified=4` validate EITHER claim — so a gate that contradicts itself
            # endorses whichever number the document happens to want. The run must have
            # said one thing about this key, and it must be the thing claimed.
            got = ", ".join(sorted(seen[k]))
            out.append(f"{k}={v} (the run printed {k}={got})"
                       if len(seen[k]) == 1 else
                       f"{k}={v} (the run printed {k} with more than one value: {got} — "
                       f"a self-contradicting gate endorses nothing)")
    for q in quoted:
        if norm(q) not in body:
            out.append(f"{q!r} (not in the run's output)")
    return out


def load_receipts(dirpath: Path):
    """-> ({command: (exit, output)}, None) or (None, error).

    A RECEIPT FROM AN EARLIER RUN MUST NOT SATISFY THIS ONE, and the previous attempt at
    that was correlation rather than freshness: a run id written INTO the receipts
    directory and compared against a caller-supplied string. The id sat next to the bytes
    it authenticated, so `--gate-run-id "$(cat .../RUN_ID)"` replayed a week-old run — 
    measured, it validated 10/10.

    Freshness is now structural, and the guarantee is narrower than "nothing survives":
    scripts/gate-receipts.sh writes into a private mktemp directory it removes on exit, so
    an earlier run's receipts are neither DISCOVERABLE by nor REUSABLE by a later
    certifying run — each mints its own unpredictable path. A SIGKILL or a host failure
    can still leave a directory behind, and this function will read one that is explicitly
    handed to it; what neither can do is contaminate the certifying path. The one thing
    left to enforce here is that a receipts directory may not live INSIDE the repository:
    a directory under version control is content, and content can be committed to look
    like the outcome of a run that never happened.
    """
    idx = dirpath / "index.tsv"
    if not idx.exists():
        return None, f"no receipts at {dirpath} (run: make gate-receipts)"
    real = dirpath.resolve()
    if real == ROOT or ROOT in real.parents:
        return None, (f"receipts at {dirpath} are inside the repository. Receipts are the "
                      f"output of a run, not content: a directory under version control "
                      f"could be committed and would then validate a run that never "
                      f"happened. scripts/gate-receipts.sh writes to a private temporary "
                      f"directory and deletes it on exit.")
    out = {}
    for line in idx.read_text(encoding="utf-8").split("\n"):
        if not line.strip() or line.startswith("#"):
            continue
        parts = line.split("\t")
        if len(parts) < 3:
            continue
        cmd, rc, slug = parts[0], parts[1], parts[2]
        body = dirpath / slug
        if not body.exists():
            return None, f"receipt body missing for {cmd!r} ({slug})"
        out[cmd] = (int(rc), body.read_text(encoding="utf-8", errors="replace"))
    return out, None


def load_manifest():
    """tests/conformance-manifest.txt as {path: (class, line-number)}, or None.

    None means the manifest could not be read, which is a gate failure and never a reason
    to accept a `conformance:` item unchecked — the runner that owns this file exits 2
    rather than report a green run without it (scripts/conformance.sh:171-175).
    """
    if not MANIFEST.exists():
        return None
    rows = {}
    for n, line in enumerate(MANIFEST.read_text(encoding="utf-8").split("\n"), 1):
        if not line.strip() or line.startswith("#"):
            continue
        parts = line.split("\t")
        if len(parts) < 2:
            continue
        rows[parts[0].strip()] = (parts[1].strip(), n)
    return rows


# --- the conformance-summary governor -------------------------------------------------
#
# WHY THIS EXISTS. `make conformance` prints a summary — verified/vacuous/xfail/reject/
# skip over N fixtures — and three PROSE sites quote it. Nothing checked them. Adding two
# fixtures therefore meant hand-editing three sentences, and three consecutive review
# rounds did exactly that; one of them invalidated a sentence written in the same round,
# because the fixture count moved again before the round ended. That is the class
# `report_ledger` in scripts/requirements.py already closes for MILESTONES' counts over
# the requirement manifest, and this is the same mechanism over the conformance manifest.
#
# THE TRUTH IS RECOUNTED FROM THE MANIFEST, NOT READ OFF A GATE RUN. A governor that
# needed `make conformance` to have run could not run on a fresh checkout, and would be
# checking the prose against a receipt rather than against the inventory the receipt is
# itself derived from. `tests/conformance-manifest.txt` is a CLOSED inventory: every
# fixture has a row and a declared class, so every number in that summary except two is a
# property of the file.
#
# THE TWO EXCEPTIONS ARE RESULTS, and are handled by stating the invariant rather than by
# pretending the manifest knows them. `failures` and `untranscribed` are outcomes of
# running the corpus; on a green tree both are 0, and `verified` is then exactly the
# number of `run` rows. So this governor holds the prose to `failures=0`,
# `untranscribed=0` and `verified == run rows` — which is not an assumption about the
# tree, it is the same statement as "conformance is green", and a tree where it is false
# fails `make conformance` before it reaches here.
#
# WHAT THIS DOES **NOT** GOVERN, so that nothing is checked twice. The three
# `gate: make conformance -> ...` strings in docs/reference/features/feature-index.toml
# carry key=value tokens and are validated by scripts/gate-receipts.sh against the real
# run — measured: planting `reject=999` there fails that gate with
# "the run printed reject=109". Governing them here as well would mean two gates to
# update per change, which is worse than one. The split is therefore:
#     feature-index.toml `gate:` strings  ->  make gate-receipts (against a real run)
#     the three prose sites below         ->  here (against the manifest)


def conformance_counts(manifest=None):
    """Recount the conformance manifest -> (counts, problems).

    `counts` is {class: n} plus "total"; `problems` names every row this reader could not
    account for. THE RECOUNT IS FAIL-CLOSED, and that is a correction: the first version
    did `if len(cols) != MANIFEST_COLUMNS: continue`, dropped unknown classes out of the
    class tally, and never looked for duplicate paths — three silent skips, each of which
    would have made the totals it publishes quietly wrong.

    WHY THIS DUPLICATES scripts/conformance.sh, DECLARED RATHER THAN ACCIDENTAL.
    `conformance.sh` already refuses all three shapes fail-closed — a wrong-width row at
    its `merr` for "expected 6 tab-separated non-empty columns", a repeat at "duplicate
    entry for '<path>'", and an unknown class through its own case analysis. It owns the
    RUN: nothing executes the corpus without going through it. This reader owns a
    different question — "do the counts quoted in prose match the inventory" — and it must
    answer on a tree where conformance has never been run, which is the whole reason its
    truth source is the manifest and not a gate receipt. A checker that is allowed to run
    standalone cannot delegate the validity of its own input to a gate that may not have
    run. So both check, on purpose; if they ever disagree about what a valid row is, that
    disagreement is itself the finding.

    Its own arithmetic rather than `load_manifest`'s, for the same reason: that helper
    returns a dict keyed by path, so two rows naming one fixture would count once. A
    governor that silently absorbs a duplicate is not counting the file.
    """
    path = MANIFEST if manifest is None else Path(manifest)
    if not path.exists():
        return None, [f"cannot read {path} — the conformance counts quoted in prose "
                      f"cannot be checked, and an unchecked count is how `over 194 "
                      f"fixtures` survived three corpus changes"]
    counts = {c: 0 for c in CONF_CLASSES}
    problems, seen, total = [], {}, 0
    name = path.name
    for n, line in enumerate(path.read_text(encoding="utf-8").split("\n"), 1):
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        cols = line.split("\t")
        if len(cols) != MANIFEST_COLUMNS:
            problems.append(
                f"{name}:{n}: expected {MANIFEST_COLUMNS} tab-separated columns, got "
                f"{len(cols)}: {line[:60]}")
            continue
        total += 1
        fixture, cls = cols[0].strip(), cols[1].strip()
        if fixture in seen:
            problems.append(
                f"{name}:{n}: duplicate entry for {fixture!r} (first declared at line "
                f"{seen[fixture]})")
        else:
            seen[fixture] = n
        if cls not in counts:
            problems.append(
                f"{name}:{n}: class {cls!r} is not one of "
                f"{' | '.join(CONF_CLASSES)} — an unrecognised class would drop out of "
                f"every count this file publishes")
            continue
        counts[cls] += 1
    counts["total"] = total
    return counts, problems


def conformance_claims(c):
    """-> [(label, path, regex, expected-tuple)] — every prose site quoting the summary.

    Each regex must match EXACTLY ONCE. A regex that stops matching is a FAILURE and never
    a skip, for `ledger_claims`' reason: the sentence being rewritten is precisely when the
    number inside it stops being checked.

    THE ASYMMETRY IN THE TUPLE IS DELIBERATE. `untranscribed` is a MANIFEST CLASS
    (scripts/conformance.sh's format comment lists it beside run/vacuous/xfail/reject/skip),
    so it is recounted like the others; hardcoding it to 0 — which the first version did —
    would have false-REDded a legitimate tree the moment one row was declared
    `untranscribed`. `failures` is NOT a class: it is a run result the manifest cannot
    know, so it stays a literal 0. Holding the prose to `failures=0` is not an assumption
    about the tree, it is the same statement as "conformance is green", and a tree where
    it is false fails `make conformance` before it reaches this gate.
    """
    full = (c["run"], c["untranscribed"], c["vacuous"], c["xfail"], c["reject"], c["skip"],
            0, c["total"])
    return [
        ("MILESTONES.md — the Conformance row of the status table",
         "docs/contributing/MILESTONES.md",
         r"`verified=(\d+) untranscribed=(\d+) vacuous=(\d+) xfail=(\d+) reject=(\d+) "
         r"skip=(\d+) failures=(\d+)` over (\d+)",
         full),
        ("MILESTONES.md — the reject-over-fixtures sentence",
         "docs/contributing/MILESTONES.md",
         r"(\d+) of them: `reject=(\d+)` over (\d+) fixtures",
         (c["reject"], c["reject"], c["total"])),
        ("language-spec.md — the conformance status paragraph",
         "docs/specification/language-spec.md",
         r"\*\*verified (\d+) · untranscribed (\d+) · vacuous (\d+) · xfail (\d+) · "
         r"reject (\d+) · skip (\d+) · failures (\d+)\*\*, over\n?\s*(\d+)",
         full),
    ]


def check_conformance_counts(manifest=None, root=None):
    """-> (problems, checked, counts). Names the SITE and prints have-vs-want.

    `manifest`/`root` exist for the negative controls in --self-test-counts, which plant a
    malformed inventory and a drifted document. They are ARGUMENTS and never environment
    variables, for scripts/requirements.py's reason: an exported variable could redirect
    the real gate at a file of the caller's choosing and no assertion about the Makefile
    would see it.
    """
    base = ROOT if root is None else Path(root)
    c, problems = conformance_counts(manifest)
    if c is None:
        return (problems, 0, None)
    if problems:
        # The counts over a file this reader could not fully account for are not evidence
        # of anything, so the comparison is not attempted and says so rather than
        # reporting a mismatch that is really an inventory defect.
        problems.append(
            "the conformance-count claims were NOT compared: the inventory above did not "
            "recount cleanly, and a total taken over rows this reader could not account "
            "for is not a number to hold prose to")
        return (problems, 0, c)
    checked = 0
    for label, rel, pattern, want in conformance_claims(c):
        text = (base / rel).read_text(encoding="utf-8")
        found = re.findall(pattern, text)
        if len(found) != 1:
            problems.append(
                f"{label}: the sentence this is checked in matched {len(found)} times, "
                f"not once (/{pattern}/). A rewritten sentence is when the number inside "
                f"it stops being checked, so this is a failure and not a skip.")
            continue
        got = tuple(int(g) for g in found[0])
        checked += 1
        if got != want:
            problems.append(
                f"{label}: {rel} states {got}, the manifest has {want} "
                f"(recounted from {MANIFEST.name})")
    return (problems, checked, c)



# --- negative controls for the conformance-count governor ------------------------------
#
# WHY THESE ARE HERE AND NOT ONLY IN A TRANSCRIPT. The governor's failure modes were
# established once, by hand, in a review report. A failure mode nobody re-runs is a
# failure mode that stops existing the moment someone edits the recount — which is the
# same argument this file makes about every `cmd:` item it executes. So the shapes are
# planted here, hermetically, and scripts/test-doc-evidence.sh asserts on them.
#
# HERMETIC: every control builds its own manifest and its own two documents in a
# temporary tree. Nothing reads the repository, so these cannot go green because the real
# tree happens to be well-formed, and cannot go red because it happens not to be.

_CLAIM_MILESTONES = """# planted by --self-test-counts

| Conformance | `verified={run} untranscribed={untr} vacuous={vac} xfail={xf} reject={rej} skip={skip} failures=0` over {total} (planted) |

{rej} of them: `reject={rej}` over {total} fixtures (planted).
"""

_CLAIM_SPEC = """# planted by --self-test-counts

**verified {run} · untranscribed {untr} · vacuous {vac} · xfail {xf} · reject {rej} · skip {skip} · failures 0**, over {total}
fixtures.
"""


def _plant_counts_tree(tmp, rows, claim=None, rewrite=False):
    """Write a manifest and the two claim documents. -> (manifest_path, root)

    `rows` are raw manifest lines (already tab-joined). `claim` overrides the numbers the
    documents state; None means "state the truth", so a control only fails for the one
    thing it is probing. `rewrite` rephrases one sentence WITHOUT changing its number,
    which is the shape that would defeat a governor whose missing match were a skip.
    """
    root = Path(tmp)
    (root / "docs" / "contributing").mkdir(parents=True, exist_ok=True)
    (root / "docs" / "specification").mkdir(parents=True, exist_ok=True)
    man = root / "conformance-manifest.txt"
    man.write_text("# planted\n" + "\n".join(rows) + "\n", encoding="utf-8")
    counts, _ = conformance_counts(man)
    said = dict(run=counts["run"], untr=counts["untranscribed"], vac=counts["vacuous"],
                xf=counts["xfail"], rej=counts["reject"], skip=counts["skip"],
                total=counts["total"])
    if claim:
        said.update(claim)
    milestones = _CLAIM_MILESTONES.format(**said)
    if rewrite:
        milestones = milestones.replace(
            "of them: `reject={rej}` over {total} fixtures".format(**said),
            "of them: reject is {rej} across {total} fixtures".format(**said))
    (root / "docs" / "contributing" / "MILESTONES.md").write_text(
        milestones, encoding="utf-8")
    (root / "docs" / "specification" / "language-spec.md").write_text(
        _CLAIM_SPEC.format(**said), encoding="utf-8")
    return man, root


def _row(path, cls):
    return "\t".join([path, cls, "-", "expected", "-", "-"])


def self_test_counts():
    """Drive the governor over planted inventories and documents. -> exit status."""
    import tempfile

    good = [_row("tests/a.pd", "run"), _row("tests/b.pd", "reject"),
            _row("tests/c.pd", "vacuous")]
    checks, fails = [], 0

    def case(label, rows, claim, want_green, *must_contain, rewrite=False):
        nonlocal fails
        with tempfile.TemporaryDirectory() as tmp:
            man, root = _plant_counts_tree(tmp, rows, claim, rewrite)
            problems, checked, _ = check_conformance_counts(manifest=man, root=root)
        green = not problems
        blob = " | ".join(problems)
        bad = []
        if green != want_green:
            bad.append(f"expected {'GREEN' if want_green else 'RED'}, got "
                       f"{'GREEN' if green else 'RED'}")
        for want in must_contain:
            if want not in blob:
                bad.append(f"message lacks {want!r}")
        if bad:
            fails += 1
            checks.append(f"  FAIL counts-control: {label}\n         " +
                          "\n         ".join(bad) +
                          ("\n         | " + blob[:400] if blob else ""))
        else:
            checks.append(f"  counts-control: {label} -> "
                          f"{'green' if green else 'red'}"
                          + ("" if green else f" ({problems[0][:96]})"))

    # THE FATAL GREEN CONTROL. Nothing below is worth anything until a well-formed
    # inventory whose documents state the truth passes.
    case("a well-formed inventory whose prose states the truth is GREEN",
         good, None, True)

    # FIX-2's regression control. `untranscribed` is a manifest CLASS, and the first
    # version hardcoded it to 0 in the expected tuple, so one legitimate row of it would
    # have false-REDded a valid tree.
    case("an `untranscribed` row does NOT false-RED a truthful tree",
         good + [_row("tests/d.pd", "untranscribed")], None, True)

    # FIX-1's three shapes, each a silent skip in the first version.
    case("a wrong-width row is NAMED, not skipped",
         good + ["tests/broken.pd\trun\t-"], None, False,
         "expected 6 tab-separated columns, got 3",
         "conformance-manifest.txt:5:")
    case("a duplicate path is NAMED, with the line it first appeared on",
         good + [_row("tests/a.pd", "reject")], None, False,
         "duplicate entry for 'tests/a.pd'", "first declared at line 2")
    case("an unrecognised class is NAMED, not dropped from the tally",
         good + [_row("tests/e.pd", "sortof")], None, False,
         "class 'sortof' is not one of", "an unrecognised class would drop out")
    case("an unaccounted inventory suppresses the comparison, and says so",
         good + ["tests/broken.pd\trun\t-"], None, False,
         "were NOT compared")

    # The claim-site shapes.
    case("a stale count is RED and NAMES THE SITE, with have-vs-want",
         good, {"total": 199}, False,
         "MILESTONES.md — the Conformance row of the status table",
         "docs/contributing/MILESTONES.md states", "the manifest has")
    # A number that appears in more than one document must be reported at EVERY site
    # that states it, not just the first: fixing one and shipping is how the other two
    # drifted apart in the first place.
    case("a stale count names EVERY site that states it, not just the first",
         good, {"rej": 999}, False,
         "MILESTONES.md — the Conformance row of the status table",
         "MILESTONES.md — the reject-over-fixtures sentence",
         "language-spec.md — the conformance status paragraph")
    case("a REWRITTEN sentence is a failure and never a skip",
         good, None, False, "matched 0 times, not once",
         "stops being checked", rewrite=True)

    for line in checks:
        print(line)
    if fails:
        print(f"conformance-count controls FAILED: {fails} of {len(checks)}")
        return 1
    print(f"conformance-count controls: {len(checks)} green (the fatal green control, the "
          f"`untranscribed` regression, three fail-closed recount shapes plus the "
          f"suppressed comparison, and three claim-site shapes)")
    return 0


# --- feature-index parsing: one parser, from the standard library ---------------------

def load_rows():
    """Parse feature-index.toml with `tomllib`.

    There is deliberately no second parser and no fallback. The previous design had a
    hand-rolled YAML reader whose honesty was established by differentially testing it
    against PyYAML — which is available exactly when the fallback is not needed, so with
    PyYAML absent the loose parser accepted whatever it managed to read. That is a weaker
    gate than the dependency it replaced. TOML is in the standard library, so the class is
    deleted rather than guarded.
    """
    if sys.version_info < (3, 11):
        raise SystemExit(
            "scripts/check-doc-evidence.sh needs Python 3.11+ for tomllib (found "
            f"{sys.version.split()[0]}). tomllib is standard library from 3.11; there is no "
            "fallback parser on purpose — see load_rows().")
    import tomllib
    with INDEX.open("rb") as fh:
        doc = tomllib.load(fh)
    rows = []

    def walk(node, path):
        if isinstance(node, dict):
            if "implementation" in node:
                rows.append((".".join(path), node.get("implementation"),
                             node.get("spec"), node.get("evidence") or []))
            else:
                for k, v in node.items():
                    walk(v, path + [k])

    walk({k: v for k, v in doc.items() if k != "meta"}, [])
    return rows, "tomllib (standard library, single parser)"


def list_gate_commands():
    """The DISTINCT commands `gate:` items cite, for scripts/gate-receipts.sh.

    Distinct: three rows cite `make selfhost`, and it is run once.
    """
    rows, _ = load_rows()
    seen = []
    for _, _, _, ev in rows:
        for item in ev:
            m = GATE_EVIDENCE.match(item) if item.startswith("gate:") else None
            if not m:
                continue
            cmd = item[len("gate:"):].split("->")[0].strip()
            if cmd not in seen:
                seen.append(cmd)
    return seen


def check_index(receipts=None):
    """-> (problems, row-count, how, counts)

    `counts` is printed by main(). Every `cmd:` item is RUN — there is no skip path, and
    that is deliberate: a skipped item that reports nothing is the same unmeasured
    denominator one layer down, and the conformance runner this file borrows its
    discipline from treats a fixture it cannot read as a failure rather than a skip
    (scripts/conformance.sh:589-594). An item the gate cannot run hermetically is a lint
    error naming the gate that owns the question, not a quiet exemption.
    """
    counts = {"cmd": 0, "conformance": 0, "src": 0, "gate": 0, "gate_validated": 0}
    if not INDEX.exists():
        return [f"feature-index: {INDEX} missing"], 0, "none", counts
    rows, how = load_rows()
    problems = []
    makefile = (ROOT / "Makefile").read_text(encoding="utf-8")
    manifest = load_manifest()
    if manifest is None:
        problems.append(
            f"conformance manifest {MANIFEST.relative_to(ROOT)} is missing, so no "
            f"`conformance:` evidence can be resolved. The corpus is a closed inventory; "
            f"without it this gate cannot know what a fixture is declared to do.")
    cmd_cache: dict = {}
    for name, impl, spec, ev in rows:
        if not isinstance(ev, (list, tuple)) or not ev:
            problems.append(f"{name}: evidence must be a non-empty list")
            continue
        for item in ev:
            if not TAGGED.match(item):
                problems.append(f"{name}: untagged evidence -> {item[:70]!r}")
            elif item.startswith("src:"):
                counts["src"] += 1
                m = SRC_EVIDENCE.match(item)
                if not m:
                    problems.append(
                        f"{name}: `src:` must cite a non-documentation repository file as "
                        f"path:line plus a symbol; anything under docs/ is circular "
                        f"-> {item[:70]!r}")
                else:
                    tgt = ROOT / m.group(1)
                    start, end = int(m.group(2)), int(m.group(3) or m.group(2))
                    if not tgt.exists():
                        problems.append(f"{name}: `src:` path missing -> {m.group(1)}")
                    else:
                        lines = tgt.read_text(encoding="utf-8",
                                              errors="replace").split("\n")
                        if start < 1 or end > len(lines):
                            problems.append(
                                f"{name}: `src:` line out of range -> {item[:60]!r}")
                        else:
                            body = norm("\n".join(lines[start - 1:end]))
                            # re.Match.end(n) returns -1 for a non-participating group, so
                            # `m.end(3) or m.end(2)` silently sliced to the LAST CHARACTER
                            # whenever the citation had no end line — which is most of them.
                            # The check ran and matched nothing for two rounds.
                            claim = item[m.end(3) if m.group(3) else m.end(2):]
                            for q in QUOTED.finditer(claim):
                                text = (q.group(1) or q.group(2) or q.group(3))
                                if norm(text) not in body:
                                    problems.append(
                                        f"{name}: `src:` cites {m.group(1)}:{start}-{end} but "
                                        f"the range does not contain the quoted claim "
                                        f"{text[:50]!r} — widen the range to include what is "
                                        f"being claimed")
            elif item.startswith("cmd:"):
                counts["cmd"] += 1
                problems.extend(check_cmd(name, item, cmd_cache))
            elif item.startswith("conformance:"):
                counts["conformance"] += 1
                m = CONF_EVIDENCE.match(item)
                if not m:
                    problems.append(
                        f"{name}: `conformance:` needs <path>.pd <class>, where <class> "
                        f"is what {MANIFEST.name} declares "
                        f"({' | '.join(CONF_CLASSES)}) -> {item[:70]!r}")
                elif not (ROOT / m.group(1)).exists():
                    problems.append(f"{name}: conformance fixture missing -> {m.group(1)}")
                elif manifest is not None:
                    declared = manifest.get(m.group(1))
                    if declared is None:
                        problems.append(
                            f"{name}: conformance fixture {m.group(1)} is not declared in "
                            f"{MANIFEST.name}, so no gate runs it — an undeclared fixture "
                            f"is outside the closed inventory and proves nothing")
                    elif declared[0] != m.group(2):
                        problems.append(
                            f"{name}: `conformance:` cites {m.group(1)} as "
                            f"{m.group(2)!r}, but {MANIFEST.name}:{declared[1]} declares "
                            f"it {declared[0]!r}. The manifest is what the gate runs; a "
                            f"row that reads PASS over a `vacuous` fixture is how a "
                            f"placeholder gets counted as coverage.")
            elif item.startswith("gate:"):
                counts["gate"] += 1
                m = GATE_EVIDENCE.match(item)
                body = item[len("gate:"):]
                gcmd, _, gresult = body.partition("->")
                gcmd, gresult = gcmd.strip(), gresult.strip()
                if not m:
                    problems.append(f"{name}: `gate:` needs a command and a result "
                                    f"-> {item[:70]!r}")
                elif m.group(1) and not re.search(rf"^{re.escape(m.group(1))}:",
                                                  makefile, re.M):
                    problems.append(f"{name}: no such make target -> {m.group(1)}")
                else:
                    # A result with nothing a machine can disagree with is prose, and
                    # prose is what let `-> total=42 pass=39` survive the output format
                    # that produced it. Same rule as `cmd:`: the result is a contract.
                    gkv, gq = gate_tokens(gresult)
                    toks = gkv + gq
                    if not toks:
                        problems.append(
                            f"{name}: `gate:` result carries nothing checkable — it needs "
                            f"at least one key=value or a quoted span that the gate's own "
                            f"output contains, so a changed number fails here instead of "
                            f"rotting -> {gresult[:70]!r}")
                    elif receipts is None:
                        pass        # counted, printed, and named by main(); never silent
                    elif gcmd not in receipts:
                        problems.append(
                            f"{name}: `gate:` cites {gcmd!r}, for which this run has no "
                            f"receipt. scripts/gate-receipts.sh runs every command the "
                            f"index cites; a command it did not run cannot be evidence.")
                    else:
                        grc, gout = receipts[gcmd]
                        if grc != 0:
                            problems.append(
                                f"{name}: `gate:` cites {gcmd!r}, which FAILED in this "
                                f"run (exit {grc}). A failing gate is not evidence for "
                                f"anything.")
                        else:
                            missing = gate_mismatches(gkv, gq, gout)
                            if missing:
                                problems.append(
                                    f"{name}: `gate:` cites {gcmd!r}, and this run does "
                                    f"not bear out:\n        "
                                    + "\n        ".join(missing)
                                    + f"\n      Re-derive the result from what the gate "
                                      f"actually reports.")
                            else:
                                counts["gate_validated"] += 1
        if impl not in ("implemented", "partial", "unimplemented"):
            problems.append(f"{name}: implementation={impl!r} not in vocabulary")
        # A BEHAVIOURAL CLAIM NEEDS EVIDENCE FROM A RUN.
        #
        # This is the durable form of the rule that refusing the literal names `pdc`,
        # `cargo` and `make` in a `cmd:` only approximates. That blocklist stops one
        # spelling; it does not stop a future author from deleting a compiler experiment,
        # replacing it with a source grep, and satisfying the schema — which is how this
        # file came to say a program "compiles, links, prints 99, no diagnostic" about a
        # program the compiler refuses.
        #
        # `implemented` and `partial` assert that pdc DOES something. Nothing static can
        # establish that: a source line proves a branch exists, not what happens when you
        # reach it. So those rows must carry at least one item that came from running the
        # compiler — a conformance fixture, or a gate whose output is checked against a
        # receipt. `unimplemented` is exempt on purpose: an absence is exactly what a
        # `cmd:` absence proof is for, and 16 rows legitimately rest on one.
        if impl in ("implemented", "partial"):
            if not any(str(x).startswith(("conformance:", "gate:")) for x in ev):
                problems.append(
                    f"{name}: implementation={impl!r} is a claim about what the compiler "
                    f"DOES, and every item here is static. A source citation proves a "
                    f"branch exists, not what happens when you reach it. Add a "
                    f"`conformance:` fixture that exercises it, or a `gate:` whose output "
                    f"says so.")
        if not spec:
            problems.append(f"{name}: no spec pointer")
    return problems, len(rows), how, counts


# --- RELOCATION BY CONTENT HASH -------------------------------------------------------
#
# THIS SECTION SUPERSEDES STEP 2 OF THE PROCEDURE IN THE MODULE DOCSTRING wherever it can:
# the hand search with `git show <base>:<path>` is now performed by the check itself, from
# the hash the pin already holds. Step 2 still governs the cases no unique match resolves.
#
# WHY THE DOCSTRING WAS NOT AMENDED IN PLACE, which is an exhibit rather than an oversight.
# The docstring is at the top of this file, so adding a line to it shifts every line below --
# including 327-329, which docs/contributing/claude-md-coverage.md cites and this repository
# pins. Applying that move means regenerating docs/citation-pins.tsv, which this branch is
# not permitted to write. So the amendment lives HERE, below the pinned range, because the
# constraint chose where the prose could go. That is the same pressure as the four source
# edits catalogued immediately below, arriving one layer up; it is recorded rather than
# absorbed, and it closes with one `--update` on a branch that may regenerate the pins.
#
# THE COST THIS REMOVES, MEASURED. A pin is keyed on (path, lines, citing-doc), so the LINE
# NUMBERS were the only way to find the cited range. Insert a line above a cited function and
# the pin fails, and the remedy this module's docstring prescribes (step 2) is to fetch the
# old text with `git show <base>:<path>` and search the working tree for it BY HAND. That is
# not a hypothetical cost; four edits landed on 2026-08-22 whose SHAPE was chosen by it:
#   * a comment written to an exact line budget so src/main.rs's pinned range would not shift;
#   * `compile_and_run` in src/driver/mod.rs defined AFTER the function it delegates to, so
#     that `let build_dir = ...` would stay on line 286, against ordinary reading order;
#   * `LC_ALL=C` set in `link` rather than in `link_command`, because that function's body is
#     content-pinned -- turning "the locale is fixed" from a property of the gcc invocation
#     into a property of one wrapper;
#   * a new verdict arm in scripts/conformance.sh paid for by DELETING an equal number of
#     lines elsewhere. The author's own words: "the constraint chose the edit."
# A gate that makes source code worse to keep itself green is charging more than it collects.
#
# WHAT IS DONE INSTEAD. The pin already stores sha256(norm(range))[:12]. That hash is an
# ADDRESS, not merely a tripwire: if the content is no longer at the pinned lines, the cited
# file is searched for a range that hashes to the same value.
#
#   exactly one match  -> RELOCATED, and a FAILURE. The text at the new lines IS the text
#                         the pin was taken from, character for character after whitespace
#                         normalisation, so the range MOVED rather than changed -- and the
#                         gate says WHERE TO. It still fails, because what a relocation
#                         establishes is that the CITING DOCUMENT IS WRONG: it names lines
#                         that now hold something else, and the pin's line numbers are
#                         stale. What the hash buys is the SIZE OF THE REPAIR, not its
#                         absence -- the author is handed the destination instead of
#                         searching for it with `git show <base>:<path>` by hand (step 2 of
#                         the procedure in this file's docstring). Printing that and exiting
#                         0 would have left a gate green over a document wrong on its face,
#                         which is the state this file exists to end; and a report nothing
#                         enforces is read once and then not at all.
#   several matches    -> AMBIGUOUS, and a FAILURE. Repeated boilerplate is real, and picking
#                         the first is how a citation silently lands on the wrong copy. The
#                         gate names every candidate and chooses none.
#   no match           -> the content CHANGED or is gone. A FAILURE, exactly as before.
#
# WHY THIS IS NOT THE LAUNDERING MACHINE THE DOCSTRING WARNS ABOUT. `--update` launders
# because it records a fingerprint for WHATEVER now sits at `path:line` -- any text at all,
# related or not. A relocation records nothing and proves something strictly narrower: hash
# equality over the normalised range. A range whose content changed hashes to something else
# and is offered no relocation; a coincidentally similar range is not a match, because the
# comparison is the whole range's digest and not a prefix, a proximity or a resemblance. And
# `--update` now REFUSES to regenerate over a pending relocation (see main), so the stale
# line numbers cannot be laundered into a pin by running the generator.
#
# THE HEIGHT OF THE RANGE IS HELD FIXED, and the reason is `norm()`. It collapses newlines
# along with every other run of whitespace, so a two-line range and the single line holding
# the same two statements hash IDENTICALLY. Searching every width would let `x.rs:10-11` come
# to rest on one line -- a different citation wearing the old one's numbers. A move preserves
# the shape; a reformat is a content change and fails.
#
# BLANK EDGES ARE TRIMMED BEFORE A WINDOW IS COUNTED, and that discharges the concession this
# paragraph used to close on ("a floor and not a fence: a window that ends on blank lines
# still normalises like the shorter window above it, so equal height is necessary for a
# relocation, not sufficient to make one unique"). `norm()` deletes a blank line outright, so
# a window that STARTS on one and the window one line below it that ENDS on one hold the SAME
# text and hash to the SAME value: both landed in `hits`, and the pin was called AMBIGUOUS
# between two ranges naming ONE piece of content. That ambiguity was manufactured by the
# search rather than found in the file, and AMBIGUOUS is a verdict the author cannot act on --
# there is no second copy to choose between. So each window is now named by its span with the
# blank edges removed, and windows sharing a trimmed span are ONE hit.
#
# THIS NARROWS THE REPORT AND NEVER THE TEST. The digest compared is unchanged and the search
# still visits exactly the windows of the pin's own height, so no pin gains a match it did not
# already have. Measured over the 380 checkable pins in docs/citation-pins.tsv (410 rows, 30
# naming files that do not exist here), 8 have a blank edge, and exactly one of those searches
# was ambiguous for this reason: src/codegen/mod.rs:4-10, matched both at 4-10 and at 3-9
# across the blank line at 3, is now the single hit 4-9. The other blank-edged pin with two
# matches is NOT of this kind and still fails: src/codegen/mod.rs:2770-2773 also matches
# 2952-2955, because the two blocks differ only in the indentation inside a string literal and
# `norm()` collapses that. Two distinct trimmed spans, two real copies, AMBIGUOUS as before.
#
# The trimmed span is also what a relocation REPORTS, and that is not a detail. Among windows
# holding one piece of content, picking whichever the loop reached first is the same arbitrary
# choice the AMBIGUOUS branch refuses to make; the trimmed span is the one name every member
# of that group agrees on, and it is the range an author should cite -- pinning it again puts
# the content at the pinned lines with no blank edge to shift.
#
# WHAT A RELOCATION DOES NOT ESTABLISH -- read this before reading it as a mere renumbering.
#   1. IT DOES NOT MAKE A WRONG CITATION RIGHT. A citation that pointed at the wrong code
#      before points at the same wrong code afterwards, in a new place. This removes the TAX,
#      not the CLASS: 25 of 220 citations were measured wrong, and two independent samples of
#      relocated citations this week found 3 of 10 and 5 of 12 content-wrong -- the arithmetic
#      was right every time and faithfully preserved citations that were already wrong. The
#      sibling proposal `docs/contributing/proposed-gate-quoted-token-citations.md` is what
#      addresses the class, by requiring the citing prose to quote a token the range contains.
#      The two are complementary and must not be conflated.
#   2. THE CITING DOCUMENT IS LEFT STALE, AND THAT IS WHY THE RUN IS RED. A relocation reports
#      that the content moved; it does not rewrite the citation, because a gate that edits
#      documentation is a gate nobody can review. Until an author applies the printed line
#      numbers, the document names lines that hold something else -- so the verdict is an
#      unfinished repair held red until it is finished, not a notice printed beside a pass.
#      What bounds the class it can report at all: the NON-SEMANTIC and delimiter-only floors
#      are checked BEFORE this, so the measured failure shape -- a citation coming to rest on
#      a blank line or a bare `}` -- still fails rather than relocating.
PIN_FINGERPRINT = re.compile(r"^[0-9a-f]{12}$")
RELOCATION_HIT_CAP = 8      # enough to say "several" and name them; not a full census


def pin_windows(lines, nlines):
    """Every candidate range of the pin's own height. -> (start, end, [line, ...])

    UNTRIMMED, and that is the contract rather than an accident of who calls it: this
    enumerates the ranges a pin of `nlines` lines could name, and the digest compared against
    the pin is taken over the WHOLE window — blank edges included — because that is what the
    pin was taken over. Trimming belongs to whoever NAMES a hit (locate_fingerprint), not to
    whoever offers a candidate; a generator that handed back trimmed spans would be answering
    a different question and would change which windows are compared at all.
    """
    for i in range(len(lines) - nlines + 1):
        yield i + 1, i + nlines, lines[i:i + nlines]


def trim_blank_edges(start: int, end: int, window):
    """A window's blank edges are no part of what it says. -> (start, end, [line, ...])

    `norm()` deletes blank lines, so ["", "x"], ["x", ""] and ["x"] all hash alike: a blank
    edge contributes nothing to the digest and nothing to a citation. The trimmed span is
    therefore the one name every window over the same content agrees on, and it is what a
    relocation dedupes on and reports. A window that is blank THROUGHOUT names no content, so
    there is nothing to trim to and it is returned as it came.
    """
    lo, hi = 0, len(window)
    while lo < hi and not window[lo].strip():
        lo += 1
    while hi > lo and not window[hi - 1].strip():
        hi -= 1
    if lo == hi:
        return start, end, window
    return start + lo, end - (len(window) - hi), window[lo:hi]


def locate_fingerprint(lines, want_fp: str, nlines: int, cap: int = RELOCATION_HIT_CAP):
    """Every DISTINCT range in `lines` whose fingerprint is `want_fp`. -> [(start, end, text)]

    Distinct after blank edges are trimmed: overlapping windows that differ only in where
    their blank line sits hold one piece of content, and counting them as two matches is how
    a unique relocation was reported AMBIGUOUS. See THE HEIGHT OF THE RANGE IS HELD FIXED.
    The height searched is still exactly the pin's; trimming names a hit, it does not admit
    one — the comparison happens on the untrimmed window pin_windows yields, and only a
    window that already matched is then named by its trimmed span.
    """
    hits: list = []
    seen: set = set()
    if nlines < 1 or nlines > len(lines):
        return hits
    for start, end, window in pin_windows(lines, nlines):
        body = "\n".join(window)
        if fingerprint(body) == want_fp:
            tstart, tend, twindow = trim_blank_edges(start, end, window)
            if (tstart, tend) in seen:
                continue
            seen.add((tstart, tend))
            hits.append((tstart, tend, "\n".join(twindow)))
            if len(hits) >= cap:
                break
    return hits


def span_height(span: str) -> int | None:
    """`'286-288'` -> 3. None when the span is not a range this gate wrote."""
    try:
        start, end = (int(v) for v in span.split("-", 1))
    except ValueError:
        return None
    return end - start + 1 if end >= start >= 1 else None


def relocation_hits(path_str: str, span: str, want_fp: str):
    """Where a pin's stored content is NOW. -> [(start, end, text)], possibly empty.

    Empty for a sentinel fingerprint (MISSING-FILE, OUT-OF-RANGE, NON-SEMANTIC): those are
    not digests, so nothing can hash to them and no relocation can be offered.
    """
    if not PIN_FINGERPRINT.match(want_fp or ""):
        return []
    target = resolve(path_str)
    if target is None or not target.exists():
        return []
    nlines = span_height(span)
    if nlines is None:
        return []
    lines = target.read_text(encoding="utf-8", errors="replace").split("\n")
    return locate_fingerprint(lines, want_fp, nlines)


def report_relocations(relocated) -> None:
    """Print the moves. Every one of them ALSO fails the run; see RELOCATION BY CONTENT HASH.

    The excerpt of what was FOUND is printed beside the excerpt that was PINNED, for the
    same reason the pin file carries an excerpt column at all: hash equality is a machine's
    statement about NORMALISED text — norm() collapses every run of whitespace, including
    inside a string literal, so two ranges differing only in indentation hash alike — and only
    a reader can say whether the claim still holds. Here the two excerpts must read
    identically; if they do not, the digest is not what it seems.
    """
    if not relocated:
        return
    print(f"\n{len(relocated)} citation(s) RELOCATED — the pinned content is unchanged "
          f"AFTER WHITESPACE NORMALISATION and unique, at different lines:")
    for (p, s, d), (_, pinned_x), (ns, ne, body) in relocated:
        print(f"  {p}:{s} -> {p}:{ns}-{ne}   (cited by {d})")
        print(f"      pinned: {pinned_x}")
        print(f"      found:  {excerpt(body)}")
    print("  Each move is justified by sha256(norm(range)) equality and by there being "
          "exactly ONE such range,\n"
          "  so the text is unchanged AFTER NORMALISATION — it moved. norm() collapses every "
          "run of whitespace,\n"
          "  including inside string literals, so equal digests mean equal normalised text "
          "and not equal bytes:\n"
          "  read the two excerpts. The printed range may also be SHORTER than the one the "
          "author wrote, because a\n"
          "  blank edge is no part of the pinned content. THE CITING DOCUMENT STILL NAMES THE "
          "OLD LINES: rewrite it\n"
          "  to the new ones and re-run --update. A relocation says the cited code still "
          "exists; it does not say the\n"
          "  claim made about it was ever right (see the module docstring, and the sibling "
          "quoted-token proposal).")


def read_pins():
    want = {}
    for line in PINS.read_text(encoding="utf-8").split("\n"):
        if not line or line.startswith("#"):
            continue
        parts = line.split("\t")
        if len(parts) < 4:
            continue
        p, span, d, f = parts[:4]
        want[(p, span, d)] = (f, parts[4] if len(parts) > 4 else "")
    return want


def main() -> int:
    global INDEX, PINS, ALLOW
    update = "--update" in sys.argv
    # `--pins` / `--allow` / `--pins-only` are the citation half's version of the
    # `--index` seam directly below, and they exist for the same reason: the only
    # way to know a gate still has an exit code is to hand it something wrong and
    # require it to say so. scripts/test-doc-evidence.sh points them at a
    # throwaway pin file and a throwaway doc that cites a bare `}` — the
    # configuration `--update` used to record silently — and requires this to go
    # red. Pointing the generated files elsewhere is what keeps that control from
    # rewriting the tracked ones.
    if "--pins" in sys.argv:
        PINS = Path(sys.argv[sys.argv.index("--pins") + 1]).resolve()
    if "--allow" in sys.argv:
        ALLOW = Path(sys.argv[sys.argv.index("--allow") + 1]).resolve()
    # `--index` / `--index-only` exist for scripts/test-doc-evidence.sh, which points the
    # evidence checks at a throwaway index containing a KNOWN-FALSE `cmd:` item and
    # requires this gate to go red on it. A gate is worth its exit code, and the only way
    # to know this one still has one is to break something on purpose.
    # (Same purpose as CONFORMANCE_MANIFEST in scripts/conformance.sh.)
    if "--index" in sys.argv:
        INDEX = Path(sys.argv[sys.argv.index("--index") + 1]).resolve()

    # scripts/gate-receipts.sh asks for the distinct commands, runs each once, then hands
    # the receipts back. The receipts are read only when that same invocation passes the
    # directory, so a stale directory from an earlier run cannot be picked up by accident.
    # `--classify-target` exists for scripts/test-doc-evidence.sh. The delimiter-only
    # floor lives on the citation-pin path, which needs the real docs corpus and is
    # therefore unreachable from `--index-only` — the mode every other control uses. So
    # the PREDICATE is addressable on its own, and the harness asserts the class over
    # inputs it supplies rather than over whatever the corpus happens to contain today.
    # That is the point of the change it guards: the corpus is not a specification of
    # which lines are delimiter-only.
    if "--classify-target" in sys.argv:
        text = sys.argv[sys.argv.index("--classify-target") + 1]
        print("delimiter-only" if is_delimiter_only(text) else "substantive")
        return 0

    if "--list-gate-commands" in sys.argv:
        for c in list_gate_commands():
            print(c)
        return 0
    receipts = None
    if "--gate-receipts" in sys.argv:
        rdir = Path(sys.argv[sys.argv.index("--gate-receipts") + 1])
        receipts, rerr = load_receipts(rdir)
        if rerr:
            print(f"FAIL:\n  {rerr}")
            return 1

    if "--self-test-counts" in sys.argv:
        return self_test_counts()

    if "--index-only" in sys.argv:
        problems, nrows, how, counts = check_index(receipts)
        print(f"feature-index rows: {nrows} via {how}")
        print(f"cmd: evidence executed: {counts['cmd']}")
        print(f"gate: evidence validated against this run: "
              f"{counts['gate_validated']}/{counts['gate']}"
              + ("" if receipts is not None else "  (no receipts passed)"))
        if problems:
            print("\nFAIL:")
            for p in problems:
                print(f"  {p}")
            return 1
        print("feature-index evidence: OK")
        return 0

    cites = collect_citations()
    fences = collect_fences()
    conts = collect_continuations()
    enum_repeats = collect_enumeration_repeats()
    violations = collect_normative_violations()

    if update:
        # REFUSED BEFORE ANYTHING IS WRITTEN. `--update` is the laundering
        # machine the docstring warns about, and a content-free target is the
        # one laundering a machine can recognise: recording it would put a
        # fingerprint that can never change into the pin file and close the
        # question. So the pins are not regenerated at all while one exists.
        nonsemantic = [(p, s, d, x) for p, s, d, f, x in cites
                       if f == "NON-SEMANTIC"]
        if nonsemantic:
            print(f"REFUSED: {len(nonsemantic)} citation(s) point at a range with no "
                  f"content. Nothing was written.")
            for p, s, d, x in nonsemantic:
                print(f"  {p}:{s}  (cited by {d})  is {x!r}")
            print("\nA blank line or a bare `}` is fingerprint-stable, so a pin on one "
                  "can never move\nand never be wrong. Correct the citation by CONTENT "
                  "first — see this file's docstring,\nstep 2 — then re-run --update.")
            return 1

        old = read_pins() if PINS.exists() else {}
        new = {(p, s, d): (f, x) for p, s, d, f, x in cites}
        changed = [(k, old[k], v) for k, v in sorted(new.items())
                   if k in old and old[k][0] != v[0]]
        added = [k for k in sorted(new) if k not in old]
        removed = [k for k in sorted(old) if k not in new]
        # REFUSED BEFORE ANYTHING IS WRITTEN, and this is what keeps relocation from being a
        # way IN to the laundering machine. The check FAILS on a relocation, naming the lines
        # to rewrite the citation to and telling the author to re-run `--update` — so running
        # `--update` is exactly what someone does next, and doing it BEFORE the citation is
        # rewritten is the wrong half of that instruction. In that state the generator would
        # record a fingerprint for whatever moved into the old lines, while the document still
        # names them — converting "this citation's address is stale" into "the gate is
        # satisfied", under a key that looks untouched. So a pin whose old content is still
        # findable, uniquely, is a MOVE THAT HAS NOT BEEN APPLIED YET, and the generator will
        # not run until the document names the lines the content is actually at.
        #
        # A pin whose content genuinely changed is NOT caught here (no hit, nothing unique),
        # which is the case `--update` exists for: it records the change and prints MOVED for
        # a human to read. That distinction is the whole point — the hash tells the two apart.
        pending = [(k, o, hits[0]) for k, o, _ in changed
                   if len(hits := relocation_hits(k[0], k[1], o[0])) == 1]
        if pending:
            print(f"REFUSED: {len(pending)} citation(s) have MOVED to lines the citing "
                  f"document does not name yet. Nothing was written.")
            for (p, s, d), (_, pinned_x), (ns, ne, body) in pending:
                print(f"  {p}:{s} -> {p}:{ns}-{ne}  (cited by {d})")
                print(f"      pinned: {pinned_x}")
                print(f"      found:  {excerpt(body)}")
            print("\nRecording the pins now would fingerprint whatever moved INTO the old "
                  "lines and close the\nquestion under an unchanged key. Rewrite each "
                  "citation to the lines above — the content is\nproved identical by hash, "
                  "so this is a renumbering and not a judgement — then re-run --update.")
            return 1
        # A RE-SNAPSHOT IS NOT A RECORD OF A CHANGE, and until this existed `--update` could
        # not tell the two apart. `pending` above catches the one shape where the KEY survives
        # and the old content is still findable. Both halves of that are load-bearing, and both
        # were walked around on 2026-08-26 by commit f8b5ff1: the citing documents were
        # RENUMBERED first — a citation naming line 1161 of the type checker was rewritten to
        # name line 1189 — so every old key was DROPPED and a new one ADDED, while `changed`
        # was empty and `pending` was empty,
        # and `--update` fingerprinted whatever now sat at the new lines. Eleven citations came
        # to rest on `generic_structs: HashMap::new(),`, `a.0.cmp(&b.0)` and a guard comment,
        # the gate went green by construction, and the only thing that caught it was a reviewer
        # reading the TSV diff. NON-SEMANTIC did not fire either: the wrong lines all carried
        # real code.
        #
        # So the two shapes are named together, because they are one question — DOES THIS RUN
        # RECORD A DIFFERENT PIECE OF CONTENT UNDER A CITATION THAT ALREADY HAD ONE? —
        # and the key is the wrong place to ask it:
        #
        #   RE-PINNED   same key, different fingerprint, and no unique relocation (so `pending`
        #               declined it). The content at the cited lines changed. That may be
        #               right, but it is a READING, and this file's whole thesis is that a
        #               reading is not a thing a generator may perform silently.
        #   RENUMBERED  the key changed because the DOCUMENT changed its line numbers, while
        #               the pinned content is still in the file, uniquely, at lines the
        #               document does NOT now name. That is a citation moved AWAY from its own
        #               content — the f8b5ff1 shape — and the destination is printed, so the
        #               repair is a renumbering rather than a search.
        #   REPLACED /  the document renumbered AND the old content is at ZERO or SEVERAL
        #   AMBIGUOUS   places, so there is no destination to hand anyone. Round 4 of external
        #               review found this falling THROUGH: the first version of this loop read
        #               `if len(hits) != 1: continue`, which is the compound laundering —
        #               renumber the citation and edit the pinned line in the SAME run — and a
        #               non-unique move, both walking straight past the guard to
        #               PINS.write_text. A guard whose unhandled cases fall through to the
        #               write is a guard for the cases somebody thought of.
        #
        # OUT OF SCOPE, DELIBERATELY AND WITH A NUMBER: a citation MERGED onto a span the same
        # document already cites (`foo.rs:10` rewritten to `foo.rs:20` where `20` was already
        # pinned) produces a removal with NO addition, so the `added_now` test below reads it
        # as an outright deletion and records it — while the content of `10` is still uniquely
        # in the file and the document no longer names it. Raised by round 4 of review, and
        # UNADOPTED, and that is a different word from unreachable — see below:
        #   * a MERGE and an ORDINARY DELETION are byte-identical in the pin diff (one key
        #     removed, none added, content still findable), so refusing one refuses both, and
        #     documents do legitimately drop citations whose targets still exist;
        #   * so the pin-level predicate is far too wide: 291 of 420 pins sit in a (file,
        #     document) pair with a second pin AND have uniquely locatable content, which means
        #     DELETING ANY ONE OF THOSE 291 WOULD BE REFUSED. That is an ELIGIBILITY count over
        #     the corpus, not a rate at which anyone deletes anything — what it measures is how
        #     much of the corpus the predicate would put behind the flag;
        #   * A DISCRIMINATOR DOES EXIST, IN INPUTS THIS FUNCTION ALREADY READS. An earlier
        #     version of this comment said the opposite — that closing the shape needed a
        #     BEFORE count the pin file cannot supply, so it started with a schema change or
        #     with git. The BEFORE count is indeed absent, but the conclusion did not follow:
        #     `collect_citations` reads the raw documents and discards MULTIPLICITY only at its
        #     last line (`return sorted(set(out))`), so an AFTER-state proxy is available
        #     without reading anything new — a merge LEAVES A DUPLICATE, and the duplicate is
        #     visible now. Refuted by round-5 review; recorded because a false impossibility
        #     claim is worse than the hole it was excusing.
        #   * MEASURED on this corpus, 2026-08-27, and REGENERATED BY
        #     `scripts/measure-g1-surface.py` — every figure below is that script's output,
        #     including the per-document breakdown this comment does not repeat:
        #     465 textual citations collapse to 420
        #     distinct, and 42 (file, span, document) triples are already cited twice or more.
        #     124 pins sit in a pair that holds a duplicate, and that is an UPPER BOUND, not
        #     the surface — an earlier version of this comment shipped it as the surface. A
        #     removal event takes ALL textual occurrences of one pin, so the after-state has
        #     to be simulated per candidate, and two groups drop out: −22 pins that are the
        #     SOLE duplicated triple of their pair (removing one destroys the only witness
        #     conjunct 3 could have had — the same "cannot be its own duplicate" reasoning,
        #     applied one step further), and −12 whose content is not uniquely locatable, so
        #     conjunct 2 is false. SURFACE = 90 of 420, 21.4%, over 7 documents. Stated as
        #     ELIGIBILITY and not as a rate: DELETING ANY ONE OF THOSE 90 WOULD BE REFUSED.
        #     How often anyone deletes a citation is not measured here and no number in this
        #     comment is about it. That is why it is not adopted YET, not why it cannot be.
        # The harm — two claims resting on one range — is owned above by
        # `collect_enumeration_repeats`, whose window was itself narrowed by measurement, and it
        # is recorded as debt in
        # docs/contributing/citation-and-predicate-debt.md, and pinned as a `guard` control in
        # scripts/test-doc-evidence.sh so widening the scope cannot happen quietly.
        #
        # `--allow-repin` is the deliberate step. It is not a way to skip the reading; it is
        # where the reading is asserted, and it belongs in a commit message beside which
        # citations were re-read and why. A legitimate re-pin is common — editing a line INSIDE
        # a cited range is one — and it is exactly as common as the laundering, which is why
        # the flag records a decision instead of the generator guessing there was one.
        allow_repin = "--allow-repin" in sys.argv
        repinned, renumbered, replaced = changed, [], []
        # The spans this run would NEWLY pin, per (cited file, citing document). Not every
        # span the document names: a document citing one file twenty times would print
        # twenty numbers, nineteen of them untouched by this run, and a report that wide is
        # one nobody finishes reading. The ADDED spans are the renumbering itself.
        added_now: dict = {}
        for p, s, d in added:
            added_now.setdefault((p, d), set()).add(s)
        for k in removed:
            p, s, d = k
            # A citation DELETED outright drops its key too, and its content is usually still
            # in the file. That is not a renumbering and there is nothing to repair, so the
            # shape is narrowed to a document that gave the same FILE new line numbers in
            # this same run: only then has it chosen numbers, and only then can they be wrong.
            if (p, d) not in added_now:
                continue
            # A PIN THAT NEVER HELD CONTENT HAS NONE TO REPLACE. MISSING-FILE and
            # OUT-OF-RANGE are sentinels rather than digests, so nothing can hash to them and
            # all three verdicts below are unanswerable for such a row — flagging one would
            # accuse a document of laundering a fingerprint that was never taken.
            # (NON-SEMANTIC never reaches the pin file: the refusal above returns first.)
            if not PIN_FINGERPRINT.match(old[k][0] or ""):
                continue
            fresh = sorted(added_now[(p, d)])
            hits = relocation_hits(p, s, old[k][0])
            # ZERO. The document renumbered AND the pinned text is gone from the file: the
            # compound move, where each half alone is handled and the pair used to be
            # handled by neither. Nothing can be printed as a destination, so nothing is —
            # and that is exactly why it may not be recorded silently.
            if not hits:
                replaced.append((k, old[k], [], fresh))
                continue
            # SEVERAL. The text is still there and no longer identifies a place. Picking the
            # first is how a citation lands on the wrong copy, which the check's own
            # AMBIGUOUS verdict refuses to do; the generator does not get to do it either.
            if len(hits) > 1:
                replaced.append((k, old[k], hits, fresh))
                continue
            ns, ne, _ = hits[0]
            if f"{ns}-{ne}" in added_now[(p, d)]:
                continue        # the new numbers ARE where the content went: a correct repair
            renumbered.append((k, old[k], hits[0], fresh))
        if (repinned or renumbered or replaced) and not allow_repin:
            print(f"REFUSED: {len(repinned)} citation(s) would be RE-PINNED to different "
                  f"content, {len(renumbered)} RENUMBERED away from theirs, and "
                  f"{len(replaced)} REPLACED or AMBIGUOUS. Nothing was written.")
            for (p, s, d), (_, was_x), (_, now_x) in repinned:
                print(f"  re-pinned  {p}:{s}  (cited by {d})")
                print(f"      was: {was_x}")
                print(f"      now: {now_x}")
            for (p, s, d), (_, was_x), (ns, ne, body), fresh in renumbered:
                print(f"  renumbered {p}:{s}  (cited by {d})")
                print(f"      the document now names {', '.join(fresh)}, and this pin's "
                      f"content is at {ns}-{ne}")
                print(f"      pinned at {s}: {was_x}")
                print(f"      that text is at {ns}-{ne}: {excerpt(body)}")
                for f_s in fresh:
                    n_fp, n_x = new[(p, f_s, d)]
                    # `or n_fp`: a MISSING-FILE or OUT-OF-RANGE row has an empty excerpt, and
                    # printing nothing at all reads as "it would pin a blank line".
                    print(f"      {f_s} would be pinned as: {n_x or n_fp}")
            for (p, s, d), (_, was_x), hits, fresh in replaced:
                where = ("nowhere in the file" if not hits else
                         "at " + ", ".join(f"{a}-{b}" for a, b, _ in hits))
                print(f"  replaced   {p}:{s}  (cited by {d})")
                print(f"      the document now names {', '.join(fresh)}, and this pin's "
                      f"content is {where}")
                print(f"      pinned at {s}: {was_x}")
                for a, b, body in hits:
                    print(f"      candidate {a}-{b}: {excerpt(body)}")
                for f_s in fresh:
                    n_fp, n_x = new[(p, f_s, d)]
                    print(f"      {f_s} would be pinned as: {n_x or n_fp}")
            print("\nRead each pair. If the citation still names the code its prose is about, "
                  "re-run with\n--allow-repin and say in the commit message which readings you "
                  "made. If it does not,\nthe CITING DOCUMENT is what changes — for a "
                  "renumbering the destination is printed above,\nand the content is proved "
                  "identical by hash, so that repair is arithmetic and not judgement.")
            if replaced:
                print("\nA REPLACED/AMBIGUOUS row carries LESS than that, and the wording is "
                      "deliberate. With the\nold content at zero or several places there is no "
                      "destination to hand you, so NOTHING HERE\nPROVES THE DROPPED CITATION "
                      "AND THE ADDED ONE ARE THE SAME CITATION: an unrelated deletion\nand an "
                      "unrelated addition, in one document, naming one file, in one run, land "
                      "here too, and\nso does a citation whose target was renumbered and "
                      "rewritten together. This run cannot tell\nthose apart — which is the "
                      "reason it refuses rather than reporting. Read the rows, then\neither "
                      "correct the citation or re-run with --allow-repin.")
            return 1
        PINS.write_text(
            "# GENERATED by scripts/check-doc-evidence.sh --update. Do not edit by hand.\n"
            "# A fingerprint proves a cited range has not moved. It CANNOT prove the range\n"
            "# still supports the claim made about it — that is a reading. The excerpt\n"
            "# column exists so a reviewer can spot-check meaning without a source checkout.\n"
            "# path\tlines\tciting-doc\tfingerprint\texcerpt\n"
            + "".join(f"{p}\t{s}\t{d}\t{f}\t{x}\n" for p, s, d, f, x in cites),
            encoding="utf-8")
        ALLOW.write_text(
            "# GENERATED by scripts/check-doc-evidence.sh --update. Do not edit by hand.\n"
            "# Every ```palladium no-compile fence, per file, indented fences included.\n"
            "# check-docs.sh reports the total but pins nothing, so without this the count\n"
            "# can drift upward while the gate stays green.\n"
            + "".join(f"{p}\t{n}\n" for p, n in fences), encoding="utf-8")
        print(f"updated: {len(cites)} citation pins, "
              f"{sum(n for _, n in fences)} fences across {len(fences)} files")
        if changed:
            print(f"\n{len(changed)} citation(s) MOVED. Read these — --update can launder a\n"
                  f"citation that now points at unrelated code, and only you stand between:")
            for (p, s, d), o, n in changed:
                print(f"  {p}:{s}   (cited by {d})")
                print(f"      was: {o[1]}")
                print(f"      now: {n[1]}")
        for k in added:
            print(f"  + new pin  {k[0]}:{k[1]}  in {k[2]}")
        for k in removed:
            print(f"  - dropped  {k[0]}:{k[1]}  in {k[2]}")
        return 0

    fail = []
    # A pin whose content, after whitespace normalisation, is provably still in the same
    # file at other lines. Every entry
    # here ALSO has one in `fail`: the two lists are separate so the moves print together as
    # their own section, ahead of the failure list, and not so that one of them escapes the
    # exit code. A relocation that appeared here and nowhere in `fail` is a stale pin the gate
    # forgave — which is what this list did until it was changed.
    relocated: list = []

    # The citation half on its own. The full run executes 42 `cmd:` items, which
    # is far too expensive for a control that has to run several configurations
    # of one probe document.
    pins_only = "--pins-only" in sys.argv

    if not PINS.exists():
        fail.append(f"missing {PINS} (run --update)")
    else:
        want = read_pins()
        have = {(p, s, d): (f, x) for p, s, d, f, x in cites}
        for key, (f, x) in sorted(have.items()):
            if f == "NON-SEMANTIC":
                fail.append(
                    f"citation {key[0]}:{key[1]} in {key[2]} -> NON-SEMANTIC: the cited "
                    f"range is {x!r}, which carries no content. A pin on a blank line or "
                    f"on punctuation is fingerprint-stable forever, so it can never MOVE "
                    f"and never be wrong — which is exactly how a citation that drifted "
                    f"onto one stayed green. Cite the code the prose is about.")
            elif f in ("MISSING-FILE", "OUT-OF-RANGE"):
                fail.append(f"citation {key[0]}:{key[1]} in {key[2]} -> {f}")
            elif is_delimiter_only(x):
                # A CITATION WHOSE TARGET IS BLANK IS NOT A WEAK CITATION; IT IS AN
                # ABSENT ONE. Everything else here checks that a cited range has not
                # MOVED, and a fingerprint of "" or "}" is perfectly stable — so sixteen
                # citations pointing at whitespace and bare braces sat green here
                # indefinitely, seven of them in a specification paragraph whose every
                # factual claim had since become false. Pinning one is worse than leaving
                # it unpinned: it converts "nobody checked this" into "the gate is
                # satisfied". `--update` cannot launder it, because --update returns
                # before this branch and the next verifying run fails on the new pin.
                #
                # This is a floor, not a semantic check. A range CAN be non-blank and
                # still not support its claim — that reading is the reviewer's job, and
                # the excerpt column exists for it (see the header written into
                # docs/citation-pins.tsv). What the floor removes is the case where there
                # is provably nothing to read.
                fail.append(
                    f"citation {key[0]}:{key[1]} in {key[2]} targets {x.strip()!r} — "
                    f"a blank line or a bare delimiter supports no claim. Repoint it at "
                    f"the line that does, or delete the claim if no line does.")
            elif key not in want:
                fail.append(f"citation {key[0]}:{key[1]} in {key[2]} is unpinned "
                            f"(run --update)")
            elif want[key][0] != f:
                # THE PINNED LINES NO LONGER HOLD THE PINNED CONTENT. Before failing on the
                # line numbers, ask the only question the stored hash can answer: is that
                # content still in this file? See RELOCATION BY CONTENT HASH above for what
                # each of the three answers proves and, more importantly, does not.
                hits = relocation_hits(key[0], key[1], want[key][0])
                if len(hits) == 1:
                    relocated.append((key, want[key], hits[0]))
                    ns, ne, _ = hits[0]
                    fail.append(
                        f"citation {key[0]}:{key[1]} cited by {key[2]} MOVED, and its "
                        f"pinned content is unchanged after whitespace normalisation and "
                        f"unique at {key[0]}:{ns}-{ne} — RELOCATED\n"
                        f"      pinned:          {want[key][1]}\n"
                        f"      at {key[1]} it is now: {x}\n"
                        f"      Hash equality says the text is unchanged AFTER NORMALISATION "
                        f"— norm() collapses every run of\n"
                        f"      whitespace, including inside a string literal — so this is a "
                        f"renumbering and not a judgement,\n"
                        f"      but the citing document still names {key[1]}, which holds "
                        f"something else, and the pin's line\n"
                        f"      numbers are stale. Rewrite the citation to {ns}-{ne} — which "
                        f"may be SHORTER than what you\n"
                        f"      wrote, since a blank edge is no part of the pinned content — "
                        f"then re-run --update to\n"
                        f"      re-stamp the pin.")
                elif len(hits) > 1:
                    where = ", ".join(f"{a}-{b}" for a, b, _ in hits)
                    fail.append(
                        f"citation {key[0]}:{key[1]} cited by {key[2]} MOVED, and its "
                        f"pinned content now appears at {len(hits)} places in {key[0]} "
                        f"({where}) — AMBIGUOUS\n"
                        f"      pinned: {want[key][1]}\n"
                        f"      now:    {x}\n"
                        f"      The gate names every candidate and chooses none: repeated "
                        f"boilerplate is real, and picking\n"
                        f"      the first is how a citation silently comes to rest on the "
                        f"wrong copy. Repoint the citation\n"
                        f"      by hand at the one the prose is about, then re-run --update.")
                else:
                    # ZERO IS A FAILURE, and it is the fail-closed half of this mechanism.
                    # No range in the file hashes to the pin, so the text CHANGED rather
                    # than moved — and a changed range is exactly what this gate exists to
                    # report. A relocation is offered only against hash equality, never
                    # against a resemblance, so nothing here can drift onto a near-match.
                    n = span_height(key[1])
                    fail.append(
                        f"citation {key[0]}:{key[1]} cited by {key[2]} MOVED\n"
                        f"      pinned: {want[key][1]}\n"
                        f"      now:    {x}\n"
                        f"      Its pinned content no longer appears anywhere in {key[0]} "
                        f"as a {n}-line range, so the text\n"
                        f"      CHANGED rather than moved. Re-read the claim against the "
                        f"code as it is now: if the code\n"
                        f"      changed meaning, the citation is what changes, not the pin.")
        for key in sorted(want):
            if key not in have:
                fail.append(f"pinned citation {key[0]}:{key[1]} in {key[2]} no longer cited "
                            f"(run --update)")

    if pins_only:
        print(f"citations pinned:   {len(cites)} (whole cited range fingerprinted)")
        report_relocations(relocated)
        if fail:
            print("\nFAIL:")
            for f in fail:
                print(f"  {f}")
            return 1
        print("citation pins: OK")
        return 0

    if not ALLOW.exists():
        fail.append(f"missing {ALLOW} (run --update)")
    else:
        want_f = {}
        for line in ALLOW.read_text(encoding="utf-8").split("\n"):
            if not line or line.startswith("#"):
                continue
            p, n = line.split("\t")
            want_f[p] = int(n)
        have_f = dict(fences)
        for p in sorted(set(want_f) | set(have_f)):
            if want_f.get(p, 0) != have_f.get(p, 0):
                fail.append(f"no-compile fences in {p}: allowed {want_f.get(p, 0)}, "
                            f"found {have_f.get(p, 0)} — justify and --update")

    for rel, n, tok, repl, sec in violations:
        fail.append(f"{rel}:{n} uses {tok!r} in a normative region — §{sec} replaced it with "
                    f"{repl}. A document carrying the NORMATIVE banner defines the language "
                    f"and cannot contradict the specification's surface.")

    for rel, ctx in conts:
        fail.append(f"unpinnable citation shorthand in {rel}: ...{ctx} — write the full path; "
                    f"a bare `:LINE` gets no pin and no movement check")

    for rel, line, cit in enum_repeats:
        fail.append(f"{rel}:{line} cites {cit} twice inside one enumeration — a list of "
                    f"citations asserts that many DISTINCT sites, so at most one of these "
                    f"can be right. This is the shape a relocation leaves behind when its "
                    f"key is not injective; the movement check cannot see it, because both "
                    f"citations agree with the pin they were rewritten to.")

    problems, nrows, how, counts = check_index(receipts)
    conf_problems, conf_checked, conf_counts = check_conformance_counts()
    fail.extend(conf_problems)
    fail.extend(problems)

    print("=" * 62)
    print(f"citations pinned:   {len(cites)} (whole cited range fingerprinted)")
    print(f"no-compile fences:  {sum(n for _, n in fences)} across {len(fences)} file(s)")
    print(f"unpinnable shorthands: {len(conts)}")
    print(f"citations repeated in one enumeration: {len(enum_repeats)}")
    print(f"normative-surface violations: {len(violations)}")
    print(f"feature-index rows: {nrows} via {how}"
          + (", all evidence tagged and resolved" if not problems
             else f", {len(problems)} evidence problem(s)"))
    # Printed, because the denominator IS the finding: for as long as this said nothing,
    # all 53 `cmd:` items were unexecuted text. `cmd:` has no skip column — it is hermetic
    # and cheap, so there is no reason to skip one. `gate:` does have a seam, because a
    # gate needs a build and this is a lint: when the receipts are absent the count says
    # so and names the command that produces them. `make gates` runs that command, so the
    # target that CERTIFIES has no unvalidated gate evidence; only the lint does.
    gv = counts["gate_validated"]
    gate_note = (f"{gv}/{counts['gate']} validated against this run's receipts"
                 if receipts is not None else
                 f"{counts['gate']}, NONE validated -- run `make gate-receipts`")
    print(f"evidence items: cmd={counts['cmd']} (all EXECUTED, none skipped) "
          f"src={counts['src']} conformance={counts['conformance']} gate={gate_note}")
    # The denominator IS the finding here too: for as long as this said nothing, three
    # prose sites quoting the conformance summary were checked by nobody, and each corpus
    # change meant editing them by hand. `feature-index.toml`'s `gate:` strings are NOT
    # counted here -- they are gate-receipts', validated against a real run.
    if conf_counts is None:
        conf_note = "the manifest could not be read"
    else:
        conf_note = (f"{conf_checked}/{len(conformance_claims(conf_counts))} prose site(s) "
                     f"agree with {MANIFEST.name} recounted "
                     f"(total={conf_counts['total']} run={conf_counts['run']} "
                     f"reject={conf_counts['reject']} vacuous={conf_counts['vacuous']} "
                     f"xfail={conf_counts['xfail']} skip={conf_counts['skip']}); "
                     f"the feature index's `gate:` strings are `make gate-receipts`'")
    print(f"conformance-count claims: {conf_note}")
    print("=" * 62)
    report_relocations(relocated)
    if fail:
        print("\nFAIL:")
        for f in fail:
            print(f"  {f}")
        return 1
    print("doc evidence: OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
