# `CLAUDE.md` is outside the doc-evidence corpus

**Updated**: 2026-08-22 · Measured, not argued. Every number here is reproducible with the
command that produced it.

## The answer, first

Nothing structural excludes it. `CLAUDE.md` is missed by a **glob**, and bringing it in costs
**one line of `scripts/check_doc_evidence.py` plus one already-applied fix to `CLAUDE.md`
itself**. The gate goes green on the result, measured.

It is written up rather than applied because it changes what the gate GOVERNS, and the same
one-line change also pulls in `README.md`, `CONTRIBUTING.md`, `FEATURES.md` and
`README-crate.md`. That is a policy decision with an owner, not a repair.

## Why it is out

`citing_sources` (`scripts/check_doc_evidence.py:234-235`) walks four globs:

```
docs/**/*.md   docs/**/*.toml   src/**/*.rs   tests/**/*.rs
```

`CLAUDE.md` is at the repository root, so it matches none of them. That is the whole reason.
The file is not exempted, not special-cased, and not skipped by any rule — it is simply not
looked at, and never has been.

Note what this is NOT: `CITED_ROOTS` has always allowed `src/`, `tests/`, `scripts/` … as legal
TARGETS of a citation. `CLAUDE.md` cites `src/`, which is a legal target. It is the *citing*
side that is unscanned.

## What it costs, measured

Every root-level `.md`, scanned with the gate's own `CITATION` and `CONTINUATION` regexes and
its own range resolution:

| file | citations | would fail | unpinnable shorthands |
|---|---|---|---|
| `CLAUDE.md` | 1 | 0 | **1** (now fixed) |
| `README.md` | 0 | 0 | 0 |
| `CONTRIBUTING.md` | 0 | 0 | 0 |
| `FEATURES.md` | 0 | 0 | 0 |
| `README-crate.md` | 0 | 0 | 0 |

The single blocker was a bare `:NNN` shorthand in `CLAUDE.md` — a colon and a line number, with
no path, naming a revision this tree no longer has. `collect_continuations` refuses those
unconditionally, and correctly: such a shorthand gets no pin and no movement check, so it
cannot be told from a citation that has silently drifted.

*(The literal form is not reproduced in this sentence. Writing it out made THIS file fail the
gate — `unpinnable citation shorthand in docs/contributing/claude-md-coverage.md` — which is
the check working: it does not care that the occurrence was an example.)* It has been rewritten to name no number, which is the rule
[`language-spec.md` A9.4](../specification/language-spec.md#a94-defect-d6-retracted) already
applies to exactly this situation.

With that fixed, the widened glob produces:

```
citations pinned:   327        (326 before; +1 is CLAUDE.md's)
unpinnable shorthands: 0
doc evidence: OK
```

and the pin `CLAUDE.md` would carry is the call path it claims:

```
src/ownership/borrow_checker.rs  891-898  CLAUDE.md  40f5ae3836be
  let call_lifetime = self.context.new_lifetime(); … self.context.end_borrows(&call_lifetime);
```

## The diff, if it is taken

```python
-    for doc in (sorted(ROOT.glob("docs/**/*.md")) + sorted(ROOT.glob("docs/**/*.toml"))
-                + sorted(ROOT.glob("src/**/*.rs")) + sorted(ROOT.glob("tests/**/*.rs"))):
+    for doc in (sorted(ROOT.glob("*.md")) + sorted(ROOT.glob("docs/**/*.md"))
+                + sorted(ROOT.glob("docs/**/*.toml"))
+                + sorted(ROOT.glob("src/**/*.rs")) + sorted(ROOT.glob("tests/**/*.rs"))):
```

Then `python3 scripts/check_doc_evidence.py --update` once, to write the new pin.

## Why it matters more than the numbers suggest

One citation is a small surface. The argument is not about the count.

`CLAUDE.md` is the file a reader — human or agent — is instructed to read FIRST, and it is the
one file whose claims about the compiler nothing has ever checked. Its single citation was
**wrong**: it pointed at `check_stmt` while claiming to point at the call path that mints and
ends a per-call lifetime. Nobody can say when it broke, because no gate has ever looked.

"0 pins" explains why the drift survived; it does not justify it. That sentence is the same one
this repository has been applying to smaller files all along — a check that nobody runs is not
a check — and the largest surface it applies to is the file that tells everyone else what is
true.

## The residual, stated rather than implied

Widening the glob pins `path:line` citations in root `.md`. It does **not** check:

- prose claims carrying no citation at all, which is most of `CLAUDE.md`;
- `cmd:`/`gate:`/`conformance:` evidence, which lives only in `feature-index.toml` rows;
- whether a pinned range SUPPORTS its claim. A pin proves the range has not moved. Reading it
  is still a reviewer's job — the excerpt column exists for that.

So this closes "the citation silently repointed", which is what actually happened here, and
nothing wider.
