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
| [Language specification](specification/language-spec.md) | every construct, marked working / parses-but-broken / not implemented, each with a source location |
| [Grammar](specification/grammar.ebnf) | EBNF derived from `src/parser/mod.rs`, not from intent |
| [Bootstrap subset (PBS-1)](specification/bootstrap-subset.md) | the subset the self-hosting compiler is written in *and* implements; the self-hosting gate; the open-defect table |
| [Builtins](reference/builtins.md) | all 38, generated from `src/builtins.rs` |

Read the specification's middle category before trusting anything. `?`, `.await` and
un-annotated `let` used to pass the type checker and then emit C that does not compile — or,
worse, C that runs and is wrong; all three are now compile errors that name the construct.

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
