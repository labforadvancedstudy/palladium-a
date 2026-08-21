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
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PINS = ROOT / "docs" / "citation-pins.tsv"
ALLOW = ROOT / "docs" / "no-compile-allowlist.txt"
INDEX = ROOT / "docs" / "reference" / "features" / "feature-index.toml"

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
CMD_EVIDENCE = re.compile(r"^cmd:\s+(.+?)\s*->\s*\S")
CONF_EVIDENCE = re.compile(r"^conformance:\s+([\w./-]+\.pd)\s+"
                           r"(PASS|COMPILE_FAIL|LINK_FAIL|RUN_FAIL|SKIP_NO_MAIN)\b")
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
    """Citation shorthands that cannot be pinned. -> (citing-doc, matched-text)"""
    out = []
    for doc in sorted(ROOT.glob("docs/**/*.md")) + sorted(ROOT.glob("docs/**/*.toml")):
        text = doc.read_text(encoding="utf-8")
        if doc.suffix == ".md":
            text = strip_fenced(text)
        rel = str(doc.relative_to(ROOT))
        for m in CONTINUATION.finditer(text):
            ctx = text[max(0, m.start() - 90):m.end()]
            # Only a continuation *of a citation* matters: something earlier on the line
            # named a real file. Line/column numbers in prose about other things do not.
            if re.search(r"(src|scripts|tests|examples|stdlib|benchmarks|runtime|bootstrap"
                         r"|docs)/[\w./-]+\.(rs|pd|sh|py|ebnf|md|toml)|Cargo\.toml|Makefile",
                         ctx):
                out.append((rel, ctx[-60:].strip()))
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
    if not INDEX.exists():
        return [f"feature-index: {INDEX} missing"], 0, "none"
    rows, how = load_rows()
    problems = []
    makefile = (ROOT / "Makefile").read_text(encoding="utf-8")
    for name, impl, spec, ev in rows:
        if not isinstance(ev, (list, tuple)) or not ev:
            problems.append(f"{name}: evidence must be a non-empty list")
            continue
        for item in ev:
            if not TAGGED.match(item):
                problems.append(f"{name}: untagged evidence -> {item[:70]!r}")
            elif item.startswith("src:"):
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
            elif item.startswith("cmd:") and not CMD_EVIDENCE.match(item):
                problems.append(f"{name}: `cmd:` shows no result — an absence needs its "
                                f"exit status -> {item[:70]!r}")
            elif item.startswith("conformance:"):
                m = CONF_EVIDENCE.match(item)
                if not m:
                    problems.append(f"{name}: `conformance:` needs <file>.pd <VERDICT> "
                                    f"-> {item[:70]!r}")
                elif not (ROOT / m.group(1)).exists():
                    problems.append(f"{name}: conformance fixture missing -> {m.group(1)}")
            elif item.startswith("gate:"):
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
    return problems, len(rows), how


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
    update = "--update" in sys.argv
    cites = collect_citations()
    fences = collect_fences()
    conts = collect_continuations()

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

    for rel, ctx in conts:
        fail.append(f"unpinnable citation shorthand in {rel}: ...{ctx} — write the full path; "
                    f"a bare `:LINE` gets no pin and no movement check")

    problems, nrows, how = check_index()
    fail.extend(problems)

    print("=" * 62)
    print(f"citations pinned:   {len(cites)} (whole cited range fingerprinted)")
    print(f"no-compile fences:  {sum(n for _, n in fences)} across {len(fences)} file(s)")
    print(f"unpinnable shorthands: {len(conts)}")
    print(f"feature-index rows: {nrows} via {how}"
          + (", all evidence tagged and resolved" if not problems
             else f", {len(problems)} evidence problem(s)"))
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
