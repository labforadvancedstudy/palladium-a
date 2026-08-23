# Proposed gate: a citation whose prose quotes a source token must name a line containing it

**Status: PROPOSED, not built.** Filed because the hole it closes is documented,
measured and recurring, and because the obvious version of the rule does not work
— which is the part worth writing down before someone builds it.

## The hole, in the checker's own words

The docstring of `scripts/check_doc_evidence.py` states it (in the section headed
"THE HOLE MOVEMENT DETECTION CANNOT SEE"):

> Everything above is about a pin whose CONTENT changed. It says nothing about a
> citation that changes its LINE NUMBERS, because the pin key contains them: edit
> `foo.rs:100` to `foo.rs:120` in a doc, run `--update`, and the old key is
> dropped while a new one is added. Neither is a MOVED.

So `make check-doc-evidence` reporting `411 pins, OK` certifies that the pin file
agrees with the docs. It never certifies that a citation points at the code its
prose is about. The same docstring records a sweep that found **25 such citations
across 220**, two of which had come to rest on an empty line and on a bare `}`.
The `}` case was mechanised — a range with no alphanumeric character is refused as
NON-SEMANTIC — and that is the narrowest wrongness a machine could name at the
time.

It keeps happening. On this branch alone:

* a relocation pass that picked its base revision by a fixed order was **wrong for
  124 of 238 citations** and had to be reverted whole;
* an independent reviewer sampled **ten** relocated citations and found **three**
  wrong;
* `src/codegen/mod.rs` cited a line of `src/typeck/mod.rs` and quoted the token
  `mutable: _` in the same parenthesis. The cited line held
  `if !inferred_types.is_empty() {`; the token was 213 lines away. (The offending
  line numbers are deliberately not written here — this file is scanned for
  citations like any other, and a citation quoted as an EXAMPLE of a wrong
  citation would be pinned as a real one.)

## The proposed rule

That last one is the shape a machine can decide, because the prose states the
expected text:

> When a citation's surrounding prose quotes a source token, the cited range must
> contain that token.

No reading is required. The citation says where, the prose says what, and the
file says whether they agree.

## Why the obvious implementation fails, measured

Implemented literally — take every backticked span within 160 characters of a
citation, require the cited range to contain at least one — the rule fires on
**240 of the 346** citations in this tree that have a quoted token nearby. It is
not a gate; it is a rewrite of the documentation.

The false positives are not noise to be tuned away, they are a category:

* prose legitimately discusses NEIGHBOURING code. A comment in
  `src/ownership/borrow_checker.rs` reads, in effect, "codegen's imported walk
  matches `Item::Struct` and `Item::Enum` (one range) and, separately,
  `Item::Function` (another)" — three quoted tokens, two citations, and each
  citation contains only one of them;
* prose quotes a USER-FACING STRING that lives in a different file entirely —
  `Undefined enum type: lib2` is quoted beside a citation into the parser;
* prose quotes a TYPE OR FUNCTION NAME as vocabulary, not as a location.

Narrowing to "the quoted token exists exactly once in the target file, and not in
the cited range" — the strongest decidable form, where the correct line is
computable — still fires **98** times, for the same reasons.

## What would have to be true for this to be a gate

The rule needs an explicit, opt-in binding between a citation and its expected
text, rather than a heuristic over adjacent prose. Something like a marker the
author writes deliberately:

```text
(`some/file.rs:NNN`, quotes `mutable: _`)
```

where `quotes` is the keyword the checker looks for. That makes the check exact
(zero false positives by construction), and it makes the coverage explicit and
countable — the same shape as the no-compile allowlist, which pins a number so it
cannot drift upward silently.

The cost is that it only protects citations somebody opted in. That is still
strictly more than zero, and it is honest about its own reach, which the
heuristic version is not.

## Blast radius

* **Adding the keyword to the checker**: small — one regex and one containment
  test in `scripts/check_doc_evidence.py`, beside the existing NON-SEMANTIC rule.
* **Adopting it across the tree**: 346 citations currently carry a quoted token
  nearby. Each one converted is a human reading, because converting it asserts
  that the citation is correct TODAY. That is the real cost and it should be paid
  incrementally — convert a citation when you touch the sentence, never in bulk.
* **The gate's own failure mode**: an author who writes the marker against a
  citation that is already wrong pins the wrongness. Same class as
  `--update` laundering, and the same mitigation — the marker is added by the
  person editing the claim, not by a sweep.

## What this does NOT fix

A citation with no quoted token, pointing at unrelated code that happens to be
semantic. That remains unmachineable, and the honest statement of the residual is
the one already in the checker's docstring: **a pin proves a range has not moved;
it cannot prove the range supports the claim, and no machine here does.**
