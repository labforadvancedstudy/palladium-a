# Known-wrong citations and un-consolidated predicates

A **debt register**, not a task list. Every row is something a reviewer or a
sweep has already established is wrong, that nobody has fixed, and that no gate
reports. It exists because `CONTRIBUTING.md` tells you to leave pre-existing
wrong citations alone and *name them as debt* — and a name in a pull-request
report is read once, while this file is in the tree.

**Adding a row is not an admission of laziness; omitting one is the defect.**
The 98 wrong citations a 2026-08 sweep found did not get there by anyone
deciding to write a wrong citation. They got there because each was left silently
by someone whose change did not cause it.

## Rule these rows exist under

From `CONTRIBUTING.md` §Documentation:

> Fix a citation when your change broke it, when a reviewer names it, or when you
> are already editing the sentence that carries it. Leave every other
> pre-existing wrong citation alone, and RECORD IT AS DEBT here.

And the amendment that produced this file:

> **A mechanical remap is not a fix.** Relocating a citation with a line map
> preserves *what it pointed at*, faithfully — including when what it pointed at
> was already the wrong code. A remapped citation is DEBT until somebody
> re-derives it BY CONTENT. It looks identical to a re-verified one in a diff,
> which is precisely why it has to be written down instead of assumed.

That last point has a measured instance, and it is the first row below. One
sentence lists five citations; two are correct and three point at unrelated code,
and all five were renumbered together by the same mechanical pass. In the diff
they are one uniform, freshly-maintained list. Nothing but this file distinguishes
the two that were read from the three that were only moved.

## Open: citations known to point at the wrong code

One sentence in `tests/m3_imported_calls.rs:1219-1223` lists five "consumers that
re-derive visibility from `ast.items`". Three of the five name code that does no
such thing. Verified 2026-08-23 by reading each line:

| # | Cites | What is actually there | Wrong? |
|---|---|---|---|
| C1 | `src/codegen/mod.rs:1910` | a comment about unrecognised constant values | yes |
| C2 | `src/codegen/mod.rs:1833` | `self.output.push_str("    return __pd_empty_owned();\n");` | yes |
| C3 | `src/codegen/mod.rs:2143` | `for (_, module_info) in &imported_modules {` — the loop header, not the visibility test inside it | yes, narrowly |
| — | `src/typeck/mod.rs:1593` | the private-import registration comment | no |
| — | `src/codegen/mod.rs:2310` | `!crate::ast::local_definition_shadows_import(program, &func.name)` | no |

All three were **pre-existing on `main`** before the recursive-data-types work,
and all three have since been relocated MECHANICALLY three times — by a `difflib`
line map that faithfully preserved what they pointed at, which was already the
wrong code. The arithmetic of each relocation is correct and each is still wrong,
which is the whole reason the "a mechanical remap is not a fix" amendment exists.

They belong to the `XFAIL` on `test_selective_import_does_not_import_the_rest`,
owned by **M4 (cross-file module imports)**. Re-derive them when that row is paid:
the walk they should name is the one M4 will rewrite, so re-deriving them now
would just have to be done again.

## Open: two more of the same class, found while repairing a re-snapshot

Found 2026-08-26 while repairing eleven pins that `f8b5ff1` had re-snapshotted onto
unrelated lines. Both rows below were ALREADY WRONG at `f8b5ff1^`, so neither was
caused by that commit or by its repair; both were relocated BY CONTENT, which is
what the amendment above says preserves a wrongness exactly.

| # | Cites | What is actually there | Wrong? |
|---|---|---|---|
| C4 | `src/typeck/mod.rs:1693-1694` | `fields .iter()`, the walk over an enum variant's named fields — not an insert of any kind | yes |
| C5 | the `src/typeck/mod.rs:1593` row of the C1–C3 table above | `self.functions.insert(func.name.clone(), func_type.clone());` — an insert, not "the private-import registration comment" as that row's third column says | the ROW's description is; the citation is not |

C4 is cited from the doc comment in `src/typeck/mod.rs` beginning "Set imported
modules for type checking", whose sentence reads "Every insert below is under the
BARE name as well as the qualified one" and then names three ranges. The first two
are inserts. The third is a field iteration, and was one before `f8b5ff1` moved it
too — the relocation is arithmetic and the citation is still wrong, which is this
file's whole thesis stated a fourth time.

