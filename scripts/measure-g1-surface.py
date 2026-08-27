#!/usr/bin/env python3
"""How many citations would the UNADOPTED G1 discriminator refuse? Measured, not argued.

THE QUESTION, AND WHO ASKS IT
-----------------------------
`docs/contributing/citation-and-predicate-debt.md` carries row **G1**: `--update` cannot see
a citation MERGED onto a span the same document already cites, because at the pin level a
merge is a removal with NO addition -- indistinguishable from an ordinary deletion. A
discriminator for it EXISTS, in inputs the gate already reads (a merge leaves a DUPLICATE,
and `collect_citations` discards multiplicity only at its last line, `return sorted(set(out))`):

    removal with no addition
      AND the removed key's content is still uniquely locatable
      AND the document now textually cites SOME span of that file two or more times.

G1 is UNADOPTED, and the reason is a number: how much of the corpus would that predicate put
behind `--allow-repin` when someone deletes a citation for entirely ordinary reasons. This
script is where that number comes from. G1 is its consumer; the two prose comments in
`scripts/check_doc_evidence.py` and `scripts/test-doc-evidence.sh` (CASE 50's contract) quote
the same figures and name this file.

WHY IT IS A SCRIPT AND NOT A SENTENCE
-------------------------------------
The figures were derived three times and were WRONG TWICE, each time by counting a set that
was nearby rather than the one the predicate tests:

    42 / 420    the duplicated triples themselves -- but a removed pin is no longer cited, so
                it can never BE the duplicate that satisfies conjunct 3;
    124 / 420   every pin in a pair that holds a duplicate -- but a removal event takes ALL
                textual occurrences of that pin, so a pin that is its pair's ONLY duplicated
                triple destroys the very witness conjunct 3 needs; and conjunct 2 was never
                evaluated at all.

Both corrections are the same mistake one step apart, which is exactly the shape a sentence
in a document cannot protect anyone from and a re-runnable measurement can. So the answer is
computed HERE, per candidate, by simulating the after-state each removal actually produces.

WHAT IT READS AND WHAT IT WRITES
--------------------------------
Read-only. It imports `scripts/check_doc_evidence.py` and uses that module's own
`citing_sources`, `CITATION`, `resolve`, `fingerprint`, `relocation_hits` and `read_pins`, so
the corpus it measures is the corpus the gate measures -- a second implementation of "what is
a citation" could disagree with the first, which is the defect class this repository has been
bitten by more than once. It writes nothing, mutates nothing, and takes no arguments.

Exit 0 on a clean measurement. Exit 1 if the residue check fails -- if the three buckets do
not account for every candidate, the split printed above them means nothing.

    python3 scripts/measure-g1-surface.py
"""
from __future__ import annotations

import collections
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

import check_doc_evidence as G  # noqa: E402


def textual_occurrences() -> list[tuple[str, str, str]]:
    """Every `(cited file, span, citing document)` AS WRITTEN. -> list, WITH multiplicity.

    `G.collect_citations()` ends in `sorted(set(out))`, so the multiplicity this whole
    measurement turns on is discarded one line before it is returned. Rebuilt here from the
    same pieces rather than reimplemented: same `citing_sources`, same `CITATION`, same
    `resolve`. The fingerprint/excerpt columns are dropped because nothing below needs them
    -- the pinned fingerprint is read from the pin file, which is what the guard reads.
    """
    out = []
    for rel, text in G.citing_sources():
        for m in G.CITATION.finditer(text):
            path_str, start = m.group(1), int(m.group(2))
            end = int(m.group(3)) if m.group(3) else start
            if G.resolve(path_str) is None:
                continue
            out.append((path_str, f"{start}-{end}", rel))
    return out


