# Changelog

All notable changes to Palladium will be documented in this file.

## [v0.2.0] - 2026-08-22

The release in which the compiler starts telling the truth.

Before this release, no `.pd` program in this repository's history had ever produced an
executable: the driver passed `runtime/palladium_runtime.c` to gcc, and that file had never been
committable because `.gitignore` carried a blanket `*.c`. Several documented features silently
generated wrong code rather than reporting an error. And the "100% bootstrap" claim in the README
was false — no Palladium-written compiler had ever compiled itself.

### Added

- **Self-hosting, verified as a fixed point.** `bootstrap/pdc.pd` is a Palladium compiler written
  in Palladium. `make selfhost` checks that the C emitted by stage 1 and by stage 2 is
  byte-identical, and it is. A compiler that merely parses its own source does not pass this gate.
- **`runtime/palladium_runtime.c`** — the C runtime the compiler had always referenced and never
  shipped, plus `runtime/pd_prelude.h` for generated C to include.
- **`arg_count()` / `arg_at(i)`** — Palladium programs can read command-line arguments. Codegen
  emits `int main(int argc, char** argv)` and captures them.
- **`pdc --print-runtime`** — prints the resolved runtime directory, so an install can be
  verified without compiling anything.
- **Gates**: `make selfhost`, `make conformance` (compiles, links and *runs* every `.pd` under
  `tests/` and `examples/`), `make check-docs` (compiles every documentation snippet),
  `make test-honest` (every test binary, including the integration tests that `cargo test --lib
  --bins` never ran), and `make gates` to run them together.
- **Benchmarks** against C and Rust, with output-equivalence checking before any timing.
- **Homebrew**: `brew install pdc` and `brew install pdc-preview` via `2lab-ai/tap`.

### Fixed

- **Nothing could link.** The C runtime is now shipped and tracked.
- **Tail expressions were silently discarded.** `fn add(a, b) -> i64 { a + b }` compiled cleanly
  and returned garbage — the generated C had no `return`. Every function in `stdlib/` that ended
  in an expression was affected.
- **`let` without a type annotation was emitted as `long long`** whatever the initializer was, so
  references, enum values and string copies became integers and failed in gcc. Inference now
  covers the common cases; an initializer with no rule is a compile error naming the variable,
  never a guess.
- **Call arguments were borrowed forever**, so a value could not be passed to two functions.
  Borrows now end with the call. `String` is a Copy type, which is what it has always been at
  runtime — it lowers to `const char*` and is never individually freed.
- **No C prototypes were emitted**, so calling a function defined later in the file produced C
  that gcc rejects, and mutual recursion was inexpressible.
- **Builtin registry drift**: the type checker knew 36 builtins and the borrow checker 25, so 11
  — including `string_len`, `string_char_at` and the `char_is_*` predicates — could not be used
  at all. Both passes now derive from one table, with tests that fail on drift.
- **An installed compiler could not compile anything**: the runtime path was relative to the
  working directory. It now resolves relative to the executable, with `$PALLADIUM_RUNTIME` as an
  override that fails loudly rather than silently falling back.
- **The release workflow built a binary that does not exist** (`palladium`, not `pdc`) and
  shipped it without the runtime.

### Changed

- **The specification is derived from the implementation.** `docs/specification/language-spec.md`
  marks every construct as working, parses-but-breaks-downstream, or not implemented, each with a
  source location. `grammar.ebnf` is derived from the parser; the previous version specified
  `else if`, closures, tuples, `loop`, compound assignment, bitwise operators and `as` casts, none
  of which parse.
- **Documentation is mechanically checked.** 508 of 560 code snippets did not compile; the
  documents that could not be salvaged were deleted rather than corrected, and
  `scripts/check-docs.sh` now compiles what remains.
- `docs/specification/bootstrap-subset.md` defines PBS-1, the subset the self-hosting compiler is
  written in *and* implements — the property whose absence made the previous bootstrap attempt
  permanently impossible.
- Milestones rewritten from measurements. The previous version listed "Traits and generics" and
  "Bootstrap/self-hosting" as completed.

### Known limitations

Unchanged and documented rather than implied: no traits (they parse and emit no code), no working
generics, no closures, no method call syntax, no `else if`, no `loop`, no compound assignment, no
floats or `char`, no `Vec`, no `Option`/`Result`, and `?`/`.await` emit C that does not compile.
See [the language specification](specification/language-spec.md).

## [v1.0-bootstrap] - 2025-01-16

> **Retracted.** This entry claimed self-hosting via "37 bootstrap compilers" and 6,508 lines of
> Palladium. Those compilers were string-rewriting programs; none compiled itself, and
> `bootstrap/v2_full_compiler` is written in a dialect its own parser does not implement, so it
> could not have. Kept for the record, not as a claim.

### Added
- Self-hosting achieved with 37 bootstrap compilers
- 6,508 lines of Palladium bootstrap code
- Complete compiler written in Palladium (lexer, parser, type checker, code generator)

### Known Limitations
- No module system (can't organize code across files)
- No generics (limits code reuse)
- Limited file I/O (only reads first line)
- No else-if support
- No continue in loops

## [v0.1-alpha] - 2025-01-01

### Added
- Initial release with core language features
- Basic types: i32, i64, u32, u64, bool, String
- Control flow: if/else, while, for loops
- Pattern matching with exhaustiveness checking
- Structs and enums
- Fixed-size arrays
