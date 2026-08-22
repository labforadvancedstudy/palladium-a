# Root-level `.md` is in the doc-evidence corpus (governance widening)

**Updated**: 2026-08-22 · Measured, not argued. Every number here is reproducible with the
command that produced it.

## What changed, and what it is

`scripts/check_doc_evidence.py` now scans root-level `*.md` for `path:line` citations. That is
a **governance widening**, not a bug fix: it changes which files the gate holds to account,
rather than repairing something that was broken in the gate's own logic.

Measured cost of the whole change: **+1 pin, 0 failures.**

| file | citations | failing | unpinnable shorthands |
|---|---|---|---|
| `CLAUDE.md` | 1 | 0 | 1 → fixed first |
| `README.md` | 0 | 0 | 0 |
| `CONTRIBUTING.md` | 0 | 0 | 0 |
| `FEATURES.md` | 0 | 0 | 0 |
| `README-crate.md` | 0 | 0 | 0 |

The four zero-citation files enter at zero cost today and are governed from now on, which is
the actual point: the next `path:line` written into `README.md` is checked, and before this it
would not have been.

## Why it was out, and why that is the interesting part

`citing_sources` (`scripts/check_doc_evidence.py:249-251`) walks a set of globs. Until this
change every one of them was under `docs/`, `src/` or `tests/`, and `CLAUDE.md` is at the
repository root — so it matched none. The file was not exempted, not special-cased, and not
skipped by any rule. It was simply never looked at.

Note what this is NOT: `CITED_ROOTS` has always allowed `src/`, `tests/`, `scripts/` … as legal
TARGETS of a citation. `CLAUDE.md` cites `src/`, which is and always was a legal target. It is
the *citing* side that was unscanned.

That asymmetry is the defect stated generally: **a gate that checks where claims POINT but not
where they are MADE is half a gate.** The missing half is the one that decides which files may
carry evidence at all — and `CLAUDE.md`, the file every agent working on this repository is
told to read first, sat entirely inside it.

It was not a hypothetical gap. `CLAUDE.md`'s single citation was **wrong**: it named
`check_stmt` while claiming to name the call path that mints a per-call lifetime and ends its
borrows. Nobody can say when it broke, because nothing had ever looked. It now reads
`src/ownership/borrow_checker.rs:891-898` and is pinned.

## The one thing that had to be fixed first

`CLAUDE.md` carried a bare `:NNN` shorthand — a colon and a line number, with no path, naming a
revision this tree no longer has. `collect_continuations` refuses those unconditionally, and
correctly: such a shorthand gets no pin and no movement check, so it cannot be told from a
citation that has silently drifted. It was rewritten to name no number at all, which is the
rule [`language-spec.md` A9.4](../specification/language-spec.md#a94-defect-d6-retracted)
already applies to exactly this situation.

*(The literal form is not reproduced in this file. Writing it out made THIS document fail the
gate — `unpinnable citation shorthand in docs/contributing/claude-md-coverage.md` — which is
the check working: it does not care that the occurrence was an example.)*

## What this does NOT buy

Stated rather than implied, because the value of a gate is bounded by what it actually reads:

- **Uncited prose is unchecked.** That is most of `CLAUDE.md`. A false sentence carrying no
  `path:line` is invisible to this.
- **`cmd:` / `gate:` / `conformance:` evidence is not read here.** Those live only in
  `feature-index.toml` rows and are executed by a different part of the gate.
- **A pin proves a range has not MOVED, not that it SUPPORTS its claim.** Reading it is still a
  reviewer's job; the excerpt column in `docs/citation-pins.tsv` exists for that.

So this closes *"the citation silently repointed at something unrelated"* — which is precisely
what had happened — and nothing wider.

## Reverting

Delete `sorted(ROOT.glob("*.md")) +` from `citing_sources` and drop the `CLAUDE.md` row from
`docs/citation-pins.tsv`. No other file depends on the wider set.
