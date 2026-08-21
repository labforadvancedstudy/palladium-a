#!/usr/bin/env python3
"""Evidence gate for the documentation. See scripts/check-doc-evidence.sh for why.

Three checks: citation pins, the no-compile allowlist, and feature-index evidence tags.
"""
from __future__ import annotations

import hashlib
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PINS = ROOT / "docs" / "citation-pins.tsv"
ALLOW = ROOT / "docs" / "no-compile-allowlist.txt"
INDEX = ROOT / "docs" / "reference" / "features" / "feature-index.yaml"

# Paths that name real repository files. A citation into anything else is prose.
CITED_ROOTS = ("src/", "scripts/", "tests/", "examples/", "stdlib/", "benchmarks/",
               "runtime/", "bootstrap/", "docs/")
CITED_FILES = ("Cargo.toml", "Makefile")

CITATION = re.compile(
    r"\b((?:src|scripts|tests|examples|stdlib|benchmarks|runtime|bootstrap|docs)/[\w./-]+?"
    r"|Cargo\.toml|Makefile|grammar\.ebnf):(\d+)(?:-(\d+))?\b"
    r"(?!:\d)"  # `path:line:col` is compiler output, not a citation
)

# Evidence tags accepted by feature-index.yaml. Anything else is an assertion.
EVIDENCE_TAG = re.compile(r"^(src|cmd|conformance|gate):\s+\S")
# A `cmd:` item must show what the command produced, or it proves nothing.
CMD_RESULT = re.compile(r"->|exit \d|error:")


def fingerprint(text: str) -> str:
    return hashlib.sha256(" ".join(text.split()).encode()).hexdigest()[:12]


def resolve(path_str: str, doc: Path) -> Path | None:
    """grammar.ebnf is cited bare from several documents; everything else is repo-root."""
    if path_str == "grammar.ebnf":
        return ROOT / "docs" / "specification" / "grammar.ebnf"
    if path_str.startswith(CITED_ROOTS) or path_str in CITED_FILES:
        return ROOT / path_str
    return None


def strip_fenced(text: str) -> str:
    """Blank out fenced code blocks.

    A `path:line` inside a fence is sample output — an illustrative compiler diagnostic —
    not a claim about this repository. Treating it as a citation produced three false
    failures on documents nobody had touched.
    """
    out, fenced = [], False
    for line in text.split("\n"):
        if line.lstrip().startswith("```"):
            fenced = not fenced
            out.append("")
            continue
        out.append("" if fenced else line)
    return "\n".join(out)


def collect_citations() -> list[tuple[str, int, str, str]]:
    """-> (path, line, citing-doc, fingerprint-of-that-line)"""
    out = []
    docs = sorted(ROOT.glob("docs/**/*.md")) + sorted(ROOT.glob("docs/**/*.yaml"))
    for doc in docs:
        text = doc.read_text(encoding="utf-8")
        if doc.suffix == ".md":
            text = strip_fenced(text)
        for m in CITATION.finditer(text):
            path_str, start, end = m.group(1), int(m.group(2)), m.group(3)
            target = resolve(path_str, doc)
            if target is None:
                continue
            rel = str(doc.relative_to(ROOT))
            if not target.exists():
                out.append((path_str, start, rel, "MISSING-FILE"))
                continue
            lines = target.read_text(encoding="utf-8", errors="replace").split("\n")
            last = int(end) if end else start
            if start < 1 or last > len(lines):
                out.append((path_str, start, rel, "OUT-OF-RANGE"))
                continue
            out.append((path_str, start, rel, fingerprint(lines[start - 1])))
    return sorted(set(out))


def collect_fences() -> list[tuple[str, int]]:
    out = []
    for doc in sorted(ROOT.glob("docs/**/*.md")) + [ROOT / "README.md"]:
        if not doc.exists():
            continue
        n = sum(1 for l in doc.read_text(encoding="utf-8").split("\n")
                if l.startswith("```palladium") and "no-compile" in l)
        if n:
            out.append((str(doc.relative_to(ROOT)), n))
    return sorted(out)


