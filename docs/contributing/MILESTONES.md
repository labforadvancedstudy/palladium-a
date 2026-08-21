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
| Language conformance | 39 of 42 programs compile, link and run — `make conformance` |
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
| D7 | an un-annotated `let` is emitted as `long long` whatever the initializer was, so references, enum values and string copies silently become integers |
| D5 | ~~`?` emits C referencing a `struct Result` layout codegen never defines; `.await` calls a `poll` member that is never generated. Neither reports an error~~ — **fixed**: both are rejected with "is not implemented", the consequence, and a workaround. The lowerings are preserved unreachable for M4 |
| D4 | `for` over an array *parameter* uses `sizeof` on a pointer that has already decayed |
| D9 | `&[T; N]` / `&mut [T; N]` parameters are rejected in codegen — `examples/practical/simple_sort.pd` still fails on exactly this |

Two structural gaps belong here too, because both are gates that cannot see their own failures:

- **`stdlib/` has no conformance coverage at all.** That is precisely why the tail-return defect
  lived there, silently miscompiling every function that ended in an expression, for a year.
- **Three hand-written builtin tables still exist outside the canonical registry** — in the
  effects checker and in two LSP files. The "one table" invariant currently holds only for the
  type checker and the borrow checker.

**Exit**: nothing in the language specification is marked ⚠️ "parses, then breaks" without also
being reported as an error. `make conformance` at 42/42. `stdlib/` behind a gate.

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
