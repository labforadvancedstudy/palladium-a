# Palladium documentation

Every claim in `specification/`, `user-guide/` and `reference/` is checked against the compiler.
Code blocks are compiled by `scripts/check-docs.sh` (`make check-docs`), and the builtin
reference is generated from the compiler's own table. If a snippet lives in those directories
and is not marked `no-compile`, it compiles.

That mechanism is the point. Before 2026-08-21 this tree held 508 non-compiling snippets
describing a language that did not exist — traits, generics with bounds, closures, `async`,
floats, a standard library. Those documents were deleted rather than corrected, because nothing
in them bore a salvageable relationship to the implementation.

## Start here

| | |
|---|---|
| [Getting started](user-guide/getting-started.md) | install, first program, verifying the install, troubleshooting |
| [Tutorial](user-guide/tutorial.md) | the language, worked through — every snippet is compiled |

## Reference

| | |
|---|---|
| [Language specification](specification/language-spec.md) | two parts: Part I defines the language, Part II records what `pdc` implements, construct by construct, with a source location for every row |
| [Grammar](specification/grammar.ebnf) | EBNF; Part A is derived from `src/parser/mod.rs`, Part B lists the normative productions the parser does not accept |
| [Bootstrap subset (PBS-1)](specification/bootstrap-subset.md) | the subset the self-hosting compiler is written in *and* implements; the self-hosting gate; the open-defect table |
| [Builtins](reference/builtins.md) | all 34, generated from `src/builtins.rs`; all callable |

Read the annex's **partial** rows before trusting anything. `?`, `.await` and un-annotated `let`
used to pass the type checker and then emit C that does not compile — or, worse, C that runs and is
wrong; all three are now compile errors that name the construct. Tuples still parse and lower to
`void*`.

## Feature definitions

[`reference/features/`](reference/features/) defines the language's features, including the three
it exists for. These documents are **normative**: they say what Palladium is, not what `pdc` does.
Their Palladium blocks are fenced `no-compile` because the compiler does not accept that syntax
yet, and each carries a "where the implementation currently diverges" section with `file:line`
evidence.

| | |
|---|---|
| [v1.0 feature list](reference/features/PALLADIUM_V1_FEATURES.md) | every feature that constitutes v1.0, and the eight advantages over Rust |
| [Async as effect](reference/features/async-system/async-as-effect.md) | no `async`, no `.await`, no function coloring |
| [Totality checking](reference/features/advanced/totality-checking.md) | `#![total(strict)]`, `#[decreases(expr)]` |
| [Implicit lifetimes](reference/features/core-language/implicit-lifetimes.md) | `ref` / `ref mut`, no `'a` |
| [Feature index](reference/features/feature-index.toml) | one row per feature: where it is defined, what `pdc` does, and the evidence |

## Internals

| | |
|---|---|
| [Architecture](internals/ARCHITECTURE.md) | compiler structure |
| [Error messages](internals/ERROR_MESSAGES_IMPROVEMENT.md) | diagnostics work |
| [Performance notes](internals/PERFORMANCE_OPTIMIZATION.md) | optimiser notes |
| [Bootstrap history](internals/bootstrap/) | earlier bootstrap attempts, kept as narrative |

> The bootstrap history predates the 2026-08 fixed point and describes attempts that never
> compiled themselves. It is kept because the reasoning is interesting, not because it is
> accurate; its snippets are excluded from the documentation check.

## Proposals

[`design/`](design/) holds designs that were never built — traits, generics, the module system,
the technical manifesto, the roadmap. Every file there opens with a PROPOSAL banner and is
excluded from the documentation check. Nothing in that directory describes current behaviour.

The specification reads three of them — the trait system, generics and the module system — as
normative for the language and unimplemented in `pdc`
([N10](specification/language-spec.md#n10-traits-and-generics),
[N11](specification/language-spec.md#n11-modules)). Their PROPOSAL banners remain accurate about
the compiler; they are not accurate about the language's intent, and reconciling the two wordings
is open.

## Project

| | |
|---|---|
| [Milestones](contributing/MILESTONES.md) | what is next, and why |
| [Palladium vs Rust](contributing/palladium_vs_rust_comparison.md) | measured benchmarks and an honest feature comparison |
| [Changelog](CHANGELOG.md) | |

## Gates

| Command | Checks |
|---|---|
| `make selfhost` | the compiler compiles itself to a byte-identical fixed point |
| `make conformance` | every `.pd` under `tests/` and `examples/` compiles, links and runs |
| `make check-docs` | every documentation snippet compiles |
| `make test-honest` | every Rust test binary, integration tests included |
