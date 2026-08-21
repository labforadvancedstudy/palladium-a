# Milestones

**Updated**: 2026-08-22 · **Released**: v0.3.0 (M1) · **Target**: 1.0.0

Ordered by what unblocks what, not by theme. Every milestone exits on a command, not on an
opinion, and every milestone ships.

## The shape of the plan

**1.0 is the language [Part I of the specification](../specification/language-spec.md#part-i-normative-specification)
defines** — N1 through N14 — with [the annex](../specification/language-spec.md#part-ii-implementation-status-annex)
reporting no `partial` and no `unimplemented` row against any of them. Today it reports one
`implemented`, nine `partial` and four `unimplemented`.

Three of those four unimplemented sections are the reasons for this language to exist rather than
to be a Rust dialect, and the
[feature list](../reference/features/PALLADIUM_V1_FEATURES.md#unique-advantages-over-rust) leads
with them:

| # | Differentiator | Normative | Definition |
|---|---|---|---|
| 1 | Asynchrony is an effect — no `async`, no await operator, no colouring | [N7](../specification/language-spec.md#n7-effects-and-asynchrony) | [`async-as-effect.md`](../reference/features/async-system/async-as-effect.md) |
| 2 | Termination is provable — `#![total(strict)]`, `#[decreases(expr)]` | [N8](../specification/language-spec.md#n8-totality) | [`totality-checking.md`](../reference/features/advanced/totality-checking.md) |
| 3 | Lifetimes are inferred — `ref` / `ref mut`, no `'a` | [N9](../specification/language-spec.md#n9-references-and-lifetimes) | [`implicit-lifetimes.md`](../reference/features/core-language/implicit-lifetimes.md) |

**None of the three can be built on the type system that exists today, and they all fail on the
same missing thing.** The type checker has no reference type: `Type::Reference` is mapped to its
inner type, so `&i64` and `i64` are indistinguishable to it
([A5](../specification/language-spec.md#a5-types), `src/typeck/mod.rs:121-125`). Without a
reference type there is nothing for a region to be inferred *over* (differentiator 3), nothing to
carry a mutation capability so that two effectful operations can be shown independent
(differentiator 1), and no inductive type surviving codegen to recurse structurally on
(differentiator 2). The annex says this in its own words: the array-parameter rule lives in code
generation "because there is no reference type in the type checker to carry the permission"
([A9.2](../specification/language-spec.md#a92-array-parameters)).

So the order is forced, and it is not the order of the differentiator list:

| Milestone | Version | What it is | Why it is here |
|---|---|---|---|
| M1 ✅ | v0.3.0 | The compiler stops lying | Done. Silent wrongness became diagnostics, and the gates became able to fail |
| M2 | v0.4.0 | The surface stops fighting you | 17 declared failures are lexer/parser gaps; and M1 shipped three of its own unpaid |
| M3 | v0.5.0 | Modules, and the Rust compiler becomes redundant | A standard library and a multi-file bootstrap compiler both need `mod`; parity before abstraction means abstraction is implemented once |
| M4 | v0.6.0 | The type system becomes real | References, generics, traits. **A prerequisite milestone, not a differentiator** — all three differentiators block on it |
| M5 | v0.7.0 | Lifetimes disappear · **differentiator 3** | First thing buildable on M4, and it closes the memory model N12 defines |
| M6 | v0.8.0 | A standard library | The first real consumer of M4 and M5; both remaining differentiators need types to be total or effectful *about* |
| M7 | v0.9.0 | Effects gate something · **differentiator 1** | Needs M4 (effects on signatures), M5 (independence needs aliasing), M6 (an I/O surface worth inferring) |
| M8 | v0.10.0 | Termination is provable · **differentiator 2** | Reuses M7's call-graph fixed point: totality propagates to callers exactly as an effect does |
| — | **1.0.0** | The first release that is not a prerelease | |

**Versioning.** M1 shipped as `v0.3.0`. Every milestone from here ships the same way: one release
per milestone, and every `0.x` is a prerelease in the semver sense — the language may still change
under you. **1.0.0 is the first non-prerelease**, and what it promises is not "feature complete" in
the abstract but the specific thing below under [1.0.0](#100--the-first-release-that-is-not-a-prerelease):
Part I implemented, no fixture that proves nothing, no declared failure left.

## Where the project actually is

Measured at `2ef170f`, not read from the previous version of this file.

| | | Command |
|---|---|---|
| Self-hosting | fixed point — stage1 and stage2 C are byte-identical (`9b0cf24e…`) | `make selfhost` |
| Conformance | `verified=43 untranscribed=0 vacuous=7 xfail=1 reject=0 skip=2 failures=0` over 53 fixtures | `make conformance` |
| Conformance gate itself | 96 cases, each pinning a way it must still go RED | `make test-conformance-runner` |
| Documentation | every snippet compiles; 224 citations fingerprinted, 28 no-compile fences pinned, 48 feature rows all with tagged evidence | `make check-docs` |
| Rust tests | 620 pass, **0 fail**, 42 ignored (524 lib + 96 integration) | `make test-honest` |
| Declared failures | 41 `xfail` + 1 `slow`, each naming the milestone that owes it, none of them passing | `make test-xfail` |
| `stdlib/` | 0 of 21 files compile; 6 drivers inventoried; 38 builtins accounted against a normative 34 | `make stdlib-gate` |
| Traits · generics · effects · async · unsafe · modules | conformance coverage is **zero** for each | `make conformance` (vacuous rows) |

Two lines in the previous version of this file are retracted by that run: "Unit tests 404 pass, 2
pre-existing failures" and "Integration tests 43 fail, all pre-existing". `make test-honest` exits
0 at `2ef170f`. The failures did not disappear — M1 converted them into 41 declared `xfail`s, each
naming the missing feature *and* the milestone that owes it, which is the inventory this plan is
built on.

## The five inventories that are the work remaining

A milestone plan that does not account for all five is incomplete.

**1. Part I, by section** — the annex's own summary table:
`sed -n '/^| Normative section | Status/,/^$/p' docs/specification/language-spec.md | awk -F'|' 'NR>2{print $3}' | sort | uniq -c`

| Status | Count | Sections |
|---|---|---|
| implemented | 1 | N13 |
| partial | 9 | N1, N2, N3, N4, N5, N6, N11, N12, N14 |
| unimplemented | 4 | **N7, N8, N9**, N10 |

The three differentiators are three of the four `unimplemented` rows. The fourth, N10, is what
they need.

**2. Per feature** — `feature-index.toml` via `tomllib`: 48 rows, **4 implemented · 16 partial ·
28 unimplemented**. Fifteen of the 48 name
[`PALLADIUM_V1_FEATURES.md`](../reference/features/PALLADIUM_V1_FEATURES.md) as their `spec`
rather than a Part I section — see [What 1.0 does not
say](#what-10-does-not-say-and-the-owner-has-to).

**3. Conformance debt, by owner** — `make conformance`, tail of the run

| Owner | xfail | vacuous | Rows |
|---|---|---|---|
| M3 | 1 | 1 | `tests/projects/hello_pdm/tests/test_math.pd` · `tests/12_modules_imports.pd` |
| M4 | 0 | 2 | `tests/07_traits_basic.pd` · `tests/08_generics_basic.pd` |
| unscheduled | 0 | 4 | `tests/02_types_enums.pd` · `tests/09_effects_system.pd` · `tests/10_async_await.pd` · `tests/11_unsafe_blocks.pd` |

**Nothing is owed to M1 or M2 here, and `reject` is empty.** This plan gives every `unscheduled`
row an owner below; after M2 no row in `tests/conformance-manifest.txt` says `unscheduled`.

**4. Declared Rust failures, by owner** — `make test-xfail`; owners parsed from the `#[ignore]`
reason by `scripts/test-xfail.py:74`

| Owner | Count | Shape of the debt |
|---|---|---|
| M4 | 18 | generics, traits, closures, function types, const generics, `?` |
| M2 | 14 | `else if`, `loop`, `+=`, bitwise, `as`, literal patterns, method calls, tuples, `const`/`static`, inline `mod` |
| M7 (was `unscheduled`) | 5 | `async fn` wrapped in `Future`; `effect` is not an item |
| **M1** | **3** | still owed by a milestone that shipped — see [F2](#f2-m1-shipped-three-of-its-own-declared-failures-and-its-exit-command-cannot-see-them) |
| M5 | 1 | `macro_rules!` — **owed to nobody**: it expects syntax N3 forbids, so it is a negative test mis-declared as a debt ([F8](#f8-one-declared-failure-expects-syntax-the-specification-forbids)) |

**5. Open defects** — [`CLAUDE.md`](../../CLAUDE.md) "남은 결함" and the annex

| Defect | Where | Owned by |
|---|---|---|
| D3b — a tail `if` is not lowered to a `return`; `fib(10)` prints `8261746944` and exits 0 | [A6.6](../specification/language-spec.md#a66-tail-expressions) | M2 |
| C-keyword identifiers — `fn double` emits `long long double(…)`; gcc rejects the compiler's own output after it prints "Compilation successful" | `tests/e2e_test.rs:269` | M2 |
| No missing-return diagnostic — `fn f() -> int { }` compiles silently | `tests/compiler_comprehensive_test.rs:567` | M2 |
| `a * -b` does not parse (multiplication takes its right operand from postfix) | [A6.3](../specification/language-spec.md#a63-expression-forms) | M2 |
| Nested arrays `[[T; M]; N]` work in neither locals nor parameters | [A5](../specification/language-spec.md#a5-types) | M2 |
| Six builtins that cannot compile — the handle representation split in two | [A8](../specification/language-spec.md#a8-builtins) | M2 (delete/re-base), M6 (signatures) |
| `pub` on an enum is parsed and discarded; `dbg!` calls a `print_debug` defined nowhere; `println!` takes exactly one argument; macros are not hygienic | [A4.3](../specification/language-spec.md#a43-enums), [A4.6](../specification/language-spec.md#a46-macros) | M2 |
| `Foo<T>` is parsed as a *const* generic argument; const generics are not monomorphised | [A5](../specification/language-spec.md#a5-types) | M4 |
| Traits emit no C; a trait method with a `self` receiver is a parse error | [A4.4](../specification/language-spec.md#a44-traits) | M4 |
| `Type::method(args)` lowers to a `Type_method__new` codegen never emits | [`CLAUDE.md`](../../CLAUDE.md) | M4 |
| `&mut` of an immutable local is accepted for struct referents | [A9.3](../specification/language-spec.md#a93-mut-of-an-immutable-local-is-accepted) | M5 |
| `String` is a Copy handle, contradicting N12's move semantics — no drop glue exists | [A9.1](../specification/language-spec.md#a91-string-is-a-copyable-handle-decision-2026-08-21) | M5 |
| **OPEN DECISION** — do `[T; N]` parameters copy or alias? | [N12.1](../specification/language-spec.md#n121-array-parameters-open-decision) | M5 |
| **OPEN** — `ref str` and `usize` are used normatively and are not primitives | [N4](../specification/language-spec.md#n4-types) | M5 |
| Effects gate nothing; propagation is a single forward pass that assumes unknown callees pure; `impl` methods are never analysed | [A4.1](../specification/language-spec.md#a41-functions), [async-as-effect §"Where the implementation currently diverges"](../reference/features/async-system/async-as-effect.md#where-the-implementation-currently-diverges) | M7 |
| Attributes do not lex — `#[total]` fails at the character `#` | [A2](../specification/language-spec.md#a2-lexical-structure) | M2 (the token), M8 (the checker) |
| The LLVM backend refuses unconditionally; 14 sites fabricated rather than lowered | [A1](../specification/language-spec.md#a1-pipeline-and-backends) | not scheduled — see [F5](#f5-two-documents-define-10-and-they-are-not-the-same-set) |

## How a milestone exits

`make m1-exit` was one line, because when it was written the only structured owner field was the
`owner` column in `tests/conformance-manifest.txt`:

```make
m1-exit: build
	@CONFORMANCE_FORBID_OWNER=M1 bash scripts/conformance.sh tests examples
```

That is `Makefile:270-271`, and it is why M1 shipped with three of its own declared failures still
red ([F2](#f2-m1-shipped-three-of-its-own-declared-failures-and-its-exit-command-cannot-see-them)).
There are **two** owner inventories, and the exit command read one of them. From M2 on, every exit
target reads both:

```make
m2-exit: build
	@CONFORMANCE_FORBID_OWNER=M2 bash scripts/conformance.sh tests examples
	@TEST_XFAIL_FORBID_OWNER=M2 python3 scripts/test-xfail.py
```

Three lines per milestone, and one change outside this document: `scripts/test-xfail.py` already
parses the owner out of every `#[ignore]` reason and rejects a reason without one
(`scripts/test-xfail.py:74`); it needs to read `TEST_XFAIL_FORBID_OWNER` and fail when a still-red
row names it. That is a filter on a field the script already has, and it is the durable fix for
the way M1 leaked — not a note in this file.

Neither check can be satisfied by a milestone that owns nothing (see
[F4](#f4-two-differentiators-own-no-failing-row-anywhere)), so each milestone below also states
what it must **add** to the corpus before it starts.

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
| Seven fixtures proved nothing while counting as coverage | Declared `vacuous`, each naming the feature it fails to cover. Seven of 53 prove nothing, which is the honest number and is now on the summary line of every run |
| The gates could not fail | `make test-conformance-runner` (96 cases), `make test-gate-probe` (every evidence producer fault-injected) |
| `tests/*.rs` never ran under `make test-rust` | `make test-honest`, and every remaining failure converted to a declared `xfail` with an owner |

Not paid, and re-owned by M2: three M1 `#[ignore]` rows
([F2](#f2-m1-shipped-three-of-its-own-declared-failures-and-its-exit-command-cannot-see-them)).

---

## M2 — The surface stops fighting you (v0.4.0)

**Why here**: seventeen declared failures live here — fourteen are one-line lexer or parser gaps,
the kind that make people give up on the first afternoon, and three are M1's unpaid debt.
Everything later is written *in* this surface, including the bootstrap compiler M3 has to grow, so
widening it first is cheaper than widening it later.

**Owns**: 14 `#[ignore]` rows owned by M2, **plus the 3 owned by M1**, plus
`tests/02_types_enums.pd`, whose row says `unscheduled` while enums are
[implemented](../specification/language-spec.md#a43-enums) — the fixture is payable today by being
rewritten to declare one.

1. **The M1 debt first, because it is a live miscompile.** A tail `if` is not lowered to a return:
   `fn fib(n: i64) -> i64 { if n <= 1 { n } else { … } }` compiles clean and `fib(10)` prints
   `8261746944`. The diagnostic and the lowering land together, as
   `tests/compiler_comprehensive_test.rs:567` already says they must. With them, C-keyword
   identifier mangling.
2. **Statements and expressions**, per [N5](../specification/language-spec.md#n5-statements-and-expressions):
   `if` and `match` become expressions, `else if`, `loop`, compound assignment, bitwise operators,
   `as` casts, `a * -b`, top-level `const` and `static`.
3. **Patterns**, per [N6](../specification/language-spec.md#n6-patterns): literal, range, or-,
   tuple patterns and guards. Today `match` cannot dispatch on an integer, which forces an
   `if`/`else` chain wherever a state machine would be natural — and a compiler is state machines.
4. **Method call syntax** `x.f()`, rejected today with "Indirect function calls not yet supported".
   Every `impl` block is unreachable from source without it.
5. **Lexical completion**, per [N2](../specification/language-spec.md#n2-lexical-structure): float,
   char and hex literals, and **the `#` attribute token**. The token only — no totality checker
   (M8). An attribute that lexes and is then ignored would recreate exactly the class M1 removed,
   so an unknown attribute must be a compile error from the day `#` lexes.
6. **The six builtins that cannot compile.** [N14](../specification/language-spec.md#n14-builtins-and-the-standard-library)
   already decides half of it: the four `*_ex` names are not part of the language, so they leave
   `BUILTINS`. `file_flush` and `file_seek` are normative and get re-based on the index handle.
   Signature alignment with N14's `Result` waits for M6.
7. **Nested arrays**, `pub` on an enum, and the macro defects: `dbg!` calls a `print_debug` defined
   nowhere, `println!` accepts exactly one argument, and nothing is hygienic
   (`grep -rn hygien src/ --include='*.rs'` returns nothing) although
   [N3](../specification/language-spec.md#n3-program-structure-and-items) says the one macro system
   is hygienic by default. `vec![e]` stays a 1-element array with a misleading name until M6 has a
   `Vec` for it to mean.

**Exit**: `make m2-exit`, plus the transitions those rows require — an `xfail` is paid off by
becoming `run` with a transcript in the same commit, never by deleting the row.

## M3 — Modules, and the Rust compiler becomes redundant (v0.5.0)

**Why here**: this is the project's thesis, and it is reachable only once M2 has widened the subset
enough to write a compiler comfortably. Modules come with it rather than before it because both
things that need `mod` — a standard library of more than one file, and a bootstrap compiler of more
than one file — arrive here.

**Owns**: both M3 conformance rows — the `xfail` at `tests/conformance-manifest.txt:91` (cross-file
imports: "Undefined function: add") and the vacuous `tests/12_modules_imports.pd`. The inline-`mod`
row `tests/advanced_features_test.rs:411` is labelled M2 and is the same feature; reassign it here.

1. **Modules**, per [N11](../specification/language-spec.md#n11-modules): a `mod` item, file-based
   nesting, visibility, and imports that may follow an item instead of all preceding it.
2. **Grow `bootstrap/pdc.pd` and PBS-1 together**, one construct at a time, holding `make selfhost`
   green at every step. Rule PBS-0 is what keeps it honest: a construct enters the subset only when
   the bootstrap compiler both *accepts* and *implements* it. Violating that rule is exactly how
   `bootstrap/v2_full_compiler` became permanently uncompilable
   ([`bootstrap-subset.md`](../specification/bootstrap-subset.md)).
3. **Diff the two compilers over the whole corpus**, then retire `src/` as the primary compiler.

**Exit**: `make m3-exit`, and `bootstrap/pdc.pd` compiles every fixture in the corpus with output
matching the Rust compiler's. That second criterion is **not sufficient as the corpus stands**:
`reject=0`, so a compiler that refuses nothing satisfies it
([F3](#f3-the-conformance-corpus-has-no-negative-tests)). M3 adds a `reject` row for each refusal
the language depends on and requires both compilers to refuse it with the same diagnostic.

## M4 — The type system becomes real (v0.6.0)

**Why here**: this is a prerequisite milestone, not a differentiator, and it is the one every
differentiator waits on. References, generics and traits are one piece of work because trait bounds
are a generic feature and a `self` receiver is a reference.

**Owns**: all 18 `#[ignore]` rows owned by M4, and the vacuous `tests/07_traits_basic.pd` and
`tests/08_generics_basic.pd`.

1. **A real reference type.** `Type::Reference` currently maps to its inner type
   (`src/typeck/mod.rs:121-125`). It becomes a type the checker can tell apart, carrying mutability.
   **Spelled `ref` / `ref mut` from the start**, per
   [N9](../specification/language-spec.md#n9-references-and-lifetimes) — building it under `&` and
   renaming it later is two surface changes for one feature. `&` and `'a` stay accepted as
   deprecated spellings until M5 deletes them.
2. **Generics that work.** Monomorphisation is partial, and inside `<…>` any all-uppercase name is
   reclassified as a *const* generic argument, so `Foo<T>` does not mean what it looks like. Generic
   struct fields are rejected in codegen; const generics are not monomorphised at all.
3. **Traits with real dispatch.** They parse and emit nothing; there is no vtable mechanism
   anywhere, trait method bodies are never typechecked, and a `self` receiver in a trait method is a
   parse error. Design: [`trait_system_design.md`](../design/trait_system_design.md),
   [`generics.md`](../design/generics.md).
4. **`Option<T>` and `Result<T, E>` become ordinary library types with methods**, and `?` lowers
   onto the representation enums actually get instead of the fabricated `struct Result` layout M1
   deleted.
5. **Exhaustiveness for every scrutinee type**, not only enums, as
   [N6](../specification/language-spec.md#n6-patterns) requires — and a non-exhaustive `match` traps
   instead of falling through.

**Exit**: `make m4-exit`.

## M5 — Lifetimes disappear (v0.7.0) · differentiator 3

**Why here**: it is the first differentiator buildable on M4, it needs nothing from the other two,
and the memory model it completes is what M6's collections rest on. A `Vec<T>` that never frees is
not a collection.

**Owns**: nothing today, in either inventory
([F4](#f4-two-differentiators-own-no-failing-row-anywhere)). Its first task is to write the fixtures
it will pay off — one `run` row per inference rule in
[`implicit-lifetimes.md`](../reference/features/core-language/implicit-lifetimes.md#inference-rules),
and one `reject` row per ambiguity class, each pinned to the diagnostic that names the ambiguity.
A milestone that owns no red row exits green on day one.

1. **Region inference.** There is none — `grep -rn 'region\|Region' src/ --include='*.rs'` returns
   nothing. Inference failure is a compile error naming the ambiguity, never a guess.
2. **Delete `'a`.** `Function.lifetime_params` is parsed and read nowhere; the surface loses
   lifetime parameter lists, and `&`/`&mut` retire in favour of `ref`/`ref mut`.
3. **N12 becomes true of the implementation**: drop glue, per-value deallocation, `String` with move
   semantics. The annex records the deviation today rather than the specification adopting it
   ([A9.1](../specification/language-spec.md#a91-string-is-a-copyable-handle-decision-2026-08-21)) —
   this is where the deviation ends.
4. **Two open questions close here, and they are the owner's**:
   [N12.1](../specification/language-spec.md#n121-array-parameters-open-decision) (do `[T; N]`
   parameters copy or alias?) and [N4](../specification/language-spec.md#n4-types) (`str` and
   `usize` are used normatively and are not primitives — `ref str` names no type until one of them
   is chosen). Both must be answered before the reference type is finished, not after: the array
   rule that lives in codegen today moves into the type system, and it cannot move until it is known
   which rule it is.
5. `&mut` of an immutable local is accepted for struct referents
   ([A9.3](../specification/language-spec.md#a93-mut-of-an-immutable-local-is-accepted)).

**Exit**: `make m5-exit`, plus `grep` over the corpus finding no `'a` and no `&` in any fixture.

## M6 — A standard library (v0.8.0)

**Why here**: [N14](../specification/language-spec.md#n14-builtins-and-the-standard-library) defines
one, so 1.0 requires it — and it is where M4 and M5 get validated by a real user before two
differentiators are built on top of them. Both remaining differentiators need something to be
effectful or total *about*: an I/O surface richer than 34 builtins, and inductive types to recurse
structurally on.

**Owns**: the `make stdlib-gate` measurement — **0 of 21 files compile**, the tree is on no default
search path, it is in neither Homebrew formula, and `grep -rn stdlib .github/` returns nothing
([A8](../specification/language-spec.md#a8-builtins)).

The gate pins a blocker class per file, and that inventory is the ordering argument for this
milestone, not an opinion about it. From `make stdlib-gate`: 8 `USE_DECL` and 2 `MOD_DECL` (M3),
3 `ATTRIBUTE`, 3 `PUB_FN_IN_IMPL`, 1 `UNINIT_LET`, 1 `FLOAT_LITERAL`, 1 `CHAR_ESCAPE` (M2), 1
`ASSOC_TYPE` and 1 `GENERIC_DEFAULT` (M4). **Every one of the 21 files is blocked on a milestone
before this one**, and ten of them on the module system alone. Starting the standard library any
earlier means writing it against a language that cannot parse it.

1. **Core, collections, math, string, I/O**: `Vec<T>`, `HashMap<K, V>`, `String` methods,
   `Option<T>`/`Result<T, E>` methods, an iterator protocol expressed as a trait.
2. **Ship it.** Packaged by both formulae, on the default search path, and every file pinned by
   `make stdlib-gate` transitions from a compile verdict to a conformance row.
3. **Signature alignment with N14**: the filesystem builtins return `Result`, and `string_char_at`
   returns `char` — both blocked until now on types that did not exist.

**Exit**: `make m6-exit`, and `make stdlib-gate` reports 21 of 21 compiling with the drivers
transitioned to `run` rows.

## M7 — Effects gate something (v0.9.0) · differentiator 1

**Why here**: it needs M4 (effects belong on function signatures, and there are no function types
today), M5 (deciding two operations are independent is an aliasing question), and M6 (an I/O
surface worth inferring over).

**Owns**: the 5 `#[ignore]` rows currently labelled `unscheduled` — they must be re-tagged `M7`
before `make m7-exit` can see them
([F9](#f9-the-milestone-labels-in-the-test-suite-were-written-against-the-old-numbering)) — and
three vacuous rows: `tests/09_effects_system.pd`, `tests/10_async_await.pd`, and
`tests/11_unsafe_blocks.pd`, because
[N7](../specification/language-spec.md#n7-effects-and-asynchrony) puts unsafe, IO, memory and panic
on the same footing as asynchrony.

1. **Give the analysis a consumer.** `crate::effects::` reaches the compiler proper in two places:
   the builtin registry annotates each builtin with its effects (`src/builtins.rs:182`), and the
   driver runs the analyser (`src/driver/mod.rs:147`) — and prints the result
   (`src/driver/mod.rs:151-157`). Nothing downstream reads it, so it cannot reject a program,
   change codegen, or schedule anything.
2. **Make propagation a fixed point over the call graph.** It is a single forward pass whose
   fallback comment is "If function is unknown, we conservatively assume it's pure"
   (`src/effects/mod.rs:280-284`). That is the unsound direction: a function defined below its
   caller contributes no effects to it. And the driver's loop matches only
   `crate::ast::Item::Function` (`src/driver/mod.rs:148-149`), so no method in an `impl` block is
   analysed at all.
3. **Delete `async` and `await` from the language.** They are keywords today — the two things N7
   says the language does not have are the two the implementation has. Making `.await` a hard error
   was M1's step toward the definition; removing the tokens is this one.
4. **Effect contexts**: `with_timeout(5.seconds) { with_retry(3) { … } }`, `effect::sync { … }`, and
   `-> async T` as the only two escape hatches. `with` and `effect` are not keywords today.
5. **Independent effectful operations are parallel by default.** This item carries a decision that
   is not the compiler's to make — see
   [F1](#f1-parallel-by-default-needs-an-execution-substrate-and-n7-appears-to-forbid-one).
   **Do not start it before that is answered.**

**Exit**: `make m7-exit`, plus `reject` rows proving the gate exists: a function declared pure that
calls an I/O builtin is refused, and a function whose callee is defined below it inherits that
callee's effects.

## M8 — Termination is provable (v0.10.0) · differentiator 2

**Why here, and why last**: it is the differentiator with the most prerequisites and the fewest
dependents. It needs M2 (attributes must lex; the blocker is one level below the parser), M4
(structural recursion is stated over inductive types with pattern matching on subterms — today
`match` has three pattern forms and generics do not survive codegen), and M7 (a `#[total]` function
requires every callee to be total, which is the same propagation-to-callers fixed point the effect
system builds; totality is the absence of a divergence effect). Building it on M7's machinery
rather than beside it is the reason it is here and not earlier.

**Owns**: nothing today, in either inventory — same gap as M5, same first task
([F4](#f4-two-differentiators-own-no-failing-row-anywhere)).

1. `#[total]`, `#![total(strict)]`, `#[decreases(expr)]`, `#[total(fuel = N)]`, `#[partial]`.
2. Structural recursion on an inductive type needs no measure; a recursive call on a strict subterm
   is proven automatically.
3. Failure to discharge an obligation is a compile error. **There is no mode in which an unproven
   `#[total]` function is accepted** — which is why the attribute must not lex before the checker
   exists (M2 item 5).
4. `unsafe` is not permitted in a `#![total(strict)]` crate, which is M7's classification doing the
   work.

**Exit**: `make m8-exit`, plus a `reject` row for each obligation the checker must refuse —
non-structural recursion with no measure, a measure that does not decrease, and fuel exhaustion.

## 1.0.0 — the first release that is not a prerelease

Not a milestone with new features: the milestone where every earlier one is proven to have
finished. Each criterion is a command.

1. `make gates` exits 0 — as it must at every commit, but with `vacuous=0`, `xfail=0` and
   `untranscribed=0` on the conformance line. No fixture in the corpus proves nothing.
2. `make test-xfail` reports `xfail=0`. Every declared failure has been paid.
3. `make m2-exit` … `make m8-exit` all exit 0.
4. The annex carries no `partial` and no `unimplemented` row against any of N1–N14. That is
   mechanical, over the same index the doc-evidence gate already parses: no row in
   `feature-index.toml` whose `spec` names a Part I anchor may have an `implementation` other than
   `implemented`. Run today it reports **21 rows still owed against Part I**; at 1.0 it reports
   zero, and it is three lines of `tomllib` — the natural body of a `make v1-exit`.
5. `make selfhost` reaches a fixed point **on the 1.0 language**, not on PBS-1 — the bootstrap
   compiler accepts what the specification defines.
6. Every OPEN block in Part I is closed: N12.1 and N4's `str`/`usize` question have answers, not
   options.

## What 1.0 does not say, and the owner has to

Two documents define 1.0 and they are not the same set. Fifteen of the 48 rows in
`feature-index.toml` name [`PALLADIUM_V1_FEATURES.md`](../reference/features/PALLADIUM_V1_FEATURES.md)
as their `spec` because **Part I has no section for them**: refinement types, proof export to
Lean/Coq, side-channel safety, incremental compilation, parallel compilation, Rust FFI, C FFI, WASM,
a debugger, a formatter, the LSP server, Cargo compatibility, a package registry — and the two
standard-library rows, which N14 does require even though the index points them elsewhere.

Twelve of the fifteen are `unimplemented`. This plan schedules none of them, and that is a choice
with a consequence either way:

- If the feature list is the 1.0 gate as written, **1.0 is years past M8** and needs milestones for
  a proof exporter and a WASM backend that no one has designed.
- If Part I is the 1.0 gate, the feature list needs amending to say so, and to name the subset that
  ships alongside — `pdc`, `pls`, the standard library and C FFI already have `partial` or
  `implemented` rows, so they are the natural ones.

**This is not the compiler's decision and it is not this file's.** The plan above assumes the second
reading, because Part I is what the specification calls normative, and it is flagged here rather
than buried so that assuming it is a visible act.

## Findings

Things measured while re-deriving this file that contradict a premise of the plan, or of a document
the plan rests on. Reported, not worked around.

### F1. Parallel-by-default needs an execution substrate, and N7 appears to forbid one

[N7](../specification/language-spec.md#n7-effects-and-asynchrony) states both "**Independent
effectful operations are parallel by default**" and "There is **no async runtime and no `Future`
boxing**. Effect tracking is entirely static and has no runtime representation", and
[`async-as-effect.md`](../reference/features/async-system/async-as-effect.md#3-optimization-opportunity)
says the compiler "can schedule them concurrently without the programmer writing a scheduler, and
it does so without tracking anything at runtime".

Two operations that each block on I/O cannot overlap unless something at run time makes them
overlap: OS threads, or non-blocking I/O plus an event loop. Both are runtime code, and the
generated C already links one — `runtime/palladium_runtime.c` supplies 16 file and path symbols.
**The C backend is not the obstacle** (pthreads and kqueue are C), so this is not the "abandon the
target" finding it might look like. What is at issue is narrower and real: whether that runtime may
grow a scheduler.

The two sentences are compatible if "no runtime representation" is read as *effect tracking* is
compile-time, while *execution* uses a runtime the programmer never names. They are incompatible on
any reading where "no async runtime" means no runtime code. The difference decides whether
differentiator 1 is "no function colouring" or "no function colouring **and** implicit parallelism",
and the second is roughly twice the milestone.

**Decision required before M7 item 5 starts.** If the answer is "no runtime code", the
parallel-by-default sentence has to leave N7 and the feature list, and this plan's M7 shrinks. If it
is "a runtime the programmer never names", N7 should say that, because as written it reads as a
prohibition.

### F2. M1 shipped three of its own declared failures, and its exit command cannot see them

`make m1-exit` exits 0 at `2ef170f`. It is `CONFORMANCE_FORBID_OWNER=M1` over
`tests/conformance-manifest.txt` and nothing else (`Makefile:270-271`), and no row there is owned by
M1. But the second owner inventory — the `(owned by M<n>)` tag every `#[ignore]` reason carries and
`scripts/test-xfail.py:74` parses — has three M1 rows, all still red:

| Row | What is still broken |
|---|---|
| `tests/e2e_test.rs:309` | a tail `if` is not lowered to a return |
| `tests/compiler_comprehensive_test.rs:567` | `fn f() -> int { }` compiles with no diagnostic |
| `tests/e2e_test.rs:269` | `fn double` emits `long long double(…)`; gcc rejects the compiler's own output |

The first reproduces at `2ef170f`: `fn fib(n: i64) -> i64 { if n <= 1 { n } else { fib(n-1) +
fib(n-2) } }` compiles clean, and `fib(10)` prints `8261746944` and exits 0. **A silent miscompile
shipped in the release named for removing silent miscompiles**, and the exit gate could not see it
because it read one of the two inventories. M2 re-owns all three, and every `m<n>-exit` from here
reads both — that is the durable fix, and it is the reason the exit targets above are three lines
rather than two.

### F3. The conformance corpus has no negative tests

`reject=0` on every run. The class exists, the runner implements it, and
[A11](../specification/language-spec.md#a11-conformance) advertises it as "how 'the compiler rejects
`.await`' gets tested instead of a program that prints prose about async being unimplemented" — and
no fixture uses it. The refusals are covered, but in Rust integration tests
(`tests/d5_unimplemented_constructs.rs`, `tests/d10_llvm_refuses.rs`), which the bootstrap compiler
will never run.

That matters for exactly one criterion: M3's "the bootstrap compiler's output matches the Rust
compiler's on every program" is satisfiable, as the corpus stands, by a compiler that refuses
nothing.

### F4. Two differentiators own no failing row anywhere

Implicit lifetimes: zero conformance rows, zero `#[ignore]` rows. Totality: zero and zero. Nothing
in this repository currently fails because lifetimes are not inferred or because termination is not
proven — the features are absent rather than broken, and absence has no fixture. `make m5-exit` and
`make m8-exit` as specified would therefore exit 0 the day they are added. Both milestones state
writing those fixtures as their first task for that reason, and neither exit criterion means
anything until they exist.

### F5. Two documents define 1.0 and they are not the same set

See [What 1.0 does not say](#what-10-does-not-say-and-the-owner-has-to). The LLVM backend is the
sharpest instance: [N1](../specification/language-spec.md#n1-overview-and-design-commitments) calls
a second backend "a second implementation of the same definition" — explicitly not a language
property — while the feature list has it under "6. Compilation & Optimization" as a 1.0 feature. It
refuses unconditionally today and this plan does not schedule it.

### F6. A second roadmap in the tree is written in the fictional present

[`docs/design/vision-roadmap.md`](../design/vision-roadmap.md) opens "Palladium α v0.7 has achieved
what many thought impossible", carries a benchmark table against Rust 1.74 for a compiler that could
not link a hello-world when it was written, and schedules "Q4 2026: Language Freeze". It does carry
the PROPOSAL banner, so it is flagged rather than hidden — but it assigns v0.7 to a past that never
happened while this file assigns v0.7 to M5. **Delete candidate**; this file is the sequencing
document, as [`PALLADIUM_V1_FEATURES.md`](../reference/features/PALLADIUM_V1_FEATURES.md#what-was-removed-from-this-document-and-why)
already says.

### F7. Three stale claims in documents this file rests on

Reported, not edited — none of them is this file:

- [A11](../specification/language-spec.md#a11-conformance) says "over 44 fixtures", "verified 33 ·
  vacuous 7 · xfail 2 · skip 2" and names "the three failures". `make conformance` at `2ef170f`
  prints `verified=43 … xfail=1 … failures=0` over 53. The `tests/stdlib/` rows M1 added are not in
  the annex's count.
- [A4.1](../specification/language-spec.md#a41-functions) and
  [`async-as-effect.md`](../reference/features/async-system/async-as-effect.md#where-the-implementation-currently-diverges)
  say `crate::effects::` "is referenced from exactly one place in the compiler". There are two
  outside the effects module and outside tests: `src/builtins.rs:182` and `src/driver/mod.rs:147`.
  The load-bearing half of the claim — that the analysis has exactly one consumer, a `println!` —
  holds; the count does not.
- The previous version of this file reported 2 unit-test failures and 43 integration failures.
  `make test-honest` exits 0.

### F8. One declared failure expects syntax the specification forbids

`tests/advanced_features_test.rs:340` is an `xfail` whose reason is that `macro_rules! vec { … }`
"is not an item". Under [N3](../specification/language-spec.md#n3-program-structure-and-items) it
must never be one — there is one macro system, and `scripts/check-doc-evidence.sh` already fails
any normative document that writes `macro_rules!`. A row that will be red forever unless the
language changes is not a debt owed to a milestone; it is a **negative test wearing the wrong
class**, and it should be re-declared so that the compiler *refusing* `macro_rules!` is what makes
it green.

It is the only row labelled M5, which is how it surfaced.

### F9. The milestone labels in the test suite were written against the old numbering

The `(owned by M<n>)` tags predate this file. M2 and M4 mean roughly what they meant before —
surface, abstraction — so those 32 rows carry over unchanged. Nine do not:

- the three tagged M1 are unpaid and are re-owned by M2
  ([F2](#f2-m1-shipped-three-of-its-own-declared-failures-and-its-exit-command-cannot-see-them));
- the five tagged `unscheduled` are effects and async, which this plan schedules as M7;
- the one tagged M5 meant "M5 — Library and tooling (v0.7+)" in the old numbering, and M5 is now
  implicit lifetimes. That row is [F8](#f8-one-declared-failure-expects-syntax-the-specification-forbids)
  and should leave the inventory rather than move.

Re-tagging those nine reasons is part of landing this plan, and it is an edit to `tests/`, which
this document could not make. Until it happens, `TEST_XFAIL_FORBID_OWNER=M7` finds nothing.

## Keeping this file honest

Every claim above is reproducible:

```bash
make gates          # conformance + gate self-test + docs + doc-evidence + selfhost + stdlib + probe
make test-honest    # every test binary, integration tests included
make test-xfail     # every declared failure, and the milestone that owes it
```

Three rules this file is held to:

1. **If a milestone's exit criterion cannot be written as a command, it is not an exit criterion.**
2. **A milestone owns rows, not intentions.** What it owes is the set of `xfail`, `vacuous` and
   `#[ignore]` rows tagged with it, in `tests/conformance-manifest.txt` and in the `#[ignore]`
   reasons — both readable by `CONFORMANCE_FORBID_OWNER` and `TEST_XFAIL_FORBID_OWNER`. A milestone
   that owns nothing cannot exit, because there is nothing for its exit command to be green about.
3. **Paying off a row is a transition, not a deletion.** The fixture stays on disk and its row
   becomes `run` with a transcript, in the same commit. Removing the row makes the fixture
   undeclared and the gate stays red — which is the point.

Every `file:line` on this page is fingerprinted by `make check-doc-evidence`, so a citation that
stops pointing at what it names fails a gate instead of rotting.
