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
"""
from __future__ import annotations

import hashlib
import os
import re
import shlex
import subprocess
import sys
import tempfile
from pathlib import Path

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
CMD_BUILD_ARTIFACT = ("target/", "build_output/", "./target/", "./build_output/")

# Shell control operators. shlex(punctuation_chars=True) surfaces these as their own
# tokens ONLY when unquoted, so `grep -nE '#\[token\("(with|effect)"\)\]' f` keeps its
# parenthesised alternation while `grep x src/; rm -rf y` is refused.
CMD_OPERATORS = {";", "&", "&&", "||", "<", ">", ">>", "<<", "(", ")"}

CONF_CLASSES = ("run", "untranscribed", "vacuous", "xfail", "reject", "skip")
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


def norm(text: str) -> str:
    return " ".join(text.split())


def fingerprint(text: str) -> str:
    return hashlib.sha256(norm(text).encode()).hexdigest()[:12]


def excerpt(text: str, width: int = 160) -> str:
    """A bounded excerpt that shows BOTH ends of a cited range.

    Showing only the first 100 characters made the load-bearing later lines of a range
    citation invisible, which is how `src/typeck/mod.rs:352-527` looked plausible while
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


def collect_citations() -> list[tuple[str, str, str, str, str]]:
    """-> (path, 'start-end', citing-doc, fingerprint-of-the-WHOLE-range, excerpt)

    The fingerprint covers the entire cited range and the endpoint is part of the key.
    Fingerprinting only the first line let everything in `path:49-228` change while green.
    """
    out = []
    docs = (sorted(ROOT.glob("docs/**/*.md")) + sorted(ROOT.glob("docs/**/*.toml")))
    for doc in docs:
        text = doc.read_text(encoding="utf-8")
        if doc.suffix == ".md":
            text = strip_fenced(text)
        rel = str(doc.relative_to(ROOT))
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
            out.append((path_str, span, rel, fingerprint(body), excerpt(body)))
    return sorted(set(out))


def collect_continuations() -> list[tuple[str, str]]:
    """Citation shorthands that cannot be pinned. -> (citing-doc, matched-text)

    UNCONDITIONAL. An earlier version required a recognizable filename within a
    90-character lookbehind, which is a heuristic: a longer continuation, or a different
    formatting, recreated an unpinnable citation with the gate green. There is now no
    window and no filename requirement — any `:NNN` backtick shorthand outside a fenced
    block fails, and the author writes the full path. The corpus was swept to zero first,
    so this costs nothing and closes the hole rather than narrowing it.
    """
    out = []
    for doc in sorted(ROOT.glob("docs/**/*.md")) + sorted(ROOT.glob("docs/**/*.toml")):
        text = doc.read_text(encoding="utf-8")
        if doc.suffix == ".md":
            text = strip_fenced(text)
        rel = str(doc.relative_to(ROOT))
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

# grep options that consume the FOLLOWING token. They are refused rather than handled: if
# the gate mis-parsed one it would mistake a pattern for a path (or the reverse) and the
# existence check below would silently stop checking. Nothing in the corpus needs them,
# and `-e PAT` has an inline spelling if it ever does.
GREP_OPTS_WITH_ARG = {"-e", "-f", "-m", "-A", "-B", "-C", "-d", "--include", "--exclude",
                      "--regexp", "--file", "--max-count"}


def path_operands(argv):
    """The operands a command will try to READ. -> (paths, error-or-None)

    This costs some command-specific knowledge and is worth every line of it. Measured on
    BSD grep 2.6.0:

        $ grep -rn 'x' src/no_such_directory/ --include='*.rs' ; echo $?
        1

    No output, no stderr, exit 1 — byte for byte what a TRUE absence proof looks like.
    (Without --include the same command exits 2 and says "No such file or directory", so
    the stderr check alone does not cover it.) Twenty-odd items in feature-index.toml have
    the shape `grep -rn <pattern> <dir>/ --include='*.rs' -> exit 1, 0 lines`, so renaming
    a directory would leave every one of them green while measuring nothing at all. That
    is the "make the gate look at less" failure, and it is the reason a path a `cmd:` item
    names must be proved to exist before its emptiness means anything.

    grep's first non-option operand is its PATTERN, not a path: `grep -v '^src/parser'`
    names no file. find's paths are the leading operands, before its first predicate.
    """
    head, rest = os.path.basename(argv[0]), argv[1:]
    operands, after_ddash = [], False
    i = 0
    while i < len(rest):
        tok = rest[i]
        if not after_ddash and tok == "--":
            after_ddash = True
        elif not after_ddash and tok.startswith("-") and tok != "-":
            if head == "find" and operands:
                break                       # find's predicates start here
            base = tok.split("=", 1)[0]
            if base in GREP_OPTS_WITH_ARG and "=" not in tok:
                return None, (f"uses the option {tok!r}, which takes a separate argument; "
                              f"the gate would not be able to tell that argument from a "
                              f"path, so it could not check the paths exist")
        else:
            operands.append(tok)
        i += 1
    if head == "grep":
        operands = operands[1:]             # drop the pattern
    for p in operands:
        if p.startswith("/") or p.startswith("~") or ".." in Path(p).parts:
            return None, (f"names {p!r}, which is outside the repository or escapes it; a "
                          f"`cmd:` item observes the checked-in tree and nothing else")
        if not (ROOT / p).exists():
            return None, (f"reads {p!r}, which does not exist. An absence measured over a "
                          f"path that is not there is not an absence: BSD grep with "
                          f"--include exits 1 and prints nothing for a missing directory, "
                          f"which is exactly what a true absence proof looks like")
    return operands, None


