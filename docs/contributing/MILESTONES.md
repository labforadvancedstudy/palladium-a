# Milestones

**Updated**: 2026-08-22 · **Released**: v0.3.0 (M1) · **Target**: 1.0.0 = `make thesis-exit`

Ordered by what unblocks what, not by theme. Every milestone exits on one command that covers its
whole goal, and every milestone ships.

## What 1.0 is

**1.0 is `make thesis-exit`: `bootstrap/pdc.pd` rewritten in the differentiated dialect — `ref` /
`ref mut` with inferred regions, a `#[total]` the compiler discharges, inferred effects, and no
`async` or `await` — still reaching a byte-identical stage1/stage2 fixed point, with a second
witness program meeting the same conditions.**

That gate is in the repository now, and it is **RED: 1 green, 21 RED** over the 22 rows it
evaluates, plus `D1-01`, which cites the gate as its own evidence and is therefore the
aggregate rather than a member — it is answered by the summary ([`scripts/thesis-exit.sh`](../../scripts/thesis-exit.sh) →
[`scripts/thesis_exit.py`](../../scripts/thesis_exit.py)). It is committed red on purpose: the
definition of 1.0 has to live here as a command, because prose drifts and commands do not.

It does not read the manifest's *text*. Conditions 2 and 3 are delegated to
`scripts/conformance.sh`, which compiles, links, runs, diffs stdout against a recorded
transcript, checks the declared failure stage, matches the declared diagnostic fingerprint, and
reports `REJECT_ACCEPTED` when a negative test is accepted. A fixture the gate names and the
corpus does not run is reported **DECLARED, ABSENT** — loudly, not as a pass
([F13](#f13-the-first-thesis-gate-was-blind-in-the-way-m1-spent-itself-curing)).

### Why not an inventory

Two earlier definitions were considered and both are rejected: *"Part I has no unmet rows"* and
*"the feature list has no unmet rows"*. They differ in scope and not in kind — both are
**completeness criteria**, and a completeness criterion is the generator of every fiction this
repository has had to retract: `progress: 85%`, "Generics 85% complete" for a feature that emits no
code, "Bootstrap 100% Complete", "Self-Hosting 100%", "v0.6: Self-hosting achieved". Draw the line
on an inventory and the same disease returns under a new name.

The previous draft of this file made that mistake concretely. Its criterion was *"no row in
`feature-index.toml` whose `spec` names a Part I anchor is other than `implemented`"* — and **it
went green with all three differentiators unimplemented**, because N7, N8 and N9 anchor no rows
there precisely *because* they are unimplemented. A health check that passes with the heart removed.

**One artifact in this repository structurally cannot lie.** A conformance fixture can print "not
yet implemented" and PASS — seven of them did, for a year, and defect D5 survived behind one. A
compiler cannot compile *itself* vacuously. `bootstrap/pdc.pd` reaching a byte-identical fixed point
is the only claim here that no amount of prose can fake, so it is what the definition rests on, and
**scope follows from what the compiler actually uses rather than from argument about what belongs in
a release**.

### The four conditions, and why condition 3 decides everything

| | Condition |
|---|---|
| 1 | `make selfhost` green, **and** `bootstrap/pdc.pd` is written in the dialect: no `async`/`await`, no lifetime parameter list, ≥1 `ref`/`ref mut` parameter, ≥1 discharged `#[total]`, and a file-IO function whose inferred effect reaches its caller |
| 2 | one **non-vacuous** conformance fixture with a transcript, per differentiator |
| 3 | per differentiator, a **reject twin** — a `#[total]` whose proof fails, a `ref` whose region is ambiguous, an ungated effect escape: each a compile error, and the region one must *name the ambiguity* |
| 4 | a **second witness program** — the JSON parser — meeting the same three conditions |

**Condition 3 is load-bearing and must never be dropped. For an inference feature, the rejection is
the product.** A region inferencer that accepts everything is a no-op, and a no-op is
indistinguishable from a working one if you only look at green fixtures — which is exactly what
`tests/07_traits_basic.pd` did for a year. Condition 4 exists so that one program's accidental shape
does not become the definition of the language.

### What the requirement manifest is now for

[`1.0-requirements.tsv`](1.0-requirements.tsv) — **191 rows, 31 satisfied · 152 owed · 8 blocked**
— stays, and it is still closed, still reconciled against both debt inventories. Its role changed:
**it enumerates, it does not gate.** Every row carries a `disposition`:

| Disposition | Count | Meaning |
|---|---|---|
| `thesis` | 23 | `make thesis-exit` reads it directly. These rows *are* the definition, and the id set is pinned in the gate: adding, removing or retyping one is a harness error |
| `1.0` | 162 | the witnesses exercise it, or a `thesis` row rests on it |
| `post-1.0` | 6 | enumerated and **explicitly deferred**, owner `P1` |

Nothing is dropped silently. A requirement the thesis does not exercise is marked `post-1.0` in
writing, with an owner — because an omission nobody wrote down becomes the next generation's
fiction, which is the same failure as a percentage nobody measured.

The three differentiators, which the thesis proves:

| # | Differentiator | Normative | Definition | Rows |
|---|---|---|---|---|
| 1 | Asynchrony is an effect — no `async`, no await operator, no colouring | [N7](../specification/language-spec.md#n7-effects-and-asynchrony) | [`async-as-effect.md`](../reference/features/async-system/async-as-effect.md) | N7-01…N7-19 |
| 2 | Termination is provable — `#![total(strict)]`, `#[decreases(expr)]` | [N8](../specification/language-spec.md#n8-totality) | [`totality-checking.md`](../reference/features/advanced/totality-checking.md) | N8-01…N8-12 |
| 3 | Lifetimes are inferred — `ref` / `ref mut`, no `'a` | [N9](../specification/language-spec.md#n9-references-and-lifetimes) | [`implicit-lifetimes.md`](../reference/features/core-language/implicit-lifetimes.md) | N9-01…N9-09 |

## What actually blocks what

Four capabilities, and the order they impose. Two corrections to the previous draft of this section
are marked, because both changed the plan.

| | Capability | What it is | Required by | Waits on |
|---|---|---|---|---|
| **C0** | Abstraction | Traits, generics, bounds, `where` clauses. Trait/generic/module conformance is **zero** today | the effect system's *signatures* · a bootstrap compiler that can grow · the standard library | the surface |
| **C1** | Reference typing | `Type::Reference` is a distinct type carrying mutability. Today it is mapped to its inner type, so `&i64` and `i64` are the same type (`src/typeck/mod.rs:121-125`) | N9 in full · N12's move semantics and drop glue · moving the array rule out of codegen ([A9.2](../specification/language-spec.md#a92-array-parameters)) · C4 · **soundness** of C0's borrows | nothing |
| **C2** | Call-graph fixed point | Per-function summaries propagated to a fixed point, unknown callees not assumed pure, `impl` methods included. Today a single source-order pass whose fallback is "conservatively assume it's pure" (`src/effects/mod.rs:280-284`) | N7's inference and gating · N8's propagation of totality to callees, the same shape | C0, for signatures to carry effects |
| **C3** | Inductive pattern support | Patterns rich enough for structural recursion to have subterms. Enums, construction and `match` already work ([A4.3](../specification/language-spec.md#a43-enums)); literal, range, or-, tuple and guard forms are missing | N6 in full · N8's automatic structural termination | the parser |
| **C4** | Alias-sensitive scheduling | Deciding two effectful operations are independent, which is an aliasing question | N7's parallel-by-default and structured concurrency only | C1, and decision **D2** |

**Correction 1 — C1 is not a prerequisite for traits and generics, and the previous draft said it
was.** It listed "N10's `self` receivers" among C1's dependants. Measured: `impl` blocks with `self`
and `&self` receivers already compile, because `&T` is erased to `T`
([A4.5](../specification/language-spec.md#a45-impl-blocks)). So C0 does not wait for C1 to *exist*;
it waits for C1 to be *sound about borrowing*, which is a different and later claim. **The reorder
wins over the previous graph, and the previous graph was wrong on this edge.**

**Correction 2 — effects can ship before abstraction, and should not.** The previous draft said
basic effect gating needs neither C1 nor C0. That is true as a statement about *capability*: the
builtin registry already classifies every builtin (`src/builtins.rs:182`), the analyser already
unions effects, and effect polymorphism has no instance today because there are no function types
and no closures ([A5](../specification/language-spec.md#a5-types)). It is false as a statement about
*sequencing*, for two reasons the previous graph did not model:

- **The effect system is a typing judgment.** The moment `fn f<T: Display>(x: T)` exists, "what is
  the effect of `T::fmt`" must be answered. Ship effects against signature machinery that cannot
  carry bounds and you design the effect system twice.
- **The thesis gate makes it decisive.** 1.0 requires `bootstrap/pdc.pd` *rewritten* in the dialect.
  It is 991 lines today because it cannot abstract; you cannot grow a self-hosting compiler to cover
  the language without generics. Under an inventory definition, cheapest-first was defensible. Under
  a thesis definition, **the compiler has to be able to grow first**.

Both statements are true about different questions, and the sequencing one governs.

| Milestone | Version | What it is | What it waits on, and why it moved |
|---|---|---|---|
| M1 ✅ | v0.3.0 | The compiler stops lying | — |
| M2 | v0.4.0 | The surface, and M1's unpaid debt | M1. Unchanged: everything is written in this surface, and M1's debt is a live miscompile |
| M3 | v0.5.0 | **Traits and generics** | M2 → **C0**. *Moved from 6th to 2nd*: the effect system needs signatures that carry bounds, and the thesis needs a compiler that can grow |
| M4 | v0.6.0 | **Modules** | C0. *Split out of the old M6*: the bootstrap compiler and the library both become multi-file here |
| M5 | v0.7.0 | **Effects, static half** · differentiator 1 | C0, C2. *Moved later by two*: not because it cannot be built earlier, but because building it earlier means building it twice |
| M6 | v0.8.0 | **Totality** · differentiator 2 | C2, C3, C0. *Moved later, and un-split*: with generics already present, generic structural recursion (N8-07) lands here too instead of waiting |
| M7 | v0.9.0 | **Reference typing and region inference** · differentiator 3 | **C1**. *Moved later*: it is the deepest single capability, and nothing before it needs it |
| M8 | v0.10.0 | The standard library, and C FFI | M3, M4, M7 |
| M9 | **1.0.0** | **The thesis** — the bootstrap compiler in the dialect | everything. `make thesis-exit` green |
| P1 | post-1.0 | Parallel by default, structured concurrency | C4, **decision D2** |

**Versioning.** M1 shipped as `v0.3.0`; every milestone ships one release; every `0.x` is a
prerelease. **M9 ships as 1.0.0** rather than as a further `0.x`, because M9's exit criterion *is*
the definition of 1.0. v0.10.0 is therefore the last prerelease.

**Self-hosting is a floor throughout.** `make selfhost` stays green at every commit, and PBS-1 grows
with each milestone whose constructs the bootstrap compiler must consume. **`src/` is not retired.**
An earlier draft's M9 said "retire `src/` as the primary compiler"; that is demoted to **parity**.
The thesis is that the *bootstrap* compiler reaches a fixed point in the dialect, which does not
require the Rust compiler to go away — and retiring it early would mean implementing region
inference, an effect fixed point and a totality checker in a Palladium that does not yet have them.
Retirement is a post-1.0 decision.

## Where the project actually is

Measured at `7484bac`, not read from the previous version of this file.

| | | Command |
|---|---|---|
| **The thesis** | **1 green, 21 RED** over 22 evaluated rows + the aggregate | `make thesis-exit` |
| Self-hosting | fixed point over PBS-1 — stage1 and stage2 C byte-identical (`9b0cf24e…`) | `make selfhost` |
| Conformance | `verified=43 untranscribed=0 vacuous=7 xfail=1 reject=0 skip=2 failures=0` over 53 | `make conformance` |
| Conformance gate itself | 96 cases, each pinning a way it must still go RED | `make test-conformance-runner` |
| Thesis gate itself | 68 cases — 48 drive the gate end to end against an injected repository state, 20 exercise a helper | `make test-thesis-runner` |
| Documentation | every snippet compiles; 232 citations fingerprinted, 28 no-compile fences pinned | `make check-docs` |
| Rust tests | 620 pass, **0 fail**, 42 ignored (524 lib + 96 integration) | `make test-honest` |
| Declared failures | 41 `xfail` + 1 `slow`, none passing | `make test-xfail` |
| `stdlib/` | 0 of 21 files compile; 38 builtins accounted against a normative 34 | `make stdlib-gate` |
| Traits · generics · effects · async · unsafe · modules | conformance coverage is **zero** for each | `make conformance` |
| 1.0 requirements | 31 satisfied · 152 owed · 8 blocked, over 191 rows | [`1.0-requirements.tsv`](1.0-requirements.tsv) |
| `bootstrap/pdc.pd` | 991 lines, and it cannot abstract — which is why M3 moved to the front | `wc -l bootstrap/pdc.pd` |

## The inventories the manifest was derived from

**1. Part I, by section** —
`sed -n '/^| Normative section | Status/,/^$/p' docs/specification/language-spec.md | awk -F'|' 'NR>2{print $3}' | sort | uniq -c`

| Status | Count | Sections |
|---|---|---|
| implemented | 1 | N13 |
| partial | 9 | N1 N2 N3 N4 N5 N6 N11 N12 N14 |
| unimplemented | 4 | **N7 N8 N9**, N10 |

**2. Per feature** — `feature-index.toml`: 48 rows, **4 implemented · 16 partial · 28
unimplemented**. Ten now carry a `milestone` field recording 1.0 scope; the other 38 are
unclassified — see [Scope](#scope-what-is-in-10-and-what-is-not).

**3. Conformance debt** — `make conformance`: one `xfail`
(`tests/projects/hello_pdm/tests/test_math.pd`, cross-file imports, now **M4**) and seven `vacuous`
rows: `02_types_enums` (**M2**), `07_traits_basic`, `08_generics_basic` (**M3**),
`12_modules_imports` (**M4**), `09_effects_system`, `10_async_await`, `11_unsafe_blocks` (**M5**).
`reject` is empty ([F3](#f3-the-conformance-corpus-has-no-negative-tests)). Every one of these rows
is named by a requirement, and that direction of the reconciliation is checked.

**4. Declared Rust failures** — `make test-xfail`; owners parsed by `scripts/test-xfail.py:74`:
18 tagged M4 → now **M3**; 14 tagged M2 → **M2**; 5 tagged `unscheduled` → **M5**; 3 tagged M1 →
**M2**; 1 tagged M5 leaves the inventory
([F8](#f8-one-declared-failure-expects-syntax-the-specification-forbids)). That is
[26 re-tags and one reclassification](#f9-the-milestone-labels-in-the-test-suite-were-written-against-the-old-numbering).

**5. Open defects** — [`CLAUDE.md`](../../CLAUDE.md) "남은 결함" and the annex. Ownership is carried
by the requirement manifest; this is the reading list.

| Defect | Where | Requirement |
|---|---|---|
| D3b — a tail `if` is not lowered to a `return`; `fib(10)` prints `8261746944` and exits 0 | [A6.6](../specification/language-spec.md#a66-tail-expressions) | N3-02, N3-03 |
| **The async producer is live** — `async fn g() { … }` compiles and emits a `Future` struct with a `state` field and a `_poll` function, which N7 forbids outright | [F11](#f11-the-async-producer-is-alive-and-violates-n7-today) | N7-18 |
| C-keyword identifiers — `fn double` emits `long long double(…)` | `tests/e2e_test.rs:269` | N3-01 |
| No missing-return diagnostic — `fn f() -> int { }` compiles silently | `tests/compiler_comprehensive_test.rs:567` | N3-03 |
| Block comments do not nest, which N2 requires | [F10](#f10-block-comments-do-not-nest-and-nothing-said-so) | N2-08 |
| `a * -b` does not parse | [A6.3](../specification/language-spec.md#a63-expression-forms) | N5-16 |
| Nested arrays work in neither locals nor parameters | [A5](../specification/language-spec.md#a5-types) | N4-10 |
| Six builtins that cannot compile — the handle representation split in two | [A8](../specification/language-spec.md#a8-builtins) | N14-01, N14-03 |
| `pub` on an enum discarded; `dbg!` undefined; `println!` takes one argument; no hygiene | [A4.6](../specification/language-spec.md#a46-macros) | N3-05, N3-12, N3-13 |
| `Foo<T>` is parsed as a *const* generic argument; const generics are not monomorphised | [A5](../specification/language-spec.md#a5-types) | N10-03, N4-21 |
| Traits emit no C; a trait method with a `self` receiver is a parse error | [A4.4](../specification/language-spec.md#a44-traits) | N10-06, N10-09 |
| `&mut` of an immutable local is accepted for struct referents | [A9.3](../specification/language-spec.md#a93-mut-of-an-immutable-local-is-accepted) | N12-06 |
| `String` is a Copy handle, contradicting N12 — no drop glue | [A9.1](../specification/language-spec.md#a91-string-is-a-copyable-handle-decision-2026-08-21) | N12-03, N12-04 |
| Effects gate nothing; propagation assumes unknown callees pure; `impl` methods unanalysed | [A4.1](../specification/language-spec.md#a41-functions) | N7-03…N7-08 |
| Attributes do not lex — `#[total]` fails at the character `#` | [A2](../specification/language-spec.md#a2-lexical-structure) | N2-10, N2-11 |
| `src/async_runtime/mod.rs` — 498 lines, one referrer (`src/lib.rs:5`), no consumer | [F11](#f11-the-async-producer-is-alive-and-violates-n7-today) | N7-19, decision **D5** |

## How a milestone exits

**One command per milestone, covering the whole goal — accepted programs as well as refused ones,
runtime observables where the goal is a runtime property, packaging where the goal is a shipped
artifact.** Three lines of Makefile each:

```make
m5-exit: build
	@REQ_MILESTONE=M5 bash scripts/requirements.sh
```

`scripts/requirements.sh` does not exist yet. It is specified precisely enough to write.
(`make thesis-exit` is the same shape and already exists — note that it both reads the
manifest *and* carries a version-controlled copy of the thesis contract to compare against
it. That duplication is a reviewed cross-check, not a second definition: the pin catches an
edit to the manifest, and the pin's own validator catches a defect in the pin.)

1. Parse [`1.0-requirements.tsv`](1.0-requirements.tsv) — **nine** tab-separated columns, all
   mandatory. A row with a missing column, an unknown evidence kind, status or disposition is a
   failure of the manifest, not of the milestone. The ninth column is the diagnostic
   fingerprint a `reject` row's refusal must carry, and for a `thesis` reject row it may not
   be `-`: any rejection would satisfy that, including one for incidental unsupported syntax.
2. For the milestone named by `REQ_MILESTONE`, **every** row must be `satisfied`.
3. Resolve each evidence locator by kind, and *run* it: `fixture` → a `run` row whose transcript
   matches · `reject` → a `reject` row refused with its declared diagnostic · `skip` → a proven
   non-program · `observable` → a named Rust test that exists, is not `#[ignore]`d, and passes ·
   `gate` → a make target that exits 0 · `decision` → recorded as resolved in
   [Decisions](#decisions-for-the-owner).
4. Reconcile both debt inventories, in both directions. The conformance half is checkable today, by
   path. The Rust half needs a `req: <id>` tag in each `#[ignore]` reason.
5. `make test-requirements-runner` plants a row for the milestone under test and proves the runner
   goes RED for it. A filter nobody has watched fail is not a filter — which is why
   `make test-thesis-runner` already exists for the thesis gate and caught a real defect in it
   ([F12](#f12-the-thesis-gates-first-lexer-could-not-fail-on-what-it-checked)).

**Why an aggregate and not an owner filter.** `CONFORMANCE_FORBID_OWNER` clears only *tagged
proxies*: it proves no declared failure still names the milestone. It cannot prove the feature
works, because a feature nobody wrote a red test for produces no tagged proxy to clear. The filters
stay as fast pre-checks; the manifest decides; and **1.0 is decided by neither — it is decided by
`make thesis-exit`.**

---

## Completed

### M1 — The compiler stops lying (v0.3.0, released 2026-08-22)

Every other kind of work was slower while the compiler could accept a program and emit wrong code.
M1 converted silent wrongness into diagnostics, and — the part that outlives it — made the gates
able to fail.

Receipts:

| What | Evidence |
|---|---|
| **D5** `?` and `.await` emitted C referencing a `struct Result` layout and a `poll` member codegen never generates | Both refused at typecheck with the consequence and a workaround; old lowerings deleted. `tests/d5_unimplemented_constructs.rs`, 12 tests. **The `.await` consumer only — the `async fn` producer is still alive, see [F11](#f11-the-async-producer-is-alive-and-violates-n7-today)** |
| **D4** `for` over an array *parameter* used `sizeof` on a decayed pointer | The bound comes from the declared length; an unresolvable length is a compile error, not a wrong bound. `tests/regression/for_over_array_param.pd` |
| **D9** `&[T; N]` / `&mut [T; N]` parameters rejected in codegen | Lowered; a write that reaches the caller can only come from a spelling that declared it ([A9.2](../specification/language-spec.md#a92-array-parameters)). `examples/practical/simple_sort.pd` runs |
| **D7** an un-annotated `let` was emitted as `long long` regardless of its initializer | Fixed in `04104c5` |
| **D6** was not a defect | Retracted with five re-run probes ([A9.4](../specification/language-spec.md#a94-defect-d6-retracted)) |
| The LLVM backend fabricated rather than lowered at 14 sites, seven of them silently | `--llvm` refuses unconditionally. `tests/d10_llvm_refuses.rs`, 9 tests |
| `stdlib/` had no coverage at all | `make stdlib-gate`: 21 files pinned per file, 38 builtins accounted, generated C checked structurally. The premise was wrong and is recorded as such — **0 of 21 compile** ([`stdlib/STATUS.md`](../../stdlib/STATUS.md)) |
| A green exit code was counted as a correct program | Every `run` fixture is diffed against a recorded transcript; there is no exit-code-only class |
| Seven fixtures proved nothing while counting as coverage | Declared `vacuous`, each naming the feature it fails to cover. Seven of 53, on the summary line of every run |
| The gates could not fail | `make test-conformance-runner` (96 cases), `make test-gate-probe` (every evidence producer fault-injected) |
| `tests/*.rs` never ran under `make test-rust` | `make test-honest`, and every remaining failure converted to a declared `xfail` with an owner |

Not paid, and re-owned by M2: three M1 `#[ignore]` rows
([F2](#f2-m1-shipped-three-of-its-own-declared-failures-and-its-exit-command-could-not-see-them)).

---

## M2 — The surface, and M1's unpaid debt (v0.4.0)

**Waits on**: M1. **Delivers**: the surface everything else is written in, **C3**, the attribute
token N8 sits below, and the first witness program.

**Owns 45 requirement rows**, seventeen declared `#[ignore]` failures (fourteen tagged M2, three
tagged M1), and the vacuous `tests/02_types_enums.pd`.

1. **The M1 debt first, because it is a live miscompile.** A tail `if` is not lowered to a return —
   `fib(10)` prints `8261746944` (N3-02) — and the missing-return diagnostic lands with it (N3-03),
   as `tests/compiler_comprehensive_test.rs:567` already says it must. With them, C-keyword
   identifier mangling (N3-01).
2. **The async producer** (N7-18). `async fn g() { print("x"); }` compiles today and emits
   `typedef struct g_Future { int state; }` plus `int g_poll(g_Future *future)`. N7 says the
   language has **no runtime representation** of effects, so this is a live normative violation and
   it is cheap to close: refuse `async fn` at codegen exactly as `.await` already is. The keyword
   itself dies at M5.
3. **Statements and expressions** (N5-03…N5-17): `if`, `match` and blocks become expressions;
   `else if`; `loop` with a value-carrying `break`; compound assignment; bitwise operators; ranges;
   `as` casts; `a * -b`; method call syntax; top-level `const` and `static`.
4. **Patterns** (N6-02…N6-11) — literal, range, tuple, or- and `@` patterns, guards, exhaustiveness
   for **every** scrutinee type, and a trap where `match` currently falls through. This is **C3**.
5. **Lexical completion** (N2-03…N2-11): float and char literals, escapes, **nesting block
   comments** ([F10](#f10-block-comments-do-not-nest-and-nothing-said-so)), and **the `#` attribute
   token** — the token only. An attribute that lexes and is then ignored would recreate the class M1
   removed, so N2-11 makes an unknown attribute a compile error from the day `#` lexes.
6. **The six builtins that cannot compile** (N14-01, N14-04): the four `*_ex` names are not part of
   the language and leave `BUILTINS`; `file_flush` and `file_seek` are normative and get re-based.
7. **Witness 1** (WT-01): a JSON parser written with no workarounds, added to the corpus. It becomes
   the thesis gate's second witness at M9.
8. **Gate integrity** (GI-06, GI-08, GI-09). `make gates` (`Makefile:303-305`) does **not** run
   `test-honest` (`Makefile:277-282`), so a non-ignored compiler regression can coexist with a green
   gate — GI-06 adds it, a one-word change. The milestone-exit target and its self-test ship before
   anything depends on them.

**Exit**: `make m2-exit`.

## M3 — Traits and generics (v0.5.0)

**Waits on**: M2 → **C0**. *This is the biggest move in the plan.* An earlier draft had it sixth, on
the argument that effects and totality do not need it. They do not need it to *exist*; they need it
in order not to be **designed twice** — the effect system is a typing judgment, and the moment a
bound exists you must say what the effect of a bounded method is. And the thesis requires a
bootstrap compiler that can grow: 991 lines, no abstraction.

**Owns 24 requirement rows** and the 18 `#[ignore]` rows tagged M4 in the old numbering, plus the
vacuous `07_traits_basic` and `08_generics_basic`.

1. **Generics that work** (N10-01…N10-05, N4-15, N4-21). Inside `<…>` any all-uppercase name is
   reclassified as a *const* generic argument, so `Foo<T>` does not mean what it looks like; generic
   struct fields are rejected in codegen; const generics are not monomorphised.
2. **Traits with real dispatch** (N10-06…N10-10). They parse and emit nothing, method bodies are
   never typechecked, and a `self` receiver in a trait method is a parse error. Design:
   [`trait_system_design.md`](../design/trait_system_design.md), [`generics.md`](../design/generics.md).
3. **`Option<T>` and `Result<T, E>` as generic types with methods** (N4-16), and `?` lowering onto
   the representation enums actually get (N4-18, N4-19). Their **prelude shipping is N4-17 and
   belongs to M8** — representation is what M3 buys; being in scope with no import is a packaging
   property, and one milestone should not claim both.
4. **Closures, function types, slices** (N5-08, N4-14, N4-11, N6-06) — and with them the first real
   instance of effect polymorphism, which is why M5 comes after this and not before.
5. **Function types and signatures reserve a latent effect variable** (N10-11). *This is a
   condition on the reorder's own argument, not a nicety.* Moving abstraction ahead of effects is
   justified by avoiding a redesign; if M3 builds effect-blind function types, M5 redesigns them
   anyway and the justification evaporates. So M3 is not done until a function type carries an
   effect slot — unpopulated is fine, absent is not.
6. `Result`-returning builtin signatures (N14-03), now that `Result` exists.

**Exit**: `make m3-exit`, including N10-09 as an observable — a bounded call must emit no vtable,
because "abstraction costs nothing at runtime" is a claim about generated code that no stdout can
show.

## M4 — Modules (v0.6.0)

**Waits on**: C0. Split out of the old combined milestone because it has a distinct consumer: this
is where both the bootstrap compiler and the standard library become multi-file.

**Owns 8 requirement rows** — N3-11 and N11-01…N11-07 — plus the corpus's one `xfail`
(`tests/conformance-manifest.txt:91`, cross-file imports) and the vacuous `12_modules_imports`.

A `mod` item, file-based nesting, **enforced** visibility (N11-02 is a `reject` row: a private item
imported must be an error, or visibility is decoration), and all four import forms.

**Exit**: `make m4-exit`.

## M5 — Effects, static half (v0.7.0) · differentiator 1

**Waits on**: C0 and **C2**. Everything in N7 except parallel execution.

**Owns 15 requirement rows**, the five `#[ignore]` rows tagged `unscheduled`, and the vacuous
`09_effects_system`, `10_async_await` and `11_unsafe_blocks` — the last because
[N7](../specification/language-spec.md#n7-effects-and-asynchrony) puts unsafe, IO, memory and panic
on one footing.

1. **Give the analysis a consumer** (N7-03, N7-08). The driver runs the analyser
   (`src/driver/mod.rs:147`) and prints the result (`src/driver/mod.rs:151-157`); nothing downstream
   reads it, so it cannot reject a program, change codegen or schedule anything.
2. **Make propagation a fixed point** (N7-04, N7-05, N7-06). It is a single forward pass whose
   fallback is "If function is unknown, we conservatively assume it's pure"
   (`src/effects/mod.rs:280-284`) — the unsound direction.
3. **Analyse methods** (N7-07). The driver's loop matches only `crate::ast::Item::Function`
   (`src/driver/mod.rs:148-149`).
4. **Delete `async` and `await` from the language** (N7-01, N7-02) — the two things N7 says the
   language does not have are the two the implementation has. The producer died at M2; the keywords
   die here.
5. **Effect contexts** (N7-10…N7-12), and N14's classification enforced (N14-05), which is what M6's
   `#![total(strict)]` uses to forbid `unsafe`.

**Exit**: `make m5-exit` — positive fixtures, the reject twin (a pure function calling an I/O
builtin), and the observables stdout cannot show (a callee defined below its caller still
propagates; `impl` methods are analysed).

## M6 — Totality (v0.8.0) · differentiator 2

**Waits on**: C2, C3, C0. **Un-split by the reorder**: with generics already present, generic
structural recursion (N8-07) lands here with the rest of N8 instead of waiting a milestone.

**Owns 12 requirement rows.** It owns no `#[ignore]` row and no conformance row today — the feature
is absent rather than broken, and absence has no fixture — so its first task is to write N8's
evidence ([F4](#f4-two-differentiators-owned-no-failing-row-anywhere)).

1. `#[total]`, `#![total(strict)]`, `#[decreases(expr)]`, `#[total(fuel = N)]`, `#[partial]`.
2. Structural recursion on an inductive type needs no measure — monomorphic (N8-06) and generic
   (N8-07).
3. `unsafe` is not permitted in a `#![total(strict)]` crate (N8-11) — M5's classification working.
4. **There is no mode in which an unproven `#[total]` function is accepted** (N8-12), as an
   observable rather than a rejection, because "no flag downgrades this" is a claim about the whole
   surface.

**Exit**: `make m6-exit`. Note the shape: five rejection rows *and* six acceptance rows. **A checker
that refuses everything passes every rejection and fails N8-01…N8-06**, which is why both halves
are required — the same reason the thesis gate's condition 3 exists.

## M7 — Reference typing and region inference (v0.9.0) · differentiator 3

**Waits on**: nothing but itself — **C1**. It is the deepest single capability and nothing before it
needs it, which is why it sits here rather than at the front.

**Owns 19 requirement rows** and two of the owner's decisions (**D3**, **D4**).

1. **A real reference type** (N4-13). **Spelled `ref` / `ref mut` from the start**, per
   [N9](../specification/language-spec.md#n9-references-and-lifetimes); building it under `&` and
   renaming later is two surface changes for one feature.
2. **Region inference** (N9-05, N9-06) — `grep -rn 'region\|Region' src/ --include='*.rs'` returns
   nothing. The **elision-total fragment** first: the fragment in which inference always succeeds.
   Everything outside it is a compile error naming the ambiguity, never a guess.
3. **Remove `'a` parameter lists** (N9-04) — **but keep `ref<'a> T`**, which N9 explicitly permits
   and N9-03 requires to be **accepted**. The receipt is two parser-level tests, not a grep for `'`:
   a grep would reject conforming programs and, once char literals land at M2, would fire on
   `let c = '<';`. The thesis gate implements exactly this distinction, and its self-test pins it.
4. **N12 becomes true of the implementation** (N12-03…N12-06): drop glue, per-value deallocation,
   `String` with move semantics, `ref mut` of a non-`mut` binding refused for every referent type.
5. **Two owner decisions close here**: **D4** (array parameters) and **D3** (`str`/`usize`).

**Exit**: `make m7-exit`.

## M8 — The standard library, and C FFI (v0.10.0)

**Waits on**: M3, M4, M7. The last prerelease.

**Owns 15 requirement rows.** What the library needs, as features rather than as compile errors:
generic ADTs with bounds (M3), associated types for an iterator protocol (M3), drop glue and moves
so a `Vec<T>` can own its buffer (M7), modules (M4).

`make stdlib-gate`'s per-file blocker column is **a lower bound and not that dependency list** — the
manifest says so itself: the blocker is the *first* construct `pdc` rejects, and a lexer-level
blocker masks every parser-level blocker behind it (`stdlib/prelude.pd` is recorded as `ATTRIBUTE`
while also containing 18 `use` and 2 `mod` declarations). The counts support exactly one claim,
**every one of the 21 files is blocked on at least one earlier milestone**, and not the stronger
claim that this is the earliest correct start.

1. Core, collections, math, string, I/O (N14-09…N14-16), and the prelude (N4-17).
2. **Ship it** (N14-06…N14-08). `make stdlib-gate` is **green right now with 0 of 21 files
   compiling** — it pins a measurement, it does not require a working library — so the evidence is
   every file reaching `ACCEPTED_NO_MAIN` in `stdlib/MANIFEST.tsv`, plus an observable that
   `import std::…` resolves with no environment variable set, plus an observable that both Homebrew
   formulae install the tree. Neither does today; `grep -rn stdlib .github/` returns nothing.
3. **C FFI** (FFI-01…FFI-03) — the one feature-list-only item kept in 1.0. It is nearly free: the
   backend already emits C. FFI-03 is a `reject` row, because an FFI boundary that is not
   effect-classified is a hole in N7.

**Exit**: `make m8-exit`.

## M9 — The thesis (1.0.0)

**Waits on**: everything. **This milestone's exit is the definition of 1.0**, so it ships as `1.0.0`
rather than as another prerelease.

**Owns 15 requirement rows**; 23 rows across the manifest carry `disposition = thesis`.

1. **Rewrite `bootstrap/pdc.pd` in the differentiated dialect** — `ref`/`ref mut` parameters with
   inferred regions, at least one discharged `#[total]`, inferred effects reaching callers, no
   `async`/`await`, no lifetime parameter list (TH-01…TH-05).
2. **`make selfhost` still reaches a byte-identical fixed point.** This is the whole argument: the
   dialect has to survive contact with a real compiler, written in it.
3. **`make selfhost-corpus`** (SH-02…SH-04), which does not exist. Today's `make selfhost` proves
   `bootstrap/pdc.pd` compiles **itself**; the 1.0 claim is that it compiles **the language** —
   every corpus fixture, matching the Rust compiler on acceptances *and* refusals. Matching only on
   acceptances is satisfiable by a compiler that refuses nothing, which is why
   [F3](#f3-the-conformance-corpus-has-no-negative-tests) has to be closed first.
4. **Witness 2 in the dialect** (WT-02, TH-06).
5. **Parity with `src/`, not retirement.**

**Exit**: `make thesis-exit`. It reports **1 green and 21 RED** today; every RED line names the
milestone that owes it, and every absent fixture says `DECLARED, ABSENT` rather than passing.

## Scope: what is in 1.0, and what is not

Deferred, and recorded as `milestone = "post-1.0"` in `feature-index.toml` so the omission is
written down rather than silent: **a package registry · WASM · the LSP server · a debugger · a
formatter · Rust FFI · Lean/Coq proof export · refinement types · side-channel bounds.**

**C FFI is kept** (`milestone = "1.0"`): it is physically nearly free because the backend already
emits C, and a systems language whose I/O cannot leave the builtin set is not 1.0.

**Parallel-by-default and structured concurrency are `post-1.0`**, owner `P1`, blocked on **D2**.
This does not shrink differentiator 1: the feature list's differentiator is *"async without
coloring — no `async`, no `.await`, effects inferred and propagated"*, and M5 delivers all of it.
Automatic parallelization is a separate bullet on that list, and it is the one item whose semantics
are undefined.

Three feature-index rows remain **unclassified** and need a ruling: incremental compilation,
parallel compilation, and the LLVM backend. The thesis exercises none of them, and
[N1](../specification/language-spec.md#n1-overview-and-design-commitments) calls a second backend "a
second implementation of the same definition" — explicitly not a language property — so `post-1.0`
is the expected answer, but it was not in the reviewed cut list and is not assumed here.

## Decisions for the owner

### D1 — What gates 1.0 · **RESOLVED 2026-08-22**

Neither of the two options previously offered. Both were inventories, and "the inventory has no
unmet rows" is a completeness criterion — the generator of the fiction this repository spent M1
burning out. **1.0 is the thesis gate.** Scope follows from what the self-hosting compiler actually
uses. The manifest enumerates so that nothing is dropped silently; it does not gate.

Recorded as `D1-01`, whose evidence is `make thesis-exit` — currently, and correctly, RED.

### D2 — May the compiler emit an unnamed execution substrate?

Unchanged and still with the owner; it now blocks only `post-1.0` work. **This is not a
contradiction in N7.** Static effect tracking can compile into generated concurrency primitives with
no `Future` boxing and no programmer-visible executor. What N7 does not answer is whether the
compiler may emit **threads, an event loop, scheduling state, cancellation and joins that the
programmer never names**, and parallel-by-default requires *some* execution mechanism. The C backend
is not the obstacle — `runtime/palladium_runtime.c` already ships, and pthreads and kqueue are C.

- **Option A — permitted, unnamed.** *Cost*: N7 must then define three things it does not:
  **(i) sequencing** — what ordering is guaranteed between independent effectful operations, and
  what `effect::sync` guarantees beyond it; **(ii) cancellation** — when a branch fails or a
  `with_timeout` fires, are siblings cancelled, at which points, and are effects already performed
  observable (they are, and that must be stated); **(iii) errors** — if two branches fail, which
  error propagates: deterministically by source order, or nondeterministically by first-to-fail.
  Until those are written, N7-13/15/17 cannot be specified.
- **Option B — not permitted.** *Cost*: "parallel by default" and "automatic parallelization" leave
  N7 and the feature list. *Benefit*: the effect system stays a pure compile-time analysis with no
  scheduling semantics to reconcile with `#![total(strict)]`.

Either way **M5 is unaffected**.

### D3 — `str` and `usize` · blocks M7

**A** — add both primitives. *Cost*: two types through lexer, parser, checker and codegen, and `str`
needs a borrowed-string representation, so it depends on C1 and interacts with D4. *Benefit*:
`ref str` is the natural referent of `ref` and the differentiator's headline example survives.
**B** — rewrite the sites to `ref String` and `u64`. *Cost*: that example changes; lengths stay
signed. *Benefit*: no new primitives.

### D4 — Array parameters: value or reference semantics · blocks M7

Stated in full at [N12.1](../specification/language-spec.md#n121-array-parameters-open-decision).
**A** makes `[T; N]` a value type, so the three spellings mean three different things and every
array argument is a memcpy unless the author writes a reference. **B** makes it alias the caller's
storage, matching C, so the reference spellings are redundant and the specification must say which
is required for a parameter written through. Until answered the rule stays in code generation, where
the type system cannot enforce it.

### D5 — `src/async_runtime/mod.rs` · blocks nothing, `post-1.0`

498 lines whose only referrer in the repository is `src/lib.rs:5` (`pub mod async_runtime;`). No
compiler phase and no generated C uses it. **A** — delete it; N7 has no async runtime, so it can
never become one. **B** — keep it as the substrate D2 might permit. Not acted on: deletion is the
owner's.

## Findings

### F11. The async producer is alive, and violates N7 today

M1 fixed the `.await` **consumer** — `src/codegen/mod.rs:3055-3059` returns
`CompileError::await_unimplemented`. The **producer** was not touched.
`src/codegen/mod.rs:2007-2012` still dispatches on `func.is_async` into
`generate_async_function_with_name`, which emits a `Future` struct and a poll routine
(`src/codegen/mod.rs:3102-3110`, commented "Simplified async - immediately ready").

It is reachable, not dead code. Measured at `7484bac`:

```text
async fn g() { print("x"); }
fn main() { print("ok"); }
```

compiles, links, runs, and the generated C contains
`typedef struct g_Future { int state; } g_Future;`, `int g_poll(g_Future *future)`, and
`g_Future g()`. A *returning* `async fn` is caught earlier by the type checker — "expected
Future<Int>, found Int" — which is why this survived: the shape that reaches codegen is the
unit-returning one, and nothing tested it.

[N7](../specification/language-spec.md#n7-effects-and-asynchrony) is explicit: *"There is no async
runtime and no `Future` boxing. Effect tracking is entirely static and has no runtime
representation."* A `struct` with a `state` field, emitted into the program's own C, is a runtime
representation. **This is a normative violation shipping today**, it is N7-18, and it is M2 work —
refuse `async fn` at codegen exactly as `.await` already is, ahead of M5 deleting the keyword.

Companion: `src/async_runtime/mod.rs` (498 lines, sole referrer `src/lib.rs:5`) is N7-19 and
decision **D5**. Not deleted here.

### F14. The gate that defines 1.0 could never say 1.0 was reached

Round two closed "the gate cannot go RED". Round three found its mirror image, and it is the same
disease: **the gate did not measure.** `D1-01` — the row whose evidence is `make thesis-exit`
itself — was recorded `False` unconditionally, with no transition. Success required every row
green, so **exit 0 was unreachable by construction**. A gate that can only ever say no is exactly
as uninformative as one that can only ever say yes.

A self-referential row is not a member of the set it measures; it *is* the aggregate. It is now
excluded from evaluation and answered by the summary line, and the self-test's **first** case
drives an all-green repository state and asserts **exit 0** — so this cannot return silently.

Six more probes of the same family, each now with a control that fails on revert:

| Was | Now |
|---|---|
| `run_conformance` ignored the exit status and had no timeout: a run that emitted parsable verdicts and then failed was accepted, and a hung one hung the gate | every subprocess goes through [`scripts/gate_probe.py`](../../scripts/gate_probe.py), whose `classify()` yields `Concluded` (has `.text`) or `Malfunction` (**no text attribute at all**), with the timeout inside `run()` |
| TH-05 parsed effect output without requiring the compile to succeed | `effect_report` refuses to return text from a `pdc` that did not conclude *or* did not succeed |
| a `HarnessError` from conformance or witness reading was caught and turned into ordinary red rows, so a failure to measure exited 1 | measurement failure propagates and exits **2**; only an artifact the repository does not contain is a finding, and it says `DECLARED, ABSENT` |
| `p_effect_is_transitive` returned true for any reported function with no *recognised* builtin call — including one that called **nothing** | the edge `caller -> callee -> builtin` must be exhibited, with all three named |
| `p_total_on_fn` called a function "live" if its name appeared in any body — a dead caller, or its **own recursive call**, sufficed | reachability from `main`, with self-edges excluded |
| `thesis_rows` validated only the column count: an unknown kind, a duplicate id or a **retyped row** dropped out of dispatch while the summary still printed the full count | the id set is **pinned**, an unknown kind is a harness error, and one result per row is asserted |

And the self-test itself, for the second time: it called the probe helpers directly, so deleting
the production wiring left every case green. It now builds a temporary repository — requirements
TSV, witnesses, conformance verdicts, `make` results, effect reports — and drives `main()`,
asserting the **exit code**. Sixty-eight cases, of which five drop the injection entirely and drive
the real subprocess boundary — including one where conformance, `pdc` and `make` all run and
conclude successfully, so the *green* path is exercised and not only the failures. The one probe
group with no negative control (the real `make` subprocess) is a **disclosure pinned verbatim**:
emptying *or rewording* it fails the self-test. It is not a derived check, and says so — nothing
computes which probes lack a control.

Two things it caught that review did not. `fn f< 'a>(x: i64)` — a *spaced* lifetime parameter
list — **compiles today**, and `grammar.ebnf:129` makes whitespace insignificant between tokens,
so TH-02's adjacency-only `<'` missed a real violation. And running the repaired gate against the
real repository showed TH-05 compiling a witness *before* checking whether it existed, so an
absent witness exited 2 instead of reporting a finding — the very distinction that round's work
was about, inverted, one function away from where it was being fixed.

**The RED count moved 22 → 21, and no probe got weaker.** The only change is that `D1-01` left
the evaluated set to become the aggregate. `SH-01` is still the sole green row, and it is green
because `make selfhost` genuinely passes.

Cross-branch constraint, now enforced rather than hoped for: a reject fixture can go green on a
sibling branch **without a compiler change**, and a runner that sees only `REJECTED` cannot tell
"refused because the prohibition is enforced" from "refused for incidental unsupported syntax".
So the manifest gained a ninth column and each thesis reject row **names the diagnostic its
refusal must carry**.

The chain, stated exactly, because condition 3 rests on it: `scripts/conformance.sh:636` runs
`grep_status F "$fp" "$TMPROOT/diag"`, and `grep_status` (`scripts/conformance.sh:145-152`)
mode `F` is `grep -qF`. So the corpus's declared fingerprint is matched as a **literal
substring of any line of the ANSI-stripped compiler log** (`scripts/conformance.sh:635`) — not
an equality, not a regex. A log it cannot read is a third outcome, `HARNESS_ERROR`, kept
distinct from "did not match" (`scripts/conformance.sh:684-689`). The thesis gate then requires
the corpus's declaration to **equal** the fingerprint its row pins. Equality on the half this
gate owns, substring on the half `conformance.sh` owns, and both stated rather than assumed —
a sibling branch was caught doing substring where it meant equality.

### F13. The first thesis gate was blind in the way M1 spent itself curing

The command that defines 1.0 shipped, in `8acfd48`, checking **manifest text**. Its `row_is` asked
whether an editable line said `run` or `reject`; it ran nothing. So a missing fixture, a malformed
row, **a reject twin the compiler happily accepted**, or a rejection for an entirely unrelated
reason all reported green — inside condition 3, which exists because *for an inference feature the
rejection is the product*. Two external reviewers rejected it and found six more probes of the same
shape. Counting F12, that is the thirteenth occurrence of this repository's signature defect, at the
highest-stakes location it has yet occupied.

The repair was not more text validation. `scripts/conformance.sh` already compiles, links, runs,
diffs stdout against a recorded transcript, checks the declared failure *stage*, matches the declared
*diagnostic fingerprint*, reports `REJECT_ACCEPTED`, and reports `MISSING`. The gate now delegates
to it and reads only its verdicts — the same move as replacing a hand-rolled module scanner with
`cargo test --list`.

Six further probes were blind. Each fix has a negative control that fails when the fix is reverted:

| Probe | What it accepted | Now |
|---|---|---|
| TH-02 | `sed "s/ref<'…>/ref/"` had no identifier boundary, so `fn myref<'a>(…)` became `fn my(…)` and a forbidden lifetime list passed | the exemption is anchored; three negative cases, including `myref<'a>` |
| TH-05 | any output containing `has effects` and an IO spelling — and `bootstrap/pdc.pd:49-51` calls `file_write` **directly**, so it passed on a direct effect while claiming propagation | it names a caller that performs no IO itself, the callee it reaches, and the builtin that callee calls |
| TH-03 | any `: ref T` anywhere, including a struct field or a local annotation | it parses each `fn` parameter list; a field and a local are both negative cases |
| TH-04 | the bare text `#[total` plus a whole-file compile, so an unused trivial function satisfied it | an attribute token attached to a `fn` that is actually called |
| TH-06 | a manifest label plus lexical decoration; it never ran the witness | witness 2 must be `PASS_VERIFIED` **and** pass every source probe |
| `--self-test` | called the helpers directly, so deleting the production wiring left all six cases green; no control at all for TH-03/04/05, SH-*, C2, C3, C4 | 29 fault-injection cases, and the two probe groups that still have no negative control are **named in the output** instead of left silent |

Two harness defects of the same shape are closed with them: an unreadable file made the scanner
yield the empty string, so TH-01/TH-02 reported **green** — a failure to measure read as a passing
measurement, the `total=0, exit 0` class `conformance.sh` already fixed once; and `MANIFEST` was
assigned and never read, so 1.0 had two definitions and only one was checked. A harness error now
exits 2 and says it is not a verdict, and the gate reads the 23 `thesis` rows out of the manifest
rather than restating them.

**The RED count went from 11 to 22, and that increase is the deliverable.** TH-01 and TH-02 were
green only because `bootstrap/pdc.pd` happens to contain no `async` and no lifetimes — a prohibition
satisfied by absence — while the second witness the same condition covers does not exist at all.
They are now honestly red.

### F12. The thesis gate's first lexer could not fail on what it checked

The first `strip_literals` treated every `'` as a quote. In `fn f<'a>(x: ref String)` the tick has
no partner, so the scanner consumed from it to end of file, and **TH-02 could never fire** — a green
line that could not go red, in the gate that defines 1.0. Caught by writing the self-test, not by
reading the code; it is F13's first instance and the reason the rest were looked for.

One deliberate consequence survives: the gate's scanner treats block comments as **non-nesting**,
because `bootstrap/pdc.pd:164-175` shows the compiler scanning for the first `*/` and breaking, with
no depth counter. [N2](../specification/language-spec.md#n2-lexical-structure) requires nesting and
the compiler does not implement it (F10, requirement N2-08). A gate that nested would disagree with
the compiler about whether a real `async` is commented out, so it matches the implementation, and a
self-test case pins that behaviour — it fails when N2-08 lands, forcing the two to flip in lockstep.

### F2. M1 shipped three of its own declared failures, and its exit command could not see them

`make m1-exit` exits 0 at `2ef170f`. It is `CONFORMANCE_FORBID_OWNER=M1` over
`tests/conformance-manifest.txt` and nothing else (`Makefile:270-271`), and no row there is owned by
M1. The second owner inventory — the `(owned by M<n>)` tag every `#[ignore]` reason carries and
`scripts/test-xfail.py:74` parses — has three M1 rows, all still red:

| Row | What is still broken |
|---|---|
| `tests/e2e_test.rs:309` | a tail `if` is not lowered to a return |
| `tests/compiler_comprehensive_test.rs:567` | `fn f() -> int { }` compiles with no diagnostic |
| `tests/e2e_test.rs:269` | `fn double` emits `long long double(…)`; gcc rejects the compiler's own output |

The first reproduces: `fib(10)` prints `8261746944` and exits 0. **A silent miscompile shipped in
the release named for removing silent miscompiles.**

Re-measured for this revision: `make m1-exit` still **exits 0**. A brief handed to this unit
expected it red; it is not, and the reason is the finding itself — the target reads the conformance
manifest, no row there is owned by M1, and the three rows that are owed to M1 live in the other
inventory. Reading both inventories fixes the omission but
not the class: owners are editable and the Rust inventory is whatever ignored tests `cargo` lists,
so deleting a test silently shrinks it. Hence a closed manifest — and, above it, a gate that rests
on a fixed point rather than on any inventory at all.

### F3. The conformance corpus has no negative tests

`reject=0` on every run. The class exists, the runner implements it, and
[A11](../specification/language-spec.md#a11-conformance) advertises it — and no fixture uses it. The
refusals are covered in Rust integration tests (`tests/d5_unimplemented_constructs.rs`,
`tests/d10_llvm_refuses.rs`), which the bootstrap compiler will never run. 23 manifest rows are
`reject` rows, three of them are thesis conditions, and M9's parity claim depends on them existing.

### F4. Two differentiators owned no failing row anywhere

Implicit lifetimes: zero conformance rows, zero `#[ignore]` rows. Totality: zero and zero. The
features are absent rather than broken, and absence has no fixture, so an owner-filter exit would
have gone green the day it was added. The manifest is one repair (M6 owns 12 rows, M7 owns 19, all
`owed`); the thesis gate is the stronger one, because it demands the features be *used* by a program
that cannot fake using them.

### F5. Two documents define 1.0 — superseded by D1

Recorded for provenance: the manifest previously had to choose between Part I and the feature list.
D1 resolved it by choosing neither. The LLVM backend remains the sharpest instance of the
disagreement and is one of the three rows still unclassified in
[Scope](#scope-what-is-in-10-and-what-is-not).

### F6. A second roadmap in the tree was written in the fictional present

[`docs/design/vision-roadmap.md`](../design/vision-roadmap.md) opened "Palladium α v0.7 has achieved
what many thought impossible", carried a benchmark table against Rust 1.74 for a compiler that could
not link a hello-world when it was written, and scheduled "Q4 2026: Language Freeze". Its body is
replaced with a supersession pointer.

### F7. Stale claims in documents this file rests on

- [A11](../specification/language-spec.md#a11-conformance) said "over 44 fixtures", "verified 33 ·
  vacuous 7 · xfail 2 · skip 2" and named "the three failures", against a measured
  `verified=43 … xfail=1 … failures=0` over 53. **Corrected**, because this file treats the annex as
  an authority and stale data in an authority is release governance, not a documentation nit.
- `feature-index.toml`'s `async_as_effect` row claims `cmd: grep -rn 'effects::' … -> 1 line`.
  Re-run, it returns **8**. `scripts/check_doc_evidence.py:50` and `scripts/check_doc_evidence.py:317-318`
  only regex-match the *shape* `cmd: X -> Y`, so a `cmd:` item's output is never checked.
  **A separate unit owns that repair**; not touched here.
- `CLAUDE.md` describes `bootstrap/pdc.pd` as "~760줄". It is 991 lines — the number that makes
  M3's move to the front an argument rather than a preference.

### F8. One declared failure expects syntax the specification forbids

`tests/advanced_features_test.rs:340` is an `xfail` whose reason is that `macro_rules! vec { … }`
"is not an item". Under [N3](../specification/language-spec.md#n3-program-structure-and-items) it
must never be one, and `scripts/check-doc-evidence.sh` already fails any normative document that
writes it. A row that stays red unless the language changes is a negative test wearing the wrong
class. N3-14 makes it a normal, passing `reject` fixture owned by M2.

### F9. The milestone labels in the test suite were written against the old numbering

Against this sequence: the 14 tagged M2 stay **M2**; the 18 tagged M4 become **M3**; the 3 tagged M1
become **M2**; the 5 tagged `unscheduled` become **M5**; the 1 tagged M5 leaves the inventory (F8).
**26 re-tags and one reclassification.** *(An earlier draft said 27; it counted the reclassified row
as a re-tag as well.)* They must land **atomically, before any owner-filtering target ships**, with
the self-test that proves the filter detects a planted row. Each reason should also gain a
`req: <id>` tag, which is what turns the Rust half of the reconciliation into a command. All of it
is an edit to `tests/`.

### F10. Block comments do not nest, and nothing said so

[N2](../specification/language-spec.md#n2-lexical-structure) requires `/* … */` to nest.
`fn main() { /* a /* b */ c */ print("ok"); }` fails with `Expected expression, but found '/'`.
[A2](../specification/language-spec.md#a2-lexical-structure) records comments as implemented without
qualifying nesting — a normative requirement with no annex row and no test, found by enumerating N2
rather than by any gate. N2-08, owned by M2.

## Keeping this file honest

```bash
make thesis-exit         # the definition of 1.0. RED until M9, by design
make test-thesis-runner  # and the proof that it can still go red
make gates               # conformance + gate self-test + docs + selfhost + stdlib + probe
make test-honest         # every test binary, integration tests included
make test-xfail          # every declared failure, and the milestone that owes it
```

Five rules this file is held to:

1. **1.0 is a command, not a count.** If the definition of done can be satisfied by a table with no
   red cells, it can be satisfied without the language existing. It has been, twice.
2. **If a milestone's exit criterion cannot be written as a command, it is not an exit criterion.**
3. **The exit command covers the goal, not a proxy.** For an inference feature that means the
   rejection, not only the acceptance: an inferencer that accepts everything is a no-op, and a no-op
   is invisible in green fixtures.
4. **A milestone owns rows in a closed inventory.** Deleting a row, moving it between milestones, or
   moving it to `post-1.0` is a contract transition and is reviewed as one.
5. **Paying off a row is a transition, not a deletion.** The fixture stays on disk and its row
   becomes `run` with a transcript, in the same commit.

Every `file:line` here is fingerprinted by `make check-doc-evidence`. What that gate cannot do is
check a `cmd:` item's *output* — see [F7](#f7-stale-claims-in-documents-this-file-rests-on).