def check_index() -> list[str]:
    problems = []
    try:
        import yaml
    except ImportError:
        return ["feature-index: PyYAML not installed; cannot validate evidence tags"]
    if not INDEX.exists():
        return [f"feature-index: {INDEX} missing"]
    data = yaml.safe_load(INDEX.read_text(encoding="utf-8"))["feature_index"]
    rows = []

    def walk(node, path):
        if isinstance(node, dict):
            if "implementation" in node:
                rows.append((path, node))
            else:
                for k, v in node.items():
                    walk(v, path + [k])

    walk(data, [])
    for path, row in rows:
        name = ".".join(path)
        ev = row.get("evidence")
        if not isinstance(ev, list) or not ev:
            problems.append(f"{name}: evidence must be a non-empty list")
            continue
        for item in ev:
            if not EVIDENCE_TAG.match(item):
                problems.append(f"{name}: evidence item is not tagged "
                                f"src:/cmd:/conformance:/gate: -> {item[:70]!r}")
            elif item.startswith("cmd:") and not CMD_RESULT.search(item):
                problems.append(f"{name}: cmd: evidence shows no result "
                                f"(an absence needs its exit status) -> {item[:70]!r}")
        if row["implementation"] not in ("implemented", "partial", "unimplemented"):
            problems.append(f"{name}: implementation={row['implementation']!r} not in vocabulary")
        if not row.get("spec"):
            problems.append(f"{name}: no spec pointer")
    return problems, len(rows)


def main() -> int:
    update = "--update" in sys.argv
    cites = collect_citations()
    fences = collect_fences()

    if update:
        PINS.write_text(
            "# GENERATED by scripts/check-doc-evidence.sh --update. Do not edit by hand.\n"
            "# path\tline\tciting-doc\tfingerprint-of-cited-line\n"
            + "".join(f"{p}\t{l}\t{d}\t{f}\n" for p, l, d, f in cites), encoding="utf-8")
        ALLOW.write_text(
            "# GENERATED by scripts/check-doc-evidence.sh --update. Do not edit by hand.\n"
            "# Every ```palladium no-compile fence, per file. check-docs.sh reports the total\n"
            "# but pins nothing, so without this the count can drift upward while green.\n"
            + "".join(f"{p}\t{n}\n" for p, n in fences), encoding="utf-8")
        print(f"updated: {len(cites)} citation pins, "
              f"{sum(n for _, n in fences)} fences across {len(fences)} files")
        return 0

    fail = []

    # 1. citation pins
    if not PINS.exists():
        fail.append(f"missing {PINS.relative_to(ROOT)} (run --update)")
    else:
        want = {}
        for line in PINS.read_text(encoding="utf-8").split("\n"):
            if not line or line.startswith("#"):
                continue
            p, l, d, f = line.split("\t")
            want[(p, int(l), d)] = f
        have = {(p, l, d): f for p, l, d, f in cites}
        for key, f in sorted(have.items()):
            if f in ("MISSING-FILE", "OUT-OF-RANGE"):
                fail.append(f"citation {key[0]}:{key[1]} in {key[2]} -> {f}")
            elif key not in want:
                fail.append(f"citation {key[0]}:{key[1]} in {key[2]} is unpinned (run --update)")
            elif want[key] != f:
                fail.append(f"citation {key[0]}:{key[1]} cited by {key[2]} MOVED "
                            f"(pinned {want[key]}, now {f}) — re-derive it, then --update")
        for key in sorted(want):
            if key not in have:
                fail.append(f"pinned citation {key[0]}:{key[1]} in {key[2]} no longer cited "
                            f"(run --update)")

    # 2. no-compile allowlist
    if not ALLOW.exists():
        fail.append(f"missing {ALLOW.relative_to(ROOT)} (run --update)")
    else:
        want = {}
        for line in ALLOW.read_text(encoding="utf-8").split("\n"):
            if not line or line.startswith("#"):
                continue
            p, n = line.split("\t")
            want[p] = int(n)
        have = dict(fences)
        for p in sorted(set(want) | set(have)):
            if want.get(p, 0) != have.get(p, 0):
                fail.append(f"no-compile fences in {p}: allowed {want.get(p, 0)}, "
                            f"found {have.get(p, 0)} — justify and --update")

    # 3. evidence tags
    problems, nrows = check_index()
    fail.extend(problems)

    total_fences = sum(n for _, n in fences)
    print("=" * 60)
    print(f"citations pinned:      {len(cites)}")
    print(f"no-compile fences:     {total_fences} across {len(fences)} file(s)")
    print(f"feature-index rows:    {nrows}, all evidence tagged" if not problems
          else f"feature-index rows:    {nrows}, {len(problems)} evidence problem(s)")
    print("=" * 60)
    if fail:
        print("\nFAIL:")
        for f in fail:
            print(f"  {f}")
        return 1
    print("doc evidence: OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