def main() -> int:
    raw = textual_occurrences()
    count = collections.Counter(raw)
    distinct = sorted(count)
    dup_triples = {k for k, n in count.items() if n >= 2}
    dup_pairs = {(p, d) for (p, s, d) in dup_triples}
    candidates = [k for k in distinct if (k[0], k[2]) in dup_pairs]
    pins = G.read_pins()

    print("G1 DISCRIMINATOR -- FALSE-REFUSAL SURFACE, MEASURED")
    print("=" * 66)
    print(f"  textual citation occurrences          : {len(raw)}")
    print(f"  distinct pins (after set())           : {len(distinct)}")
    print(f"  collect_citations() agrees            : "
          f"{len(G.collect_citations()) == len(distinct)}")
    print(f"  duplicated (file, span, doc) triples  : {len(dup_triples)}")
    print(f"  (file, doc) pairs holding a duplicate : {len(dup_pairs)}")
    print(f"  pair-level candidates  [UPPER BOUND]  : {len(candidates)}")
    print()

    # PER CANDIDATE, ON THE AFTER-STATE. A removal event for pin P takes every textual
    # occurrence of P with it, so conjunct 3 is asked of the tree MINUS P -- which is why P
    # can never be its own witness, and why a pin that is its pair's only duplicated triple
    # cannot satisfy it either.
    fires, no_c3, no_c2, neither, unpinned = [], [], [], [], []
    for k in candidates:
        p, s, d = k
        c3 = any(s2 != s and count[(p, s2, d)] >= 2
                 for (p2, s2, d2) in dup_triples if p2 == p and d2 == d)
        row = pins.get(k)
        if row is None:
            unpinned.append(k)
            continue
        c2 = len(G.relocation_hits(p, s, row[0])) == 1
        (fires if (c2 and c3) else
         no_c2 if c3 else
         no_c3 if c2 else neither).append(k)

    n = len(distinct)
    print("  after-state simulation, one removal event per candidate")
    print(f"    FIRES  (conjuncts 2 and 3 both hold) : {len(fires)}")
    print(f"    drops, conjunct 3 fails only         : {len(no_c3)}   "
          f"(sole duplicated triple of its pair)")
    print(f"    drops, conjunct 2 fails only         : {len(no_c2)}   "
          f"(content not uniquely locatable)")
    print(f"    drops, both fail                     : {len(neither)}")
    print(f"    candidates with no pin row           : {len(unpinned)}")
    total = len(fires) + len(no_c3) + len(no_c2) + len(neither) + len(unpinned)
    ok = total == len(candidates)
    print(f"    residue check                        : "
          f"{len(fires)}+{len(no_c3)}+{len(no_c2)}+{len(neither)}+{len(unpinned)}"
          f" = {total} vs {len(candidates)} candidates -- {'OK' if ok else 'MISMATCH'}")
    print()
    print(f"  SURFACE = {len(fires)} of {n} pins = {100 * len(fires) / n:.1f}%")
    print("  Read as ELIGIBILITY, never as a rate: deleting any one of those "
          f"{len(fires)} would be")
    print("  refused. How often a citation is actually deleted is not measured here.")
    print()

    # THE OTHER NUMBER G1's PROSE CARRIES, derived here so the row has ONE derivation and
    # not two. Before the after-state discriminator was found, the only candidate predicate
    # was pin-level: "this pin sits in a (file, document) pair that has a second pin, and its
    # content is uniquely locatable". G1 contrasts the two, so both are measured together --
    # a contrast whose halves come from different runs is a contrast nobody can re-check.
    by_pair = collections.Counter((p, d) for (p, _s, d) in distinct)
    multi = {k for k, m in by_pair.items() if m >= 2}
    wide = [k for k in distinct
            if (k[0], k[2]) in multi and k in pins
            and len(G.relocation_hits(k[0], k[1], pins[k][0])) == 1]
    print(f"  for contrast, the PIN-LEVEL predicate G1 rejected: {len(wide)} of {n} pins "
          f"= {100 * len(wide) / n:.1f}%")
    print()

    docs = collections.Counter(d for (_p, _s, d) in fires)
    print(f"  the surface by citing document ({len(docs)} documents)")
    for d, c in sorted(docs.items(), key=lambda kv: (-kv[1], kv[0])):
        print(f"    {c:4d}  {d}")
    print(f"    {sum(docs.values()):4d}  TOTAL")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
