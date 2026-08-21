# Milestones

**Updated**: 2026-08-22 · **Current version**: 0.2.0

Ordered by what unblocks what, not by theme. Every milestone exits on a command, not on an
opinion.

> The previous version of this file recorded "v0.4: Traits and generics" and "v0.6:
> Bootstrap/self-hosting achieved" as completed milestones. Neither had happened — traits emit no
> code at all, and no Palladium-written compiler had ever compiled itself. That document was
> planning from a fictional present, which is how a project ends up with a detailed roadmap to
> 1.0 and a compiler that cannot link a hello-world. This one starts from measurements.

## Where the project actually is

| | |
|---|---|
| Self-hosting | ✅ fixed point — `make selfhost` |
| Language conformance | 33 of 44 fixtures run with a transcript diffed byte-for-byte; 7 are vacuous placeholders, 2 declared-failing, 2 non-programs — `make conformance` |
| Conformance gate itself | 87 regression cases, each proving it still goes RED — `make test-conformance-runner` |
| Documentation | every snippet compiles — `make check-docs` |
| Unit tests | 404 pass, 2 pre-existing failures |
| Integration tests | 43 fail, all pre-existing — `make test-honest` |
| Traits | parse, emit nothing |
| Generics | partial monomorphisation; `Foo<T>` is misparsed |
| Standard library | none — 38 builtins, fixed-size arrays, no `Vec` |

## M1 — The compiler stops lying (v0.3)

**Why first**: every other kind of work is slower while the compiler can accept a program and
emit wrong code. This milestone converts silent wrongness into diagnostics.

| Defect | What happens today |
|---|---|
| D5 | `?` emits C referencing a `struct Result` layout codegen never defines; `.await` calls a `poll` member that is never generated. Neither reports an error |
| D4 | `for` over an array *parameter* uses `sizeof` on a pointer that has already decayed |
| D9 | `&[T; N]` / `&mut [T; N]` parameters are rejected in codegen — `examples/practical/simple_sort.pd` still fails on exactly this, and is the one M1-owned entry in `tests/conformance-manifest.txt` |

Closed:

- **D7** — an un-annotated `let` was emitted as `long long` regardless of its initializer. Fixed in
  `04104c5` ("fix(codegen): infer let types instead of defaulting them to i64"). Verified: a program
  containing `let s = "hello"; let b = true; let p = P { x: 3 };` compiles, links and runs, and the
  emitted C declares `const char* s` and `struct P p` rather than `long long`.

Two structural gaps belong here too, because both are gates that cannot see their own failures:

- **`stdlib/` has no conformance coverage at all.** That is precisely why the tail-return defect
  lived there, silently miscompiling every function that ended in an expression, for a year.
- **Three hand-written builtin tables still exist outside the canonical registry** — in the
  effects checker and in two LSP files. The "one table" invariant currently holds only for the
  type checker and the borrow checker.

**Exit** — every criterion is a command, not a reading of prose:

1. `make conformance` exits 0 (`failures=0`).
2. `make m1-exit` exits 0. This is `CONFORMANCE_FORBID_OWNER=M1`, which fails while any fixture
   in `tests/conformance-manifest.txt` is still owed to M1. Today exactly one is:
   `examples/practical/simple_sort.pd` (D9). The owner column is a structured, enforced field, so
   this criterion is decided by the runner rather than by reading the table above.
3. `make test-conformance-runner` exits 0 — the gate is still able to fail.
4. Nothing in the language specification is marked ⚠️ "parses, then breaks" without also being
   reported as an error.
5. `stdlib/` behind a gate.

`42/42` was the old exit criterion and it was the wrong target twice over.

It counted **seven** placeholder programs — `02_types_enums`, `07_traits_basic`,
`08_generics_basic`, `09_effects_system`, `10_async_await`, `11_unsafe_blocks`,
`12_modules_imports` — that only *print* "not yet implemented". None declares an enum or a trait,
instantiates a generic, carries an effect annotation, opens an `unsafe` block, or contains an
`async fn` or `.await`. The consequence was not theoretical: **defect D5 survived because
`10_async_await.pd` was counted as async coverage while testing nothing.** These are now reported
as `vacuous`, each declaring which feature it fails to cover, and are expected to stay vacuous
through M1. Sixteen per cent of the corpus proves nothing, which is the honest number.

