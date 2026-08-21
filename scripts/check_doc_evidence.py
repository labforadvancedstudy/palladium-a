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
INDEX = ROOT / "docs" / "reference" / "features" / "feature-index.yaml"

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
    docs = sorted(ROOT.glob("docs/**/*.md")) + sorted(ROOT.glob("docs/**/*.yaml"))
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


# --- feature-index parsing, without a hard PyYAML dependency -------------------------

def _parse_rows_fallback(text: str):
    """Extract rows with no third-party parser.

    PyYAML is not in the standard library and nothing in this repository provisions it, so
    coupling the gate to `make check-docs` must not introduce a host-dependency failure.
    When PyYAML IS present, its parse is cross-checked against this one, so the fallback
    cannot silently drift.
    """
    rows, stack, cur = [], [], None
    for raw in text.split("\n"):
        if not raw.strip() or raw.lstrip().startswith("#"):
            continue
        indent = len(raw) - len(raw.lstrip(" "))
        line = raw.strip()
        if line.startswith("- "):
            if cur is not None and cur.get("_in") == "evidence":
                v = line[2:].strip()
                if len(v) >= 2 and v[0] == v[-1] == '"':
                    v = v[1:-1].replace('\\"', '"').replace("\\\\", "\\")
                cur["evidence"].append(v)
            continue
        if ":" not in line:
            continue
        key, _, rest = line.partition(":")
        key, rest = key.strip(), rest.strip()
        while stack and stack[-1][0] >= indent:
            stack.pop()
        if rest in ("", ">-", "|", "|-", ">"):
            if cur is not None and key == "evidence":
                cur["_in"] = "evidence"
                continue
            if cur is not None and key in ("note", "description"):
                cur["_in"] = None
                continue
            stack.append((indent, key))
            cur = {"path": [k for _, k in stack], "evidence": [], "_in": None}
            rows.append(cur)
        else:
            if cur is not None:
                if key in ("implementation", "spec"):
                    cur[key] = rest.strip('"')
                cur["_in"] = None
    return [r for r in rows if "implementation" in r]


def load_rows(text: str | None = None, strict_only: bool = False):
    """Parse feature-index rows, and CROSS-CHECK the two parsers semantically.

    Comparing row counts only was not a cross-check: a duplicate `evidence` key makes the
    fallback keep both lists while PyYAML keeps the last, and `spec: null` is the truthy
    string "null" to the fallback but None to PyYAML. Either divergence passes a count
    comparison, so the gate could pass without PyYAML and fail with it. The comparison is
    now over the full (name, implementation, spec, evidence) tuple of every row.
    """
    if text is None:
        text = INDEX.read_text(encoding="utf-8")
    # Drop the `feature_index:` root so both parsers name rows identically. The semantic
    # cross-check caught this the moment it was switched on from counting to comparing.
    loose = [(".".join(r["path"][1:] if r["path"][:1] == ["feature_index"] else r["path"]),
              r.get("implementation"), r.get("spec"), tuple(r["evidence"]))
             for r in _parse_rows_fallback(text)]
    try:
        import yaml
    except ImportError:
        if strict_only:
            raise
        return loose, "fallback parser (PyYAML absent)"
    doc = yaml.safe_load(text)
    strict = []

    def walk(node, path):
        if isinstance(node, dict):
            if "implementation" in node:
                ev = node.get("evidence") or []
                strict.append((".".join(path), node.get("implementation"),
                               node.get("spec"),
                               tuple(ev) if isinstance(ev, list) else (ev,)))
            else:
                for k, v in node.items():
                    walk(v, path + [k])

    walk(doc.get("feature_index", {}), [])
    if strict_only:
        return strict, "PyYAML"
    if strict != loose:
        diff = [f"    row {i}: fallback={l!r}\n              PyYAML  ={s!r}"
                for i, (l, s) in enumerate(zip(loose, strict)) if l != s][:3]
        extra = ""
        if len(strict) != len(loose):
            extra = f"\n    row counts differ: fallback={len(loose)} PyYAML={len(strict)}"
        raise SystemExit(
            "the two feature-index parsers disagree, so the gate's verdict would depend on "
            "whether PyYAML happens to be installed. Fix by writing the file in the supported "
            "style (block mappings, double-quoted scalars, one `evidence:` key per row) or by "
            "teaching _parse_rows_fallback the construct.\n"
            + "\n".join(diff) + extra)
    return strict, "PyYAML, cross-checked semantically against the built-in fallback parser"


SELFTESTS = [
    ("duplicate evidence key",
     'feature_index:\n a:\n  b:\n   implementation: partial\n   spec: "x"\n'
     '   evidence:\n    - "cmd: one -> ok"\n   evidence:\n    - "cmd: two -> ok"\n'),
    ("null spec",
     'feature_index:\n a:\n  b:\n   implementation: partial\n   spec: null\n'
     '   evidence:\n    - "cmd: one -> ok"\n'),
    ("single-quoted scalar",
     "feature_index:\n a:\n  b:\n   implementation: partial\n   spec: 'x'\n"
     '   evidence:\n    - \'cmd: one -> ok\'\n'),
    ("inline collection",
     'feature_index:\n a:\n  b:\n   implementation: partial\n   spec: "x"\n'
     '   evidence: ["cmd: one -> ok"]\n'),
    ("folded scalar in evidence",
     'feature_index:\n a:\n  b:\n   implementation: partial\n   spec: "x"\n'
     '   evidence:\n    - >-\n       cmd: one\n       -> ok\n'),
]


def selftest(quiet: bool = False) -> int:
    """Differential tests: the fallback must agree with PyYAML, or say it cannot parse.

    Silent disagreement is the failure mode that matters. A fallback that refuses a
    construct is safe; one that returns a different answer is not.
    """
    try:
        import yaml  # noqa: F401
    except ImportError:
        print("selftest: needs PyYAML to differentially compare; skipped")
        return 0
    bad = 0
    for name, doc in SELFTESTS:
        try:
            loose, _ = load_rows(doc, strict_only=False)
        except SystemExit as e:
            if not quiet:
                print(f"  {name:28s} DIVERGENCE CAUGHT (gate fails loudly, as intended)")
                print("      " + str(e).split(chr(10))[0])
            continue
        strict, _ = load_rows(doc, strict_only=True)
        if loose == strict:
            if not quiet:
                print(f"  {name:28s} agree -> {strict}")
        else:
            print(f"  {name:28s} SILENT DISAGREEMENT  fallback={loose} pyyaml={strict}")
            bad += 1
    if not quiet:
        print("selftest:", "OK" if not bad else f"{bad} silent divergence(s)")
    return 1 if bad else 0


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
                        n = len(tgt.read_text(encoding="utf-8",
                                              errors="replace").split("\n"))
                        if start < 1 or end > n:
                            problems.append(
                                f"{name}: `src:` line out of range -> {item[:60]!r}")
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
    if "--selftest" in sys.argv:
        return selftest()
    update = "--update" in sys.argv
    cites = collect_citations()
    fences = collect_fences()

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

    problems, nrows, how = check_index()
    fail.extend(problems)

    if selftest(quiet=True):
        fail.append("parser differential selftest found a SILENT disagreement between the "
                    "fallback and PyYAML (scripts/check-doc-evidence.sh --selftest)")

    print("=" * 62)
    print(f"citations pinned:   {len(cites)} (whole cited range fingerprinted)")
    print(f"no-compile fences:  {sum(n for _, n in fences)} across {len(fences)} file(s)")
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
