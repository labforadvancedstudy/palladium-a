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
| C1 | `src/codegen/mod.rs:1776` | a comment about unrecognised constant values | yes |
| C2 | `src/codegen/mod.rs:1699` | `self.output.push_str("    return __pd_empty_owned();\n");` | yes |
| C3 | `src/codegen/mod.rs:2009` | `for (_, module_info) in &imported_modules {` — the loop header, not the visibility test inside it | yes, narrowly |
| — | `src/typeck/mod.rs:1587` | the private-import registration comment | no |
| — | `src/codegen/mod.rs:2176` | `!crate::ast::local_definition_shadows_import(program, &func.name)` | no |

All three were **pre-existing on `main`** before the recursive-data-types work,
and all three have since been relocated MECHANICALLY three times — by a `difflib`
line map that faithfully preserved what they pointed at, which was already the
wrong code. The arithmetic of each relocation is correct and each is still wrong,
which is the whole reason the "a mechanical remap is not a fix" amendment exists.

They belong to the `XFAIL` on `test_selective_import_does_not_import_the_rest`,
owned by **M4 (cross-file module imports)**. Re-derive them when that row is paid:
the walk they should name is the one M4 will rewrite, so re-deriving them now
would just have to be done again.

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
