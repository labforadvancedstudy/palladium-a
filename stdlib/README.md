# Palladium Standard Library — does not exist yet

> **Read [`STATUS.md`](STATUS.md) first.** It is the measurement; this file is the summary.

**0 of the 21 `.pd` files in this directory compile, and the compiler never loads any of them.**
Everything here is a sketch of a standard library written in a dialect Palladium does not
implement. A program compiled today gets the 38 builtins in `src/builtins.rs` and nothing else.

Verify with `make stdlib-gate`.

## What this file used to say

The previous version of this README carried an "Implementation Status" section claiming ✅ for
core types, collections, string utilities, math functions, basic I/O, memory utilities, trait
definitions and the prelude, and it documented an API — `use std::option::…`, `Vec::new()`,
`map.insert(…)`, `s.trim()`, `Box::new(42)` — with worked examples.

None of it was true. Not one of those items exists, because not one of the files defining them
compiles, and because nothing in `src/` ever reads this directory. The examples used module
syntax, method-call syntax and generics that the compiler does not implement. It has been deleted
rather than corrected, because a "status" table whose every row is wrong is worse than no table.

## Why nothing here compiles

Grouped by the first feature each file demands (full table in [`STATUS.md`](STATUS.md)):

| Missing feature | Files |
|---|---|
| `use` declarations (module system) | 8 |
| `#[...]` attributes | 3 |
| `pub` on a method inside `impl` | 3 |
| `mod` declarations (module system) | 2 |
| float literals | 1 |
| escapes in char literals | 1 |
| associated types in traits | 1 |
| generic parameter defaults | 1 |
| `let` without an initialiser | 1 |

The module system alone accounts for at least 10 of the 21 files. Note that this is the *first*
blocker per file, not the whole bill — lexing is a whole-file pass, so a lexer-level blocker masks
the parser-level blockers behind it.

`std/collections/vec_i64.pd` is the sole exception: it is ordinary Palladium and fails on one
construct, `let mut v: VecI64;`. A working port of it is exercised by the gate at
`tests/stdlib/stdlib_vec_i64.pd`.

## Is this directory loaded?

No. `grep -rn stdlib src/` returns zero hits. The module resolver searches `.`, `examples` and
`<exe_dir>/std` (`src/resolver/mod.rs:37-44`) — never `stdlib/` — and `Cargo.toml:34` excludes
`stdlib/*` from the published crate. `stdlib/prelude.pd` is not injected into anything; the only
prelude the compiler knows is `runtime/pd_prelude.h`, which is C.

## Where the real coverage lives

Because these files cannot run, they cannot be tested. What *can* be tested is the language
surface and the builtins a standard library would be built from, and that is what
`tests/stdlib/` does — including the tail-expression return that miscompiled unnoticed. See
[`STATUS.md`](STATUS.md).

## What would make this directory real

In dependency order, and each one is a milestone, not a patch:

1. A module system (`mod` + `use`) — unblocks 10 files and is the precondition for the rest.
2. `pub` on methods, and method-call syntax so an `impl` block is reachable at all
   (`docs/contributing/MILESTONES.md`, M2).
3. Traits with real dispatch and working generics (M4) — without them `Option<T>`, `Result<T, E>`
   and `Vec<T>` cannot be expressed.
4. Float literals, char escapes, attributes — small lexer work, but only worth doing once
   something above depends on them.

Until (1)-(3) land, the honest thing is to leave these files as the design sketch they are, and
keep the gate pinning the fact that they do not compile.