C5 is a defect in THIS FILE rather than in the compiler: the citation is right and
the sentence describing it is not. It is recorded rather than quietly corrected
because the third column is what a reader checks the citation AGAINST, so a silent
edit would erase the evidence that the two ever disagreed — and a table that
adjudicates citations is exactly the wrong place to demonstrate that a description
can be rewritten without anybody knowing.

Neither is re-derivable without deciding what the sentence SHOULD name, which is a
reading and not an arithmetic. C4 belongs with the C1–C3 rows to **M4 (cross-file
module imports)**: the insert sites its sentence is about are the ones M4 rewrites.

## Open: the re-pin guard cannot see a citation MERGED onto an existing one

Not a wrong citation — a **declared hole in a gate**, recorded here because the
alternative is that it lives in one reviewer's memory. Raised by round 4 of external
review of the `--allow-repin` guard, 2026-08-26; its *justification* was refuted and
rewritten by round 5, 2026-08-27, which is the more important half of this row.

| # | Shape | Verdict today |
|---|---|---|
| G1 | A document that cites a file at two places rewrites the first citation to name the second's lines, merging two claims onto one range. At the pin level that is a removal with NO addition — the destination key already existed — so `--update` reads it as an outright deletion and records it silently, although the dropped pin's content is still uniquely in the file and nothing now names it. | recorded, not refused — **UNADOPTED, false-refusal surface 90 of 420 pins = 21.4%, measured 2026-08-27 by `scripts/measure-g1-surface.py`** (124 is the pair-level upper bound: −22 sole-duplicate, −12 non-unique) |