def split_pipeline(cmd: str):
    """Split a `cmd:` command into argv segments. -> (segments, error-or-None)

    There is NO SHELL anywhere in this path. A `cmd:` item is argv, not a script: parsed
    with shlex, run with Popen, piped by file descriptor. That is not only a safety
    property — it is what makes the allowlist below meaningful, because with `shell=True`
    a quoted string could still smuggle a substitution past any token inspection.
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

    segments, current = [], []
    for tok in tokens:
        if tok == "|":
            segments.append(current)
            current = []
        elif tok in CMD_OPERATORS:
            return None, (f"uses the shell operator {tok!r}; only a plain pipeline of "
                          f"allowed commands may be a `cmd:` item")
        else:
            current.append(tok)
    segments.append(current)

    for seg in segments:
        if not seg:
            return None, "has an empty pipeline segment"
        head = os.path.basename(seg[0])
        if head in CMD_REFERRED:
            return None, f"runs `{head}`, which is not an observation — {CMD_REFERRED[head]}"
        if head not in CMD_ALLOWED:
            return None, (f"runs `{head}`, which is not in the `cmd:` allowlist "
                          f"({', '.join(sorted(CMD_ALLOWED))}). A `cmd:` item is a "
                          f"hermetic observation of the checked-in tree; anything else "
                          f"belongs to the gate that owns it")
        for tok in seg:
            if tok.startswith(CMD_BUILD_ARTIFACT):
                return None, (f"reads the build artifact {tok!r}; a `cmd:` item must be "
                              f"reproducible from a clean checkout, so a generated file "
                              f"is evidence only through the gate that generates it")
        _, err = path_operands(seg)
        if err:
            return None, err
    return segments, None


def run_pipeline(segments, timeout: int = 120):
    """Run the pipeline. -> (rc, stdout, harness-error-or-None)

    `rc` is the LAST segment's status, which is what `$?` reports for a pipeline and so
    what an item's `exit <N>` means.

    STDERR FROM ANY SEGMENT IS A HARNESS ERROR, NOT AN EMPTY RESULT. grep answers three
    questions, not two — 0 matched, 1 did not match, 2 COULD NOT LOOK — and collapsing
    the third into "did not match" turns an unreadable path into a proof of absence. That
    is exactly how a probe for the LLVM backend's CLI flag was recorded as `exit 1, 0
    lines`: the pattern began with two dashes, grep read it as an option, exited 2, and
    printed its usage to stderr. Nothing was ever searched. (That probe is also why no
    comment here may spell the flag out: it greps scripts/, so a mention would BE a hit.)
    scripts/conformance.sh:140-152 draws the same distinction for the same reason.

    stderr goes to a temporary FILE, never a pipe, so a chatty segment cannot deadlock
    the gate against a full pipe buffer while nobody is reading it.
    """
    env = dict(os.environ, LC_ALL="C")     # so the answer does not depend on a locale
    procs, errfiles, prev = [], [], subprocess.DEVNULL
    try:
        for seg in segments:
            errf = tempfile.TemporaryFile()
            errfiles.append(errf)
            try:
                p = subprocess.Popen(seg, cwd=ROOT, env=env, stdin=prev,
                                     stdout=subprocess.PIPE, stderr=errf)
            except OSError as exc:
                return None, "", f"could not start `{seg[0]}`: {exc}"
            if prev is not subprocess.DEVNULL:
                prev.close()
            prev = p.stdout
            procs.append(p)
        try:
            out, _ = procs[-1].communicate(timeout=timeout)
        except subprocess.TimeoutExpired:
            for p in procs:
                p.kill()
            return None, "", f"did not finish within {timeout}s"
        for p in procs[:-1]:
            p.wait(timeout=timeout)
        rc = procs[-1].returncode
        for errf, seg in zip(errfiles, segments):
            errf.seek(0)
            err = errf.read().decode("utf-8", "replace").strip()
            if err:
                return None, "", (f"`{seg[0]}` wrote to stderr, so it did not answer the "
                                  f"question — a command that could not look is not a "
                                  f"proof of absence: {err.splitlines()[0][:160]}")
        return rc, out.decode("utf-8", "replace"), None
    finally:
        for errf in errfiles:
            errf.close()


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

    if cmd not in cache:
        segments, err = split_pipeline(cmd)
        if err:
            cache[cmd] = (None, "", f"`cmd:` {err}")
        else:
            cache[cmd] = run_pipeline(segments)
    rc, out, err = cache[cmd]
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


def load_manifest():
    """tests/conformance-manifest.txt as {path: (class, line-number)}, or None.

    None means the manifest could not be read, which is a gate failure and never a reason
    to accept a `conformance:` item unchecked — the runner that owns this file exits 2
    rather than report a green run without it (scripts/conformance.sh:112-116).
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


def check_index():
    """-> (problems, row-count, how, counts)

    `counts` is printed by main(). Every `cmd:` item is RUN — there is no skip path, and
    that is deliberate: a skipped item that reports nothing is the same unmeasured
    denominator one layer down, and the conformance runner this file borrows its
    discipline from treats a fixture it cannot read as a failure rather than a skip
    (scripts/conformance.sh:512-517). An item the gate cannot run hermetically is a lint
    error naming the gate that owns the question, not a quiet exemption.
    """
    counts = {"cmd": 0, "conformance": 0, "src": 0, "gate": 0}
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
                if not m:
                    problems.append(f"{name}: `gate:` needs a command and a result "
                                    f"-> {item[:70]!r}")
                elif m.group(1) and not re.search(rf"^{re.escape(m.group(1))}:",
                                                  makefile, re.M):
                    problems.append(f"{name}: no such make target -> {m.group(1)}")
        if impl not in ("implemented", "partial", "unimplemented"):
            problems.append(f"{name}: implementation={impl!r} not in vocabulary")
        if not spec:
            problems.append(f"{name}: no spec pointer")
    return problems, len(rows), how, counts


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
    global INDEX
    update = "--update" in sys.argv
    # `--index` / `--index-only` exist for scripts/test-doc-evidence.sh, which points the
    # evidence checks at a throwaway index containing a KNOWN-FALSE `cmd:` item and
    # requires this gate to go red on it. A gate is worth its exit code, and the only way
    # to know this one still has one is to break something on purpose.
    # (Same purpose as CONFORMANCE_MANIFEST in scripts/conformance.sh.)
    if "--index" in sys.argv:
        INDEX = Path(sys.argv[sys.argv.index("--index") + 1]).resolve()
    if "--index-only" in sys.argv:
        problems, nrows, how, counts = check_index()
        print(f"feature-index rows: {nrows} via {how}")
        print(f"cmd: evidence executed: {counts['cmd']}")
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
    violations = collect_normative_violations()

    if update:
        old = read_pins() if PINS.exists() else {}
        new = {(p, s, d): (f, x) for p, s, d, f, x in cites}
        changed = [(k, old[k], v) for k, v in sorted(new.items())
                   if k in old and old[k][0] != v[0]]
        added = [k for k in sorted(new) if k not in old]
        removed = [k for k in sorted(old) if k not in new]
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

    if not PINS.exists():
        fail.append(f"missing {PINS.relative_to(ROOT)} (run --update)")
    else:
        want = read_pins()
        have = {(p, s, d): (f, x) for p, s, d, f, x in cites}
        for key, (f, x) in sorted(have.items()):
            if f in ("MISSING-FILE", "OUT-OF-RANGE"):
                fail.append(f"citation {key[0]}:{key[1]} in {key[2]} -> {f}")
            elif key not in want:
                fail.append(f"citation {key[0]}:{key[1]} in {key[2]} is unpinned "
                            f"(run --update)")
            elif want[key][0] != f:
                fail.append(f"citation {key[0]}:{key[1]} cited by {key[2]} MOVED\n"
                            f"      pinned: {want[key][1]}\n"
                            f"      now:    {x}")
        for key in sorted(want):
            if key not in have:
                fail.append(f"pinned citation {key[0]}:{key[1]} in {key[2]} no longer cited "
                            f"(run --update)")

    if not ALLOW.exists():
        fail.append(f"missing {ALLOW.relative_to(ROOT)} (run --update)")
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

    problems, nrows, how, counts = check_index()
    fail.extend(problems)

    print("=" * 62)
    print(f"citations pinned:   {len(cites)} (whole cited range fingerprinted)")
    print(f"no-compile fences:  {sum(n for _, n in fences)} across {len(fences)} file(s)")
    print(f"unpinnable shorthands: {len(conts)}")
    print(f"normative-surface violations: {len(violations)}")
    print(f"feature-index rows: {nrows} via {how}"
          + (", all evidence tagged and resolved" if not problems
             else f", {len(problems)} evidence problem(s)"))
    # Printed, and printed as a total with no skip column, because the denominator IS the
    # finding: for as long as this said nothing, all 53 `cmd:` items were unexecuted text.
    print(f"evidence items: cmd={counts['cmd']} (all EXECUTED, none skipped) "
          f"src={counts['src']} conformance={counts['conformance']} gate={counts['gate']}")
    print("=" * 62)
    if fail:
        print("\nFAIL:")
        for f in fail:
            print(f"  {f}")
        return 1
    print("doc evidence: OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
