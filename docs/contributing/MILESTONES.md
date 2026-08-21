# Milestones

**Updated**: 2026-08-22 · **Released**: v0.3.0 (M1) · **Target**: 1.0.0

Ordered by what unblocks what, not by theme. Every milestone exits on one command that covers its
whole goal, and every milestone ships.

## The shape of the plan

**1.0 is [`1.0-requirements.tsv`](1.0-requirements.tsv)** — 178 rows enumerated from
[Part I of the specification](../specification/language-spec.md#part-i-normative-specification),
one per thing 1.0 requires, each with an owning milestone and its own exit evidence: a positive
fixture with a transcript, a negative fixture with a diagnostic, a runtime observable, a gate, or
an owner decision that must be made before the row can be written at all. Today **31 are
satisfied, 138 are owed and 9 are blocked on a decision**.

That file exists because the first draft of this one got the definition of 1.0 wrong in the way
this project keeps getting things wrong. It defined 1.0 as *"no row in `feature-index.toml` whose
`spec` names a Part I anchor is other than `implemented`"* — a criterion computed by filtering an
index. The three differentiators do not name Part I anchors; they name their feature documents.
**That gate went green with all three unimplemented.** A criterion satisfiable without the thing it
exists to measure is the same defect as a conformance fixture that only prints "not yet
implemented", one level up. The repair is not a better filter. It is enumeration: a closed
inventory, in the same sense and with the same vocabulary as
[`tests/conformance-manifest.txt`](../../tests/conformance-manifest.txt), where adding a
requirement is an edit to the file and removing one is a contract transition.

Three of those requirements are the reasons for this language to exist rather than to be a Rust
dialect, and the
[feature list](../reference/features/PALLADIUM_V1_FEATURES.md#unique-advantages-over-rust) leads
with them:

| # | Differentiator | Normative | Definition | Rows |
|---|---|---|---|---|
| 1 | Asynchrony is an effect — no `async`, no await operator, no colouring | [N7](../specification/language-spec.md#n7-effects-and-asynchrony) | [`async-as-effect.md`](../reference/features/async-system/async-as-effect.md) | N7-01…N7-17 |
| 2 | Termination is provable — `#![total(strict)]`, `#[decreases(expr)]` | [N8](../specification/language-spec.md#n8-totality) | [`totality-checking.md`](../reference/features/advanced/totality-checking.md) | N8-01…N8-12 |
| 3 | Lifetimes are inferred — `ref` / `ref mut`, no `'a` | [N9](../specification/language-spec.md#n9-references-and-lifetimes) | [`implicit-lifetimes.md`](../reference/features/core-language/implicit-lifetimes.md) | N9-01…N9-09 |

## What actually blocks what

An earlier draft claimed all three differentiators block on one type-system milestone. **That was
wrong, and it pushed two of them years to the right for no reason.** The real graph has four
independent capabilities:

| | Capability | What it is | Required by | Waits on |
|---|---|---|---|---|
| **C1** | Reference typing | `Type::Reference` is a distinct type in the checker, carrying mutability. Today it is mapped to its inner type, so `&i64` and `i64` are the same type (`src/typeck/mod.rs:121-125`) | N9 in full · N12's move semantics and drop glue · moving the array-parameter rule out of codegen ([A9.2](../specification/language-spec.md#a92-array-parameters)) · N10's `self` receivers · C4 | nothing |
| **C2** | Call-graph fixed point | Per-function summaries propagated to a fixed point, unknown callees not assumed pure, `impl` methods included. Today it is a single source-order pass whose fallback is "conservatively assume it's pure" (`src/effects/mod.rs:280-284`) | N7's inference and gating · N8's propagation of totality to callees, which is the same shape | a surface that can state an expectation |
| **C3** | Inductive pattern support | Patterns rich enough for structural recursion to have subterms. Enums, construction and `match` already work ([A4.3](../specification/language-spec.md#a43-enums)); what is missing is literal, range, or-, tuple and guard forms | N6 in full · N8's automatic structural termination, **monomorphically** | the parser |
| **C4** | Alias-sensitive scheduling | Deciding two effectful operations are independent, which is an aliasing question | N7's parallel-by-default and structured concurrency only | C1, and decision **D2** |

Three consequences the earlier draft missed, each of which moves a milestone:

- **Basic effect inference and gating need none of C1, traits, generics or a standard library.**
  The builtin registry already carries an effect classification per builtin (`src/builtins.rs:182`),
  and the analyser already unions effects across statements and calls. What is missing is a fixed
  point, the `impl` traversal, a surface to state "this must be pure", and a consumer — the result
  is `println!`ed and nothing reads it (`src/driver/mod.rs:147`, `src/driver/mod.rs:151-157`). So
  N7's core is an early milestone, not a late one.
- **Monomorphic totality needs none of C1 or N10 either.** Measure checking is arithmetic over an
  attribute expression, and structural recursion over the enums that already exist is checkable
  with C3. Only structural recursion over *generic* inductive types waits for N10, so N8 splits
  into two rows at two milestones (N8-06 and N8-07) instead of waiting entirely.
- **Only parallel-by-default plausibly needs the capability model**, and it also needs an owner
  decision (**D2**) before it can be specified at all.

So one differentiator is genuinely deep (N9 needs C1, which needs nothing but itself and is
expensive), one is early (N7's core needs C2), one is early-and-split (N8 needs C3 now, N10 later),
and the piece that is neither early nor cheap — parallel-by-default — is isolated at the end where
its decision can be taken without holding anything else up.

| Milestone | Version | What it is | What it waits on |
|---|---|---|---|
| M1 ✅ | v0.3.0 | The compiler stops lying | — |
| M2 | v0.4.0 | The surface, and M1's unpaid debt | M1 |
| M3 | v0.5.0 | Effects gate something · **differentiator 1, core** | M2's annotation surface → **C2** |
| M4 | v0.6.0 | Termination is provable, monomorphically · **differentiator 2** | M2's patterns and attributes → **C3**; M3's propagation → C2 |
| M5 | v0.7.0 | Reference typing, lifetimes, memory model · **differentiator 3** | nothing but itself → **C1**. Scheduled here because M6 and M7 need it |
| M6 | v0.8.0 | Traits, generics, modules | C1 |
| M7 | v0.9.0 | The standard library | M6, C1 |
| M8 | v0.10.0 | Parallel by default · **differentiator 1, completed** | C1, C2, **decision D2** |
| M9 | v0.11.0 | The Rust compiler becomes redundant | everything |
| — | **1.0.0** | The first release that is not a prerelease | |

**Versioning.** M1 shipped as `v0.3.0`. Every milestone from here ships the same way: one release
per milestone, every `0.x` a prerelease in the semver sense. **1.0.0 is the first non-prerelease**,
and it means the requirement manifest is fully satisfied — see
[1.0.0](#100--the-first-release-that-is-not-a-prerelease).

**Self-hosting is a floor throughout, not a milestone until M9.** `make selfhost` must stay green
at every commit, and PBS-1 grows with each milestone whose constructs the bootstrap compiler must
consume. M9 is where the remaining gap closes and `src/` retires. The earlier draft put this at M3,
arguing that abstraction should be implemented once, in Palladium; that argument inverts once you
notice what would then be written in a pre-abstraction Palladium — region inference, an effect
fixed point and a totality checker, in a language with no generics, no traits and no `Vec`. The
Rust compiler stays the implementation vehicle until the language is finished.

## Where the project actually is

Measured at `2ef170f`, not read from the previous version of this file.

| | | Command |
|---|---|---|
| Self-hosting | fixed point over PBS-1 — stage1 and stage2 C are byte-identical (`9b0cf24e…`) | `make selfhost` |
| Conformance | `verified=43 untranscribed=0 vacuous=7 xfail=1 reject=0 skip=2 failures=0` over 53 fixtures | `make conformance` |
| Conformance gate itself | 96 cases, each pinning a way it must still go RED | `make test-conformance-runner` |
| Documentation | every snippet compiles; 226 citations fingerprinted, 28 no-compile fences pinned, 48 feature rows all with tagged evidence | `make check-docs` |
| Rust tests | 620 pass, **0 fail**, 42 ignored (524 lib + 96 integration) | `make test-honest` |
| Declared failures | 41 `xfail` + 1 `slow`, each naming the milestone that owes it, none of them passing | `make test-xfail` |
| `stdlib/` | 0 of 21 files compile; 6 drivers inventoried; 38 builtins accounted against a normative 34 | `make stdlib-gate` |
| Traits · generics · effects · async · unsafe · modules | conformance coverage is **zero** for each | `make conformance` (vacuous rows) |
| 1.0 requirements | 31 satisfied · 138 owed · 9 blocked, over 178 rows | [`1.0-requirements.tsv`](1.0-requirements.tsv) |

Two lines in the previous version of this file are retracted by that run: "Unit tests 404 pass, 2
pre-existing failures" and "Integration tests 43 fail, all pre-existing". `make test-honest` exits
0 at `2ef170f`. The failures did not disappear — M1 converted them into 41 declared `xfail`s, each
naming the missing feature *and* the milestone that owes it.

## The five inventories the manifest was derived from

**1. Part I, by section** — the annex's own summary table:
`sed -n '/^| Normative section | Status/,/^$/p' docs/specification/language-spec.md | awk -F'|' 'NR>2{print $3}' | sort | uniq -c`

| Status | Count | Sections |
|---|---|---|
| implemented | 1 | N13 |
| partial | 9 | N1 N2 N3 N4 N5 N6 N11 N12 N14 |
| unimplemented | 4 | **N7 N8 N9**, N10 |

**2. Per feature** — `feature-index.toml` via `tomllib`: 48 rows, **4 implemented · 16 partial ·
28 unimplemented**. Fifteen of the 48 name
[`PALLADIUM_V1_FEATURES.md`](../reference/features/PALLADIUM_V1_FEATURES.md) as their `spec` rather
than a Part I section, which is decision **D1**. This index is an input to the manifest and not its
source: it is organised by feature, not by requirement, and it was never built to be exhaustive.

**3. Conformance debt, by owner** — `make conformance`, tail of the run

| Owner as tagged | xfail | vacuous | Rows, and where they go |
|---|---|---|---|
| M3 | 1 | 1 | `tests/projects/hello_pdm/tests/test_math.pd` · `tests/12_modules_imports.pd` — both now **M6** |
| M4 | 0 | 2 | `tests/07_traits_basic.pd` · `tests/08_generics_basic.pd` — both now **M6** |
| unscheduled | 0 | 4 | `tests/02_types_enums.pd` (**M2**) · `tests/09_effects_system.pd`, `tests/10_async_await.pd`, `tests/11_unsafe_blocks.pd` (**M3**) |

`reject` is empty — the corpus has no negative tests at all
([F3](#f3-the-conformance-corpus-has-no-negative-tests)). Every row above is named by a requirement,
and that direction of the reconciliation is checked: no `xfail` or `vacuous` row is unaccounted for.

**4. Declared Rust failures, by owner** — `make test-xfail`; owners parsed from the `#[ignore]`
reason by `scripts/test-xfail.py:74`

| Owner as tagged | Count | Shape of the debt |
|---|---|---|
| M4 | 18 | generics, traits, closures, function types, const generics, `?` |
| M2 | 14 | `else if`, `loop`, `+=`, bitwise, `as`, literal patterns, method calls, tuples, `const`/`static`, inline `mod` |
| unscheduled | 5 | `async fn` wrapped in `Future`; `effect` is not an item |
| **M1** | **3** | still owed by a milestone that shipped — [F2](#f2-m1-shipped-three-of-its-own-declared-failures-and-its-exit-command-could-not-see-them) |
| M5 | 1 | `macro_rules!` — owed to nobody; it expects syntax N3 forbids ([F8](#f8-one-declared-failure-expects-syntax-the-specification-forbids)) |

The tags predate this numbering and need re-mapping
([F9](#f9-the-milestone-labels-in-the-test-suite-were-written-against-the-old-numbering)).

**5. Open defects** — [`CLAUDE.md`](../../CLAUDE.md) "남은 결함" and the annex. Ownership is now
carried by the requirement manifest; this table is the reading list.

| Defect | Where | Requirement |
|---|---|---|
| D3b — a tail `if` is not lowered to a `return`; `fib(10)` prints `8261746944` and exits 0 | [A6.6](../specification/language-spec.md#a66-tail-expressions) | N3-02, N3-03 |
| C-keyword identifiers — `fn double` emits `long long double(…)`; gcc rejects the compiler's own output after it prints "Compilation successful" | `tests/e2e_test.rs:269` | N3-01 |
| No missing-return diagnostic — `fn f() -> int { }` compiles silently | `tests/compiler_comprehensive_test.rs:567` | N3-03 |
| Block comments do not nest, which N2 requires — measured here, not previously recorded | [F10](#f10-block-comments-do-not-nest-and-nothing-said-so) | N2-08 |
| `a * -b` does not parse | [A6.3](../specification/language-spec.md#a63-expression-forms) | N5-16 |
| Nested arrays work in neither locals nor parameters | [A5](../specification/language-spec.md#a5-types) | N4-10 |
| Six builtins that cannot compile — the handle representation split in two | [A8](../specification/language-spec.md#a8-builtins) | N14-01, N14-03 |
| `pub` on an enum discarded; `dbg!` calls an undefined `print_debug`; `println!` takes one argument; no hygiene | [A4.3](../specification/language-spec.md#a43-enums), [A4.6](../specification/language-spec.md#a46-macros) | N3-05, N3-12, N3-13 |
| `Foo<T>` is parsed as a *const* generic argument; const generics are not monomorphised | [A5](../specification/language-spec.md#a5-types) | N10-03, N4-21 |
| Traits emit no C; a trait method with a `self` receiver is a parse error | [A4.4](../specification/language-spec.md#a44-traits) | N10-06, N10-09 |
| `Type::method(args)` lowers to a `Type_method__new` codegen never emits | [`CLAUDE.md`](../../CLAUDE.md) | N5-17 |
| `&mut` of an immutable local is accepted for struct referents | [A9.3](../specification/language-spec.md#a93-mut-of-an-immutable-local-is-accepted) | N12-06 |
| `String` is a Copy handle, contradicting N12 — no drop glue exists | [A9.1](../specification/language-spec.md#a91-string-is-a-copyable-handle-decision-2026-08-21) | N12-03, N12-04 |
| Effects gate nothing; propagation assumes unknown callees pure; `impl` methods are never analysed | [A4.1](../specification/language-spec.md#a41-functions), [async-as-effect](../reference/features/async-system/async-as-effect.md#where-the-implementation-currently-diverges) | N7-03…N7-08 |
| Attributes do not lex — `#[total]` fails at the character `#` | [A2](../specification/language-spec.md#a2-lexical-structure) | N2-10, N2-11 |
| The LLVM backend refuses unconditionally; 14 sites fabricated rather than lowered | [A1](../specification/language-spec.md#a1-pipeline-and-backends) | decision **D1** |

## How a milestone exits

**One command per milestone, covering the whole goal — accepted programs as well as refused ones,
runtime observables where the goal is a runtime property, packaging where the goal is a shipped
artifact.** Three lines of Makefile each:

```make
m3-exit: build
	@REQ_MILESTONE=M3 bash scripts/requirements.sh
```

`scripts/requirements.sh` does not exist yet and is the one change this plan needs outside
documentation. It is specified here precisely enough to write:

1. Parse [`1.0-requirements.tsv`](1.0-requirements.tsv) — seven tab-separated columns, all
   mandatory. A row with a missing column, an unknown evidence kind or an unknown status is a
   failure of the manifest, not of the milestone.
2. For the milestone named by `REQ_MILESTONE`, **every** row must be `satisfied`. An `owed` or
   `blocked` row fails.
3. Resolve each evidence locator by kind, and *run* it:
   `fixture` → a `run` row in `tests/conformance-manifest.txt` whose transcript matches ·
   `reject` → a `reject` row refused with its declared diagnostic ·
   `skip` → a `skip` row proven so by the compiler ·
   `observable` → a named Rust test that exists, is not `#[ignore]`d, and passes ·
   `gate` → a make target that exists and exits 0 ·
   `decision` → the decision is recorded as resolved in [Decisions](#decisions-for-the-owner).
4. Reconcile both debt inventories, in both directions — the rule is written at the top of the
   manifest. The conformance half is checkable today, by path. The Rust half needs a `req: <id>`
   tag in each `#[ignore]` reason, which is an edit to `tests/`.
5. `make test-requirements-runner` plants a row for the milestone under test and proves the runner
   goes RED for it (requirement GI-09). A filter nobody has watched fail is not a filter.

**Why an aggregate and not the two owner filters.** `CONFORMANCE_FORBID_OWNER` and a matching
`TEST_XFAIL_FORBID_OWNER` clear only *tagged proxies*: they prove no declared failure still names
the milestone. They cannot prove the milestone's feature works, because a feature nobody wrote a
red test for produces no tagged proxy to clear — which is exactly how a milestone goes green with
its goal unmet, and it applies to all eight, not to the two this file first noticed. The two
filters stay, as fast pre-checks and because
[F2](#f2-m1-shipped-three-of-its-own-declared-failures-and-its-exit-command-could-not-see-them)
needs them; the manifest is what decides.

**Why the manifest is closed.** Owners are editable and the Rust inventory is whatever ignored
tests `cargo` currently lists, so **deleting a test silently shrinks it**. A row in the manifest
that names a deleted artifact fails. Deleting the row, or moving it to another milestone, changes
what 1.0 means and is a reviewed contract transition — the same discipline
`tests/conformance-manifest.txt` already applies to fixtures, and the same vocabulary.

---

## Completed

### M1 — The compiler stops lying (v0.3.0, released 2026-08-22)

Every other kind of work was slower while the compiler could accept a program and emit wrong code.
M1 converted silent wrongness into diagnostics, and — the part that outlives it — made the gates
able to fail.

Receipts:

| What | Evidence |
|---|---|
| **D5** `?` and `.await` emitted C referencing a `struct Result` layout and a `poll` member codegen never generates | Both refused at typecheck with the consequence and a workaround; old lowerings deleted, not flagged. `tests/d5_unimplemented_constructs.rs`, 12 tests |
| **D4** `for` over an array *parameter* used `sizeof` on a decayed pointer | The bound comes from the declared length; an unresolvable length is a compile error, not a wrong bound. `tests/regression/for_over_array_param.pd` |
| **D9** `&[T; N]` / `&mut [T; N]` parameters rejected in codegen | Lowered; a write that reaches the caller can only come from a spelling that declared it ([A9.2](../specification/language-spec.md#a92-array-parameters)). `examples/practical/simple_sort.pd` runs |
| **D7** an un-annotated `let` was emitted as `long long` regardless of its initializer | Fixed in `04104c5` |
| **D6** was not a defect | Retracted with five re-run probes ([A9.4](../specification/language-spec.md#a94-defect-d6-retracted)) |
| The LLVM backend fabricated rather than lowered at 14 sites, seven of them silently | `--llvm` refuses unconditionally. `tests/d10_llvm_refuses.rs`, 9 tests |
| `stdlib/` had no coverage at all | `make stdlib-gate`: 21 files pinned per file, 38 builtins accounted, generated C checked structurally. The premise was wrong and is recorded as such — **0 of 21 compile**, so nothing there was ever miscompiled ([`stdlib/STATUS.md`](../../stdlib/STATUS.md)) |
| A green exit code was counted as a correct program | Every `run` fixture is diffed against a recorded transcript; there is no exit-code-only class |
| Seven fixtures proved nothing while counting as coverage | Declared `vacuous`, each naming the feature it fails to cover. Seven of 53, on the summary line of every run |
| The gates could not fail | `make test-conformance-runner` (96 cases), `make test-gate-probe` (every evidence producer fault-injected) |
| `tests/*.rs` never ran under `make test-rust` | `make test-honest`, and every remaining failure converted to a declared `xfail` with an owner |

Not paid, and re-owned by M2: three M1 `#[ignore]` rows
([F2](#f2-m1-shipped-three-of-its-own-declared-failures-and-its-exit-command-could-not-see-them)).

---

## M2 — The surface, and M1's unpaid debt (v0.4.0)

**Waits on**: M1. **Delivers**: the surface every later milestone is written in, plus **C3**'s
parser half and the attribute token the whole of N8 sits below.

**Owns 45 requirement rows**, seventeen declared `#[ignore]` failures (fourteen tagged M2, three
tagged M1), and the vacuous `tests/02_types_enums.pd`, whose row says `unscheduled` while enums are
[implemented](../specification/language-spec.md#a43-enums) — payable today by rewriting the fixture
to declare one.

1. **The M1 debt first, because it is a live miscompile.** A tail `if` is not lowered to a return:
   `fn fib(n: i64) -> i64 { if n <= 1 { n } else { … } }` compiles clean and `fib(10)` prints
   `8261746944` (N3-02). The missing-return diagnostic lands with it (N3-03), as
   `tests/compiler_comprehensive_test.rs:567` already says it must, and with them C-keyword
   identifier mangling (N3-01).
2. **Statements and expressions** (N5-03…N5-17): `if`, `match` and blocks become expressions;
   `else if`; `loop` with a value-carrying `break`; compound assignment; bitwise operators; ranges;
   `as` casts; `a * -b`; method call syntax; top-level `const` and `static`.
3. **Patterns** (N6-02…N6-11) — literal, range, tuple, or- and `@` patterns, guards, exhaustiveness
   for **every** scrutinee type, and a trap where a `match` currently falls through no arm. This is
   **C3**, and M4 cannot start without it.
4. **Lexical completion** (N2-03…N2-11): float and char literals, escapes, **nesting block
   comments** — which N2 requires and which do not nest today
   ([F10](#f10-block-comments-do-not-nest-and-nothing-said-so)) — and **the `#` attribute token**.
   The token only; the checker is M4. An attribute that lexes and is then ignored would recreate
   the class M1 removed, so N2-11 requires an unknown attribute to be a compile error from the day
   `#` lexes.
5. **A surface for stating an effect expectation**, so that M3 has something to gate against.
   Whether that is an attribute or the `![io]` clause the grammar already names is M3's design
   call; the lexing is M2's.
6. **The six builtins that cannot compile** (N14-01, N14-04).
   [N14](../specification/language-spec.md#n14-builtins-and-the-standard-library) already decides
   half of it: the four `*_ex` names are not part of the language, so they leave `BUILTINS`;
   `file_flush` and `file_seek` are normative and get re-based on the index handle. Signatures
   returning `Result` wait for M6 (N14-03).
7. **Nested arrays, tuples, macros**: `dbg!`, `println!` with more than one argument, hygiene
   (`grep -rn hygien src/ --include='*.rs'` returns nothing), and `macro_rules!` refused as N3
   requires (N3-14). `vec![e]` stays a 1-element array with a misleading name until M7 has a `Vec`
   for it to mean.
8. **Gate integrity** (GI-06, GI-08, GI-09): `make gates` starts running `test-honest`; the
   milestone-exit target and its self-test ship *before* any milestone depends on them.

**Exit**: `make m2-exit`.

## M3 — Effects gate something (v0.5.0) · differentiator 1, core

**Waits on**: M2 item 5 only. **Delivers**: **C2**, which M4 then reuses.

Everything in N7 except parallel execution is reachable here. It needs no reference type, no
traits, no generics and no standard library — the builtin registry already classifies every builtin
(`src/builtins.rs:182`) and the analyser already unions effects across statements and calls. What is
missing is a fixed point, a traversal that sees methods, and a consumer.

**Owns 15 requirement rows**, the five `#[ignore]` rows tagged `unscheduled`, and the vacuous
`tests/09_effects_system.pd`, `tests/10_async_await.pd` and `tests/11_unsafe_blocks.pd` — the last
because [N7](../specification/language-spec.md#n7-effects-and-asynchrony) puts unsafe, IO, memory
and panic on one footing.

1. **Give the analysis a consumer** (N7-03, N7-08). Today the driver runs the analyser
   (`src/driver/mod.rs:147`) and prints the result (`src/driver/mod.rs:151-157`); nothing
   downstream reads it, so it cannot reject a program, change codegen or schedule anything.
2. **Make propagation a fixed point** (N7-04, N7-05, N7-06). It is a single forward pass whose
   fallback comment is "If function is unknown, we conservatively assume it's pure"
   (`src/effects/mod.rs:280-284`) — the unsound direction: a function defined below its caller
   contributes no effects to it.
3. **Analyse methods** (N7-07). The driver's loop matches only `crate::ast::Item::Function`
   (`src/driver/mod.rs:148-149`), so no method in an `impl` block is analysed at all.
4. **Delete `async` and `await` from the language** (N7-01, N7-02). They are keywords today — the
   two things N7 says the language does not have are the two the implementation has.
5. **Effect contexts** (N7-10, N7-11, N7-12): `with_timeout(5.seconds) { with_retry(3) { … } }`,
   `effect::sync { … }`, and `-> async T`. `with` and `effect` are not keywords today.
6. **Enforce N14's effect classification** (N14-05) and the unsafe effect (N12-07), which is what
   M4's `#![total(strict)]` will use to forbid `unsafe`.

**Exit**: `make m3-exit` — the positive fixtures (N7-04, N7-09, N7-10, N7-12), the negative ones (a
pure function calling an I/O builtin; an unresolved callee assumed pure), and the observables that
stdout cannot show (a callee defined below its caller still propagates; `impl` methods are
analysed; `effect::sync` serialises).

## M4 — Termination is provable, monomorphically (v0.6.0) · differentiator 2

**Waits on**: M2's attributes and patterns (**C3**), M3's propagation (**C2**). Not on references,
traits or generics.

**Owns 11 requirement rows.** It owns no `#[ignore]` row and no conformance row today, because the
feature is absent rather than broken and absence has no fixture — so its first task is to write
N8-01…N8-12's evidence ([F4](#f4-two-differentiators-owned-no-failing-row-anywhere)).

1. `#[total]`, `#![total(strict)]`, `#[decreases(expr)]`, `#[total(fuel = N)]`, `#[partial]`.
2. **Structural recursion on a monomorphic inductive type needs no measure** (N8-06). The generic
   case (N8-07) is M6's, and is the only part of N8 that waits for the type system.
3. `unsafe` is not permitted in a `#![total(strict)]` crate (N8-11) — M3's classification doing the
   work.
4. **There is no mode in which an unproven `#[total]` function is accepted** (N8-12), asserted as
   an observable rather than a rejection, because "no flag downgrades this" is a claim about the
   compiler's whole surface and cannot be shown by one refused program.

**Exit**: `make m4-exit`. Note the shape of N8's evidence: five rejection rows *and* six acceptance
rows. **A checker that refuses everything passes every rejection and fails N8-01…N8-06**, which is
why both halves are required.

## M5 — Reference typing, lifetimes and the memory model (v0.7.0) · differentiator 3

**Waits on**: nothing but itself. It is scheduled here because M6 and M7 need **C1**, not because
anything before it does.

**Owns 19 requirement rows** and two of the owner's four decisions (**D3**, **D4**).

1. **A real reference type** (N4-13) — `Type::Reference` maps to its inner type today
   (`src/typeck/mod.rs:121-125`). **Spelled `ref` / `ref mut` from the start**, per
   [N9](../specification/language-spec.md#n9-references-and-lifetimes); building it under `&` and
   renaming it later is two surface changes for one feature.
2. **Region inference** (N9-05, N9-06). There is none — `grep -rn 'region\|Region' src/
   --include='*.rs'` returns nothing. Inference failure is a compile error naming the ambiguity.
3. **Remove `'a` parameter lists** (N9-04) — **but keep `ref<'a> T`**, which
   [N9](../specification/language-spec.md#n9-references-and-lifetimes) explicitly permits where
   inference cannot resolve, and which N9-03 requires to be **accepted**. An earlier draft's exit
   was a grep for no `'a` anywhere in the corpus; that would have rejected conforming programs and
   driven the implementation toward a narrower language than the normative text. The receipt is two
   parser-level tests — `fn f<'a>(…)` refused, `ref<'a> T` accepted — not a grep.
4. **N12 becomes true of the implementation** (N12-03…N12-06): drop glue, per-value deallocation,
   `String` with move semantics, and `ref mut` of a non-`mut` binding refused for every referent
   type. The annex records the deviation today rather than the specification adopting it
   ([A9.1](../specification/language-spec.md#a91-string-is-a-copyable-handle-decision-2026-08-21)).
5. **Two owner decisions close here**: **D4** (array parameters) and **D3** (`str` and `usize`).
   Both must be answered before the reference type is finished — the array rule that lives in
   codegen today moves into the type system (N12-09), and it cannot move until it is known which
   rule it is.

**Exit**: `make m5-exit` — nine N9 rows, seven N12 rows, and the two decisions recorded.

## M6 — Traits, generics and modules (v0.8.0)

**Waits on**: C1. These three are one milestone because they have one consumer, the standard
library, and nothing before M7 needs any of them.

**Owns 32 requirement rows** — the largest milestone — plus the 18 `#[ignore]` rows tagged M4 in
the old numbering, and four conformance rows: the vacuous `07_traits_basic`, `08_generics_basic`
and `12_modules_imports`, and the `xfail` at `tests/conformance-manifest.txt:91`.

1. **Generics that work** (N10-01…N10-05, N4-15, N4-21). Inside `<…>` any all-uppercase name is
   reclassified as a *const* generic argument, so `Foo<T>` does not mean what it looks like; generic
   struct fields are rejected in codegen; const generics are not monomorphised.
2. **Traits with real dispatch** (N10-06…N10-10). They parse and emit nothing, trait method bodies
   are never typechecked, and a `self` receiver in a trait method is a parse error. Design:
   [`trait_system_design.md`](../design/trait_system_design.md), [`generics.md`](../design/generics.md).
3. **`Option<T>` and `Result<T, E>` as generic types with methods** (N4-16), and `?` lowering onto
   the representation enums actually get (N4-18, N4-19). **Their prelude shipping is N4-17 and
   belongs to M7** — representation and generic-enum support are what M6 buys; being in scope with
   no import is a library-packaging property, and one milestone should not claim both.
4. **Modules** (N11-01…N11-07): a `mod` item, file-based nesting, enforced visibility, and all four
   import forms.
5. **Closures, function types, slices** (N5-08, N4-14, N4-11, N6-06), and generic structural
   recursion (N8-07), which is the half of N8 that waited for this milestone.

**Exit**: `make m6-exit`, including N10-09 as an observable — a bounded call must emit no vtable,
because "abstraction costs nothing at runtime" is a claim about the generated code that no
fixture's stdout can show.

## M7 — The standard library (v0.9.0)

**Waits on**: M6 and C1. [N14](../specification/language-spec.md#n14-builtins-and-the-standard-library)
requires a standard library, so 1.0 does.

**Owns 12 requirement rows.**

What the library needs, stated as features rather than inferred from compile errors: generic ADTs
with methods and trait bounds (M6), associated types for an iterator protocol (M6), drop glue and
move semantics so a `Vec<T>` can own its buffer (M5), modules for more than one file (M6), and
`Result`-returning I/O signatures (N14-03).

`make stdlib-gate`'s per-file blocker column is **a lower bound, and it is not that dependency
list**. The manifest says so itself: the blocker is the *first* construct `pdc` rejects, and a
lexer-level blocker masks every parser-level blocker behind it — `stdlib/prelude.pd` is recorded as
`ATTRIBUTE` while also containing 18 `use` and 2 `mod` declarations. So the counts (8 `USE_DECL`,
3 `ATTRIBUTE`, 3 `PUB_FN_IN_IMPL`, 2 `MOD_DECL`, and one each of `UNINIT_LET`, `FLOAT_LITERAL`,
`CHAR_ESCAPE`, `ASSOC_TYPE`, `GENERIC_DEFAULT`) support exactly one claim — **every one of the 21
files is blocked on at least one earlier milestone** — and not the stronger claim that M7 is the
earliest correct start. That comes from the feature list above.

1. **Core, collections, math, string, I/O** (N14-09…N14-16), and the prelude (N4-17).
2. **Ship it** (N14-06, N14-07, N14-08). `make stdlib-gate` is **green right now with 0 of 21 files
   compiling** — it pins a measurement, it does not require a working library — so this milestone's
   evidence is not that gate. It is every file reaching `ACCEPTED_NO_MAIN` in `stdlib/MANIFEST.tsv`,
   plus an observable that `import std::…` resolves with no environment variable set, plus an
   observable that both Homebrew formulae install the tree. Neither does today, and
   `grep -rn stdlib .github/` returns nothing.

**Exit**: `make m7-exit`.

## M8 — Parallel by default (v0.10.0) · differentiator 1, completed

**Waits on**: C1 and C2 — and on **decision D2**, without which this milestone cannot be specified,
let alone built.

**Owns 6 requirement rows, two of which are the decision itself** (N7-14, N7-15). N7-13, N7-16 and
N7-17 cannot be written as tests until D2 answers what an implementation may emit and what the
observable semantics are.

**Do not start this milestone before D2 is answered.** If D2 comes back "no compiler-generated
concurrency", this milestone does not exist: parallel-by-default and automatic parallelization
leave N7 and the feature list, differentiator 1 is complete at M3, and 1.0 arrives a milestone
sooner.

**Exit**: `make m8-exit`, whose core is an observable — two independent effectful operations must
overlap in wall clock — plus the cancellation and error semantics D2 defines, plus an observable
that effect tracking left no representation in the generated C.

## M9 — The Rust compiler becomes redundant (v0.11.0)

**Waits on**: everything. The bootstrap compiler must accept the 1.0 language, so it goes last;
`make selfhost` stays green throughout as a floor.

**Owns 7 requirement rows** (SH-02…SH-04, N1-01…N1-03, GI-10).

1. **`make selfhost-corpus`**, which does not exist. Today's `make selfhost` proves
   `bootstrap/pdc.pd` compiles **itself** to a byte-identical fixed point over PBS-1 — a real
   result, and not the claim 1.0 makes. The 1.0 claim is that it compiles **the language**: every
   fixture in the corpus, with output matching the Rust compiler's on acceptances *and* refusals
   (SH-02, SH-03). Matching only on acceptances is satisfiable by a compiler that refuses nothing,
   which is why [F3](#f3-the-conformance-corpus-has-no-negative-tests) has to be closed first.
2. **PBS-1 is the whole language** (SH-04), not a subset with a written justification.
3. Retire `src/` as the primary compiler.

**Exit**: `make m9-exit`.

## 1.0.0 — the first release that is not a prerelease

Not a milestone with new features: the one where every earlier milestone is proven to have
finished. `make v1-exit`, which is:

1. **The whole requirement manifest** — 178 rows, none `owed`, none `blocked`, with both
   reconciliations holding. This is the criterion; the rest are what it rests on.
2. `make gates` exits 0, with `vacuous=0`, `xfail=0` and `untranscribed=0` on the conformance line,
   and `reject` non-empty.
3. `make test-honest` exits 0. **`make gates` does not run it today** (`Makefile:303-305` versus
   `Makefile:277-282`), so a non-ignored compiler regression can coexist with a green gate. GI-06
   puts it in `gates` at M2, and `v1-exit` runs it regardless.
4. `make test-xfail` reports `xfail=0`.
5. `make selfhost-corpus` exits 0 — the bootstrap compiler compiles the 1.0 language, not a subset
   of itself.
6. Every OPEN block in Part I is closed: D3 and D4 have answers, not options.

## Decisions for the owner

Four decisions block 9 requirement rows. Each is stated with both options and what each costs,
because the manifest cannot be finished until they are answered.

### D1 — Is 1.0 gated by Part I, or by the feature list as written?

Fifteen of 48 `feature-index.toml` rows name
[`PALLADIUM_V1_FEATURES.md`](../reference/features/PALLADIUM_V1_FEATURES.md) as their `spec`
because Part I has no section for them: refinement types, proof export to Lean/Coq, side-channel
safety, incremental compilation, parallel compilation, Rust FFI, C FFI, WASM, a debugger, a
formatter, the LSP server, Cargo compatibility, a package registry — and two standard-library rows
that N14 *does* require despite where the index points them. Twelve of the fifteen are
`unimplemented`.

- **Option A — Part I gates 1.0.** The feature list is amended to say so and to name the ecosystem
  subset that ships alongside (`pdc`, `pls`, the standard library and C FFI already have `partial`
  or `implemented` rows, so they are the candidates).
  *Cost*: someone writes that amendment, and nine ecosystem features become explicitly post-1.0 —
  a written deferral rather than a silent one. *Benefit*: 1.0 is the 178 enumerated rows, and it is
  reachable.
- **Option B — the feature list gates 1.0 as written.** 1.0 additionally requires a proof exporter
  to Lean/Coq, refinement types, a constant-time-code checker against the threat model the feature
  list states, incremental and parallel compilation, Rust FFI, a WASM target, a debugger, a
  formatter and a package registry.
  *Cost*: none of the nine has a design document in this repository, so the first cost is not
  implementation but definition — the manifest cannot gain their rows until each is specified
  precisely enough to write an exit criterion against. 1.0 moves out by an amount nobody can
  currently estimate, which is itself the argument for answering this now rather than at M9.

**This plan assumes Option A and says so.** `1.0-requirements.tsv` is written to Part I, and
`D1-01` stays `blocked` until the owner rules.

### D2 — May the compiler emit an unnamed execution substrate?

**This is not the contradiction an earlier draft of this file reported.** Static effect tracking can
compile into generated concurrency primitives with no `Future` boxing and no programmer-visible
executor, and N7's "no runtime representation" is satisfied by that. The question N7 does not
answer is narrower and sharper: **may the compiler emit threads, an event loop, scheduling state,
cancellation and joins that the programmer never names?** Parallel-by-default requires *some*
execution mechanism, and the C backend is not the obstacle — `runtime/palladium_runtime.c` already
ships, and pthreads and kqueue are C.

- **Option A — permitted, unnamed.** N7 gains a sentence saying the compiler may emit an execution
  substrate the program cannot name or configure.
  *Cost*: N7 must then define three things it currently does not, because "parallel by default" is
  otherwise unimplementable to a test:
  **(i) sequencing** — what ordering is guaranteed between independent effectful operations, and
  what `effect::sync` guarantees beyond it;
  **(ii) cancellation** — when one branch fails or a `with_timeout` fires, are siblings cancelled,
  at which points, and are effects already performed observable (they are, and that has to be
  said);
  **(iii) errors** — if two branches fail, which error propagates: deterministically by source
  order, or nondeterministically by first-to-fail.
  Until those are written, N7-13, N7-15 and N7-17 cannot be specified and M8 has no exit.
- **Option B — not permitted.** No compiler-generated concurrency.
  *Cost*: "independent effectful operations are parallel by default" and "automatic
  parallelization" leave N7 and the feature list. Differentiator 1 becomes "no function colouring,
  effects inferred and gated" — still a real differentiator, and M3 delivers all of it.
  *Benefit*: M8 disappears, 1.0 arrives a milestone sooner, and the effect system stays a pure
  compile-time analysis with no scheduling semantics to specify or to reconcile with
  `#![total(strict)]`.

Either way **M3 is unaffected**: inference, gating, effect contexts and the removal of
`async`/`await` need no substrate.

### D3 — `str` and `usize`

[N4](../specification/language-spec.md#n4-types) lists the primitives and neither is among them,
while [`implicit-lifetimes.md`](../reference/features/core-language/implicit-lifetimes.md) writes
`ref str` and `position: usize` in normative examples, and N14 gives `string_len` an `i64` return.

- **Option A — add both primitives.** *Cost*: two types through lexer, parser, checker and codegen,
  and `str` needs a borrowed-string representation, so it depends on C1 and interacts with D4.
  *Benefit*: `ref str` is the natural referent of `ref`, and the differentiator's headline example
  keeps working as written.
- **Option B — rewrite the sites** to `ref String` and `u64`. *Cost*: the feature document's
  headline example changes and lengths stay signed. *Benefit*: no new primitives.

Blocks N4-08 and the exact spelling of M5's fixtures.

### D4 — Array parameters: value semantics or reference semantics

Stated in full at [N12.1](../specification/language-spec.md#n121-array-parameters-open-decision),
with measured consequences at [A9.2](../specification/language-spec.md#a92-array-parameters).
**Option A** makes `[T; N]` a value type, so the three spellings mean three different things and
every array argument is a memcpy unless the author writes a reference. **Option B** makes it alias
the caller's storage, matching C, so the reference spellings are redundant and the specification
must say which one is required for a parameter written through.

Blocks N12-08 and N12-09. Until it is answered the rule stays in code generation, where the type
system cannot enforce it, and `sum2(v, v)`'s current rejection is correct only under Option B.

## Findings

Measured while re-deriving this file. Reported, not worked around.

### F1. Parallel-by-default needs an execution substrate — reframed as decision D2

An earlier draft of this file reported N7 as self-contradictory. **It is not**, and the correction
is recorded here rather than quietly dropped: static effect tracking is compatible with generated
concurrency primitives, so "no runtime representation" and "parallel by default" can both hold.
What N7 genuinely does not say is whether an unnamed execution substrate is permitted, and what the
sequencing, cancellation and error semantics of implicit parallelism are. That is
[D2](#d2--may-the-compiler-emit-an-unnamed-execution-substrate), and it blocks M8 — not M3.

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

The first reproduces at `2ef170f`: `fib(10)` prints `8261746944` and exits 0. **A silent miscompile
shipped in the release named for removing silent miscompiles.**

Reading both inventories fixes the omission but not the class: owners are editable, and the Rust
inventory is whatever ignored tests `cargo` lists, so **deleting a test silently shrinks it**. That
is why the exit criterion is a closed requirement manifest and not a pair of filters — see
[How a milestone exits](#how-a-milestone-exits).

### F3. The conformance corpus has no negative tests

`reject=0` on every run. The class exists, the runner implements it, and
[A11](../specification/language-spec.md#a11-conformance) advertises it as "how 'the compiler rejects
`.await`' gets tested instead of a program that prints prose about async being unimplemented" — and
no fixture uses it. The refusals are covered, but in Rust integration tests
(`tests/d5_unimplemented_constructs.rs`, `tests/d10_llvm_refuses.rs`), which the bootstrap compiler
will never run. 23 rows of the requirement manifest are `reject` rows; M9's parity claim depends on
them existing.

### F4. Two differentiators owned no failing row anywhere

Implicit lifetimes: zero conformance rows, zero `#[ignore]` rows. Totality: zero and zero. Nothing
in this repository currently fails because lifetimes are not inferred or termination is not proven —
the features are absent rather than broken, and absence has no fixture. Under the old
owner-filter exits, `make m5-exit` and `make m8-exit` would have exited 0 the day they were added.
The requirement manifest is the repair: M4 owns 11 rows and M5 owns 19, all `owed`, so neither can
go green before its evidence exists. Reviewers established that the same hole applies to **all
eight** milestones, not to these two — an owner filter clears tagged proxies, never a feature
contract.

### F5. Two documents define 1.0 — now decision D1

See [D1](#d1--is-10-gated-by-part-i-or-by-the-feature-list-as-written). The LLVM backend is the
sharpest instance: [N1](../specification/language-spec.md#n1-overview-and-design-commitments) calls
a second backend "a second implementation of the same definition" — explicitly not a language
property — while the feature list has it under "6. Compilation & Optimization" as a 1.0 feature.

### F6. A second roadmap in the tree was written in the fictional present

[`docs/design/vision-roadmap.md`](../design/vision-roadmap.md) opened "Palladium α v0.7 has achieved
what many thought impossible", carried a benchmark table against Rust 1.74 for a compiler that could
not link a hello-world when it was written, and scheduled "Q4 2026: Language Freeze" — assigning
v0.7 to a past that never happened while this file assigns it to M5. Its body is replaced with a
supersession pointer in this change.

### F7. Stale claims in documents this file rests on

- [A11](../specification/language-spec.md#a11-conformance) said "over 44 fixtures", "verified 33 ·
  vacuous 7 · xfail 2 · skip 2" and named "the three failures", against a measured
  `verified=43 … xfail=1 … failures=0` over 53. **Corrected in this change**, because this file
  treats the annex as a 1.0 authority and stale data in an authority is release-governance drift,
  not a documentation nit.
- `feature-index.toml`'s `async_as_effect` row carries
  `cmd: grep -rn 'effects::' src/ --include='*.rs' | grep -v '^src/effects' -> 1 line,
  src/driver/mod.rs:147`. Re-run, that command returns **8 lines** — seven in `src/builtins.rs`
  plus the driver. The load-bearing half of the claim holds (the analysis has exactly one consumer,
  a `println!`); the count does not. **Not corrected here**: it is a `cmd:` evidence item, and
  `check-doc-evidence.sh` validates the *form* of one but never runs it — so this class of rot is
  invisible to the gate that exists to catch it. Reported as its own defect.
- The previous version of this file reported 2 unit-test failures and 43 integration failures.
  `make test-honest` exits 0.

### F8. One declared failure expects syntax the specification forbids

`tests/advanced_features_test.rs:340` is an `xfail` whose reason is that `macro_rules! vec { … }`
"is not an item". Under [N3](../specification/language-spec.md#n3-program-structure-and-items) it
must never be one, and `scripts/check-doc-evidence.sh` already fails any normative document that
writes `macro_rules!`. A row that stays red unless the language changes is not a debt: it is a
negative test wearing the wrong class. N3-14 makes it a normal, passing `reject` fixture owned by
M2.

### F9. The milestone labels in the test suite were written against the old numbering

The `(owned by M<n>)` tags predate this file, and this re-derivation moves more of them than the
first draft did. Against the new sequence: the 14 tagged M2 stay **M2**; the 18 tagged M4 become
**M6**; the 3 tagged M1 become **M2**; the 5 tagged `unscheduled` become **M3**; the 1 tagged M5
leaves the inventory as a passing negative test (F8). That is **27 re-tags and one
reclassification**, and they must land **atomically, before any owner-filtering target ships**,
together with the self-test that proves the filter detects a planted row (requirement GI-09).
Shipping the filter first would mean a milestone whose exit reads labels that mean something else.

Each reason should also gain a `req: <id>` tag, which is what turns the Rust half of the manifest's
reconciliation into a command instead of a review. All of it is an edit to `tests/`, outside this
document's scope.

### F10. Block comments do not nest, and nothing said so

[N2](../specification/language-spec.md#n2-lexical-structure) requires `/* … */` to nest.
`fn main() { /* a /* b */ c */ print("ok"); }` fails with `Expected expression, but found '/'`.
[A2](../specification/language-spec.md#a2-lexical-structure) records comments as implemented without
qualifying nesting, so this is a normative requirement with no annex row and no test — found by
enumerating N2, not by any gate. It is N2-08, owned by M2, and it is the kind of gap enumeration
exists to surface.

## Keeping this file honest

Every claim above is reproducible:

```bash
make gates          # conformance + gate self-test + docs + doc-evidence + selfhost + stdlib + probe
make test-honest    # every test binary, integration tests included
make test-xfail     # every declared failure, and the milestone that owes it
```

Four rules this file is held to:

1. **If a milestone's exit criterion cannot be written as a command, it is not an exit criterion.**
2. **The exit command covers the goal, not a proxy for it.** Positive evidence as well as negative,
   a runtime observable wherever the goal is a runtime property, packaging wherever the goal is a
   shipped artifact. Clearing every red row tagged with a milestone proves only that nobody wrote a
   red row for the part that is missing.
3. **A milestone owns rows in a closed inventory.** [`1.0-requirements.tsv`](1.0-requirements.tsv)
   is what 1.0 means. Deleting a row, or moving it between milestones, is a contract transition and
   is reviewed as one; it is not a side effect of deleting a test.
4. **Paying off a row is a transition, not a deletion.** The fixture stays on disk and its row
   becomes `run` with a transcript, in the same commit.

Every `file:line` on this page is fingerprinted by `make check-doc-evidence`, so a citation that
stops pointing at what it names fails a gate instead of rotting. What that gate cannot do is check
a `cmd:` item's *output* — see [F7](#f7-stale-claims-in-documents-this-file-rests-on).