**Why it is open, and why that is a decision rather than an omission.** A merge and an
ordinary deletion produce a byte-identical pin diff: one key removed, none added, the
content still findable. Refusing the merge therefore refuses every ordinary deletion of a
citation whose target still exists, and documents do that legitimately. Measured on this
corpus, **291 of 420 pins** sit in a (file, document) pair that has a second pin *and* have
uniquely locatable content, which means **deleting any one of those 291 would be refused**
(`scripts/measure-g1-surface.py` prints this contrast beside the surface, so the two halves
come from one run).
That figure is deletion ELIGIBILITY — how much of the corpus the predicate puts behind the
flag — and not a rate at which anybody deletes anything; the earlier wording ("two thirds of
every citation deletion") read as a frequency and was wrong to.

**A DISCRIMINATOR DOES EXIST, and the previous version of this row denied it.** It said the
only discriminator was the document's TEXTUAL citation count for that file, that the *before*
number is absent from every input `--update` reads, and therefore that closing G1 "starts
with a schema change to the pin file". The middle clause is true and the conclusion is not.
`collect_citations` in `scripts/check_doc_evidence.py` reads the raw documents and discards
multiplicity only at its last line, `return sorted(set(out))` — so an AFTER-STATE proxy is
available with no new input and no git: **a merge leaves a duplicate**, and the duplicate is
visible in the tree as it stands. The candidate predicate is

> removal with no addition **∧** the removed key's content still uniquely locatable **∧**
> the document now textually cites some span of that file two or more times.

Recorded prominently because an impossibility claim that turns out to be false is worse than
the hole it was excusing: it retires the question instead of parking it.

**Measured, 2026-08-27, and why it is still UNADOPTED.** Every figure in this row is
regenerated by `scripts/measure-g1-surface.py`, which reads the corpus through
`check_doc_evidence.py`'s own collectors and prints all of them, this histogram included;
run it rather than trusting the numbers here. 465 textual citations collapse to
420 distinct pins; 42 `(file, span, document)` triples are already cited twice or more; 124
pins sit in a `(file, document)` pair that holds a duplicate.

**124 is the UPPER BOUND, not the surface, and this row shipped it as the surface for one
round.** That is the same overcount it had just corrected in someone else's number, one step
further along. A removal event takes **all** textual occurrences of a pin, so the after-state
has to be simulated per candidate rather than read off the current tree. Doing that, two
groups drop out:

| | dropped | why |
|---|---|---|
| conjunct 3 fails | **22** | the pin is the SOLE duplicated triple of its pair, so removing it destroys the only witness the conjunct could have had — a pin cannot be its own duplicate, and here it cannot be its pair's either |
| conjunct 2 fails | **12** | the pinned content is not uniquely locatable, so `relocation_hits` does not answer 1 |
| fail both | 0 | — |

22 + 12 + 90 = 124, so the **false-refusal surface is 90 of 420 = 21.4%**, spread over 7
documents (the "10 documents" in the previous version described the 124-set).
`tests/m3_imported_calls.rs` holds 26 of the 90 and `docs/specification/language-spec.md` 34
— the full histogram is printed by `scripts/measure-g1-surface.py`, which is where these two
come from. The narrower 42/420 figure a reviewer first proposed counts the duplicated triples themselves,
which is not what the conjunct tests either.

**Read every one of these as ELIGIBILITY, never as a frequency.** *Deleting any one of those
90 citations would be refused* — that is the whole claim. How often anybody actually deletes a
citation is not measured here, and no number in this row is about it; an earlier phrasing
("one deletion in three") read as a rate and was wrong to.

So the choice is a live trade with a number on it, not a closed door: adopt when the surface
is narrowed further (a tighter third conjunct, or the *before* count recorded in the pin
file), and re-measure before adopting — 21.4% is measured on today's corpus and moves with it.

**What is NOT open.** The harm a merge does — two claims resting on one range — is owned by
`collect_enumeration_repeats`, whose 240-character window was itself set by measurement after
the broad rule flagged 41 groups that were every one legitimate. And the scope decision is
mechanically pinned: `scripts/test-doc-evidence.sh` carries a `guard`-role control asserting
that a dropped citation IS recorded, so widening the guard turns that control red and forces
this row to be answered rather than quietly outgrown.

## Open: a second, stale definition of the AST

| # | Artifact | Problem |
|---|---|---|
| D1 | `src/parser/mod.rs.backup` | **Tracked in git**, not ignored. 2,339 lines against the live parser's 4,554. Added by `14c0ef6` ("feat: Enhance parser with qualified imports and module aliasing") and present on `origin/main`. |

It contains an `EnumDef` literal with **no `visibility` field** — a stale copy of
exactly the definition the 2026-08-23 visibility work unified across parser,
resolver, typeck and codegen. It compiles nothing and gates nothing, so no check
notices it disagrees with the real AST.

Deleting it is a **separate call from a separate branch**: it is on `main`, it
predates this work, and a `.backup` somebody committed on purpose may be load
bearing to a person rather than to a build. Recorded here so the next reader of
`EnumDef` finds the second definition before it confuses them.

## Open: the shadowing predicate's un-consolidated sites

`crate::ast::local_type_shadows_import` is the one definition of "has a local
declaration taken this name". Three consumers call it: `enum_names_in_scope`,
code generation's imported emission walk, and `LayoutItems::of`.

| # | Site | Status |
|---|---|---|
| P1 | `TypeChecker::drop_imports_shadowed_by_local_types` | **Deliberate.** Asks a strictly narrower question the shared predicate cannot express — *is the local declaration that took the name an `enum`* — and widening the shared one would give three callers a distinction only this one uses. Documented at the method. Listed anyway: an unnamed fourth site is how a fifth gets written. |
| P2 | The constructor filter in the same method | **Fragile.** Keys on the strings `"<name>::"` and `"::<name>::"`, so a module whose name equals a shadowed type's name loses every qualified function it exports. No witness — module and type names have not collided in any tracked program. Closing it means keying the function table by a structured name instead of a string. |

## How this file goes stale, and what stops it

Nothing checks these rows. A row can be fixed and left here, or become wrong in a
new way, and no gate cares — this file has exactly the weakness it exists to
document, one level up.

Two things narrow it, and neither is a gate:

* every row names the **evidence** (a file, a commit, a measurement) rather than
  a verdict, so a reader can re-derive it in one command;
* every row names an **owner or a condition** for retirement, so "still open" is
  a claim somebody can falsify rather than a default.

If you fix a row, delete it in the same commit as the fix, and say in the commit
message that you did. If you find a new one, add it in the commit that found it —
not in the one that fixes it, which may never come.