It also counted a *green exit code* as a correct program. A missing C `return` is undefined
behaviour: measured here at both `-O0` and `-O2`, `long long f(a,b){ (a+b); }` returns
`8261746944` and exits 0. That is defect D3's exact signature, which is how D3 miscompiled
`stdlib/` for a year underneath a green gate. The runner now diffs each fixture's stdout against a
recorded transcript. There is no exit-code-only class: a fixture that genuinely cannot be
transcribed must be declared `untranscribed` with an owner and a `why:` reason, and is reported as
a debt on every run. That count is currently zero.

## M2 — Writing real programs stops hurting (v0.4)

**Why second**: after M1 the compiler is trustworthy but still unpleasant. These are the gaps
that make people give up on the first afternoon, in rough order of how often they bite.

1. **Method call syntax** `x.f()` — rejected today with "Indirect function calls not yet
   supported". The largest ergonomic gap: every `impl` block is unreachable from source without
   it.
2. **`else if`** — a one-line parser change. Today every conditional chain must be nested.
3. **`loop` and compound assignment** (`+=`) — both are missing lexer tokens.
4. **Literal patterns in `match`** — `match` cannot dispatch on an integer or a string, which
   forces `if`/`else` chains wherever a state machine would be natural.
5. **A growable `Vec`** — the first item here that is a design question rather than a fix: it
   needs an allocation and ownership story, not just a type.

**Exit**: a non-trivial program written with no workarounds — a JSON parser is the proposed
benchmark — and added to the conformance corpus.

## M3 — The Rust compiler becomes redundant (v0.5)

**Why third**: this is the project's actual thesis, and it only becomes achievable once M1 and M2
have widened the subset enough to express a real compiler comfortably.

`bootstrap/pdc.pd` today implements a deliberately small subset: structs, functions, `let`,
assignment, `if`/`else`, `while`, `return`, calls, indexing, field access, and the builtins. It
compiles itself to a byte-identical fixed point, which is the hard part — but it has no enums, no
`match`, no `for`, no generics and no modules, so it cannot yet compile what the Rust compiler
accepts.

1. Grow the bootstrap compiler and PBS-1 together, one construct at a time, holding
   `make selfhost` green at every step. Rule PBS-0 is what keeps this honest: a construct enters
   the subset only when the bootstrap compiler both *accepts* and *implements* it. Violating that
   rule is exactly how `bootstrap/v2_full_compiler` became permanently uncompilable.
2. When the bootstrap compiler accepts everything the Rust compiler accepts, compile the whole
   corpus with both and diff the output.
3. Retire `src/` as the primary compiler.

**Exit**: `bootstrap/pdc.pd` compiles the full conformance corpus, and its output matches the
Rust compiler's on every program.

## M4 — Abstraction (v0.6)

Deliberately after self-hosting: implementing traits twice, once in Rust and once in Palladium,
is the kind of duplicated work that kills projects like this one.

- **Traits with real dispatch.** They parse today and produce no C; there is no vtable mechanism
  anywhere in the compiler. A design exists at
  [`docs/design/trait_system_design.md`](../design/trait_system_design.md).
- **Generics that work.** Monomorphisation is partial, and inside `<…>` any all-uppercase name is
  reclassified as a *const* generic argument, so `Foo<T>` does not mean what it looks like.
- **A real reference type.** The type checker has no reference type at all — `&T` and `T` are
  indistinguishable to it. Until that changes, `String` cannot be given move semantics, because
  the language has no way to express borrowing one.

**Exit**: `Option<T>` and `Result<T, E>` are ordinary library types with working methods, and `?`
works against the real one instead of a fabricated C layout.

## M5 — Library and tooling (v0.7+)

Only meaningful once M4 lands: a standard library without traits or generics is a pile of free
functions.

- Collections — `Vec`, `HashMap`, a string builder
- I/O beyond the current handle-based builtins
- `pdm` (package manager) and `pls` (language server): both exist as binaries, neither is driven
  by any gate
- The LLVM backend, which is skeletal — break, continue, pattern matching, enum construction, `?`
  and `await` are all unimplemented there

## Not scheduled, and why

`async`/`.await`, the effect system, and totality checking appear throughout the older
documentation as though they were close. They are not. Effect clauses do not exist in the surface
syntax; effects are inferred after the fact and gate nothing; `async` emits a call to a function
that is never generated. They are design proposals — see [`docs/design/`](../design/) — and belong
after M4 at the earliest, because each needs a type system able to express it.

## Keeping this file honest

Every claim above is reproducible:

```bash
make gates          # conformance + documentation + self-hosting
make test-honest    # every test binary, integration tests included
```

If a milestone's exit criterion cannot be written as a command, it is not an exit criterion.
