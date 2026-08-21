# `stdlib/` — measured status

**Measured**: 2026-08-22, against `pdc` built from this tree at `cargo build --release`.

## The short version

**0 of 21 files under `stdlib/` compile, and the compiler never loads any of them.**
`stdlib/` is not a standard library. It is a sketch of one, written in a dialect this language
does not implement, and it is not reachable from any program.

Reproduce with `make stdlib-gate`, which recomputes every number on this page.

## Is `stdlib/` live code or dead weight?

Dead weight. Four independent pieces of evidence:

| Question | Evidence | Answer |
|---|---|---|
| Does anything in the compiler reference `stdlib/`? | `grep -rn stdlib src/` → 0 hits (the only match anywhere is `#include <stdlib.h>` at `src/codegen/mod.rs:420`) | No |
| Is `stdlib/` on the module search path? | `src/resolver/mod.rs:37-38` searches `.` and `examples`; `src/resolver/mod.rs:44` adds `<exe_dir>/std`, which does not exist; `src/resolver/mod.rs:52` adds `$PALLADIUM_PATH`. `stdlib/` appears in none of them | No |
| Is the prelude injected into user programs? | The only `prelude` in `src/` is `runtime/pd_prelude.h`, the **C** runtime header (`src/runtime_paths.rs:5`). `stdlib/prelude.pd` is never read. Module resolution only runs at all when the program has its own imports (`src/driver/mod.rs:89`) | No |
| Is `stdlib/` even shipped? | `Cargo.toml:34` lists `stdlib/*` under `exclude` | No |

So a Palladium program today gets the 38 builtins in `src/builtins.rs` and nothing else. That
matches the status table in `docs/contributing/MILESTONES.md` — "Standard library — none" — and
contradicts `stdlib/README.md`, which claims ✅ for collections, strings, math, I/O, memory,
traits and the prelude. None of those are true; see [Correcting the record](#correcting-the-record).

## The verdict table

Every file, compiled with `./target/release/pdc compile <file>`:

| File | Verdict | First blocker | What it is waiting on |
|---|---|---|---|
| `stdlib/prelude.pd` | COMPILE_FAIL | ATTRIBUTE | `#[cfg(debug_assertions)]` at line 163; behind it, 18 `use` + 2 `mod` |
| `stdlib/std/async.pd` | COMPILE_FAIL | ASSOC_TYPE | `type Output;` in a trait, line 6 |
| `stdlib/std/collections/hashmap.pd` | COMPILE_FAIL | ATTRIBUTE | `#[macro]` at line 475; also generics, `impl`, `use` |
| `stdlib/std/collections/mod.pd` | COMPILE_FAIL | MOD_DECL | `pub mod vec;` at line 4 |
| `stdlib/std/collections/vec.pd` | COMPILE_FAIL | ATTRIBUTE | `#[macro]` at line 452; also generics and `impl` |
| `stdlib/std/collections/vec_i64.pd` | COMPILE_FAIL | UNINIT_LET | `let mut v: VecI64;` at line 12 — **and nothing else** |
| `stdlib/std/env.pd` | COMPILE_FAIL | USE_DECL | `use std::result::{…};` at line 4 |
| `stdlib/std/fs.pd` | COMPILE_FAIL | USE_DECL | `use std::io::{…};` at line 4 |
| `stdlib/std/io.pd` | COMPILE_FAIL | USE_DECL | `use crate::std::option::Option;` at line 4 |
| `stdlib/std/math.pd` | COMPILE_FAIL | FLOAT_LITERAL | `3.14159…` at line 5 — the lexer has no float literal |
| `stdlib/std/mem.pd` | COMPILE_FAIL | USE_DECL | line 4 |
| `stdlib/std/mod.pd` | COMPILE_FAIL | MOD_DECL | `pub mod option;` at line 5 |
| `stdlib/std/net.pd` | COMPILE_FAIL | USE_DECL | line 4 |
| `stdlib/std/option.pd` | COMPILE_FAIL | PUB_FN_IN_IMPL | `pub fn is_some(…)` inside `impl<T> Option<T>`, line 11 |
| `stdlib/std/process.pd` | COMPILE_FAIL | USE_DECL | line 4 |
| `stdlib/std/result.pd` | COMPILE_FAIL | PUB_FN_IN_IMPL | line 11 |
| `stdlib/std/string.pd` | COMPILE_FAIL | CHAR_ESCAPE | `'\t'` at line 200 — escapes work in string literals, not char literals |
| `stdlib/std/sync.pd` | COMPILE_FAIL | USE_DECL | line 4 |
| `stdlib/std/thread.pd` | COMPILE_FAIL | USE_DECL | line 4 |
| `stdlib/std/time.pd` | COMPILE_FAIL | PUB_FN_IN_IMPL | line 12 |
| `stdlib/std/traits.pd` | COMPILE_FAIL | GENERIC_DEFAULT | `pub trait PartialEq<Rhs = Self>` at line 5 |

**Totals: 0 COMPILE_OK, 0 ACCEPTED_NO_MAIN, 21 COMPILE_FAIL, 0 LINK_FAIL, 0 RUN_OK.**

Every failure is at lex or parse time. Not one file reaches the type checker, the borrow checker
or codegen.

### A note on `ACCEPTED_NO_MAIN`

`pdc compile` rejects any file without a `fn main` ("No main function found"), and every file here
is a library module. So `COMPILE_OK` is *unreachable* for `stdlib/` no matter how much the language
grows, and a gate whose success condition is `COMPILE_OK` could never fire.

The gate therefore has a fourth verdict, `ACCEPTED_NO_MAIN`: the language accepted the file and only
the harness's main-function requirement stands in the way. That is the verdict a working stdlib
module would carry, and it is what the XPASS check compares against. Verified: patching the single
blocking line in `vec_i64.pd` moves it to `ACCEPTED_NO_MAIN` and the gate reports

```
FAIL XPASS: stdlib/std/collections/vec_i64.pd is recorded COMPILE_FAIL
     but the language now accepts it (ACCEPTED_NO_MAIN) — update stdlib/MANIFEST.tsv
```

## Failures grouped by the feature they demand

| Missing feature | Files | Which |
|---|---|---|
| `use` declarations (module system) | 8 | env, fs, io, mem, net, process, sync, thread |
| `#[...]` attributes (lexer rejects `#`) | 3 | prelude, collections/vec, collections/hashmap |
| `pub` on a method inside `impl` | 3 | option, result, time |
| `mod` declarations (module system) | 2 | std/mod, collections/mod |
| float literals | 1 | math |
| escapes in char literals | 1 | string |
| associated types in traits | 1 | async |
| generic parameter defaults | 1 | traits |
| `let` without an initialiser | 1 | collections/vec_i64 |

**This grouping is a floor, not the full bill.** Lexing is a whole-file pass, so a lexer-level
blocker anywhere in a file masks every parser-level blocker in it. `prelude.pd` is listed under
attributes because of line 163, but it opens with `pub use crate::std::option::{…}` on line 9,
which the parser also rejects. The honest reading of the table is: *the module system alone
accounts for at least 10 of 21 files, and no file is one feature away except `vec_i64.pd`.*

Each category was reproduced independently with a one-line probe rather than inferred from the
first error, e.g.:

```
use crate::std::option::Option;          → Expected function, struct, enum, trait, type, impl, …
pub use crate::std::option::Option;      → same (the `pub` makes no difference)
pub mod vec;                             → same
impl S { pub fn get(self: &S) -> i64 {…} } → Expected 'fn' for method, but found 'pub'
let x: f64 = 3.14;                       → Expected field name, but found integer 14
let c: char = '\t';                      → Unexpected character '\'
#[cfg(debug_assertions)]                 → Unexpected character '#'
trait Future { type Output; }            → Expected 'fn' for trait method, but found 'type'
trait PartialEq<Rhs = Self> {…}          → Expected '>' after generic parameters, but found '='
let mut v: S;                            → Expected '=' after variable name, but found ';'
```

## The one file that is nearly real

`stdlib/std/collections/vec_i64.pd` is the only file not written in an unimplemented dialect. It
uses no `use`, no `mod`, no `impl`, no generics, no traits, no attributes and no floats. It fails
on exactly one construct — `let mut v: VecI64;` at line 12 — and replacing that single line with a
struct literal makes the whole file compile, link and run.

That is measured, not asserted: `tests/stdlib/stdlib_vec_i64.pd` is that port, and the gate runs it
on every invocation. If the language ever grows uninitialised `let`, the manifest entry for the
original goes XPASS and must be updated.

## Correcting the record

### `stdlib/README.md` is fiction

Its "Implementation Status" section claims ✅ for core types, collections, string utilities, math
functions, I/O, memory utilities, trait definitions and the prelude. Measured: **none of these
exist**, because none of the files that define them compile, and nothing loads them. Its usage
examples (`use std::option::…`, `Vec::new()`, `s.trim()`, `map.insert(…)`) use module syntax,
method-call syntax and generics that the compiler does not implement.

### "The tail-return defect lived in `stdlib/`" is false

`docs/contributing/MILESTONES.md` claimed:

> `stdlib/` has no conformance coverage at all. That is precisely why the tail-return defect
> lived there, silently miscompiling every function that ended in an expression, for a year.

The first sentence was true. The second is not, and the same claim appears in `docs/CHANGELOG.md`,
`docs/specification/language-spec.md`, `docs/specification/bootstrap-subset.md` and in the message
of commit `191f8c1` ("All of `stdlib/` was affected").

Nothing under `stdlib/` was ever miscompiled, because nothing under `stdlib/` was ever compiled.
All 21 files are rejected at lex or parse time — they never reach the codegen pass where D3 lived —
and the compiler never loads them in the first place. A file that cannot be parsed cannot be
miscompiled.

What is true is a weaker, counterfactual statement: `stdlib/` contains **437** functions that end
in a tail expression, so *if* it had ever compiled, D3 would have miscompiled all of them. The
defect's real victims were ordinary user programs.

The distinction matters because the false version misdirects the fix. It says "gate `stdlib/`",
which would produce a gate over 21 files that cannot compile — a gate that can only ever report
what it already knows, i.e. one that cannot fail for the right reason. The true version says: gate
the *language surface* that a standard library would rest on, starting with tail returns. That is
what `tests/stdlib/` does.

## What the gate does instead

`make stdlib-gate` (`scripts/stdlib-gate.sh`) has four phases:

- **Phase 0 — negative control.** Proves the harness can fail: a deliberately-wrong transcript must
  be detected, and `panic()` must exit non-zero. If these pass silently, every later "ok" is void.
- **Phase 1 — pin this page.** Recompiles all 21 files and diffs the result against
  `stdlib/MANIFEST.tsv`. Fails on REGRESSION, XPASS, VERDICT_CHANGED, BLOCKER_CHANGED, or file-set
  drift.
- **Phase 2 — the real coverage.** Compiles, links, runs and **transcript-diffs** the drivers in
  `tests/stdlib/`.
- **Phase 3 — builtin accounting.** Every builtin in `src/builtins.rs` must be COVERED (and
  actually called) or UNUSABLE (and re-proved unusable) in `tests/stdlib/BUILTINS.tsv`.

### Why the drivers compare transcripts and not just exit codes

D3 makes the generated C undefined, and gcc at `-O2` is entitled to exploit that. Measured with the
fix reverted, `tests/stdlib/stdlib_tail_return.pd` printed `8261746944` where it should have printed
`42` — and **still exited 0**, because gcc folded the `if (r != 42)` guard away and deleted the
`panic` that was supposed to catch it.

`scripts/conformance.sh` judges a program by its exit code alone. It therefore **cannot** catch D3.
The stdlib gate compares the whole transcript, which can, and does.

## Known defects this measurement surfaced (reported, not fixed)

1. **Six builtins are registered but do not compile.** `file_flush`, `file_seek`, `file_open_ex`,
   `file_close_ex`, `file_read_ex` and `file_write_ex` are declared in `src/builtins.rs` as taking
   an `I64` handle, but `runtime/pd_prelude.h:227-250` declares the same functions over
   `FileHandle` (`void*`). Calling any of them type-checks and borrow-checks cleanly, then dies in
   gcc: `incompatible integer to pointer conversion passing 'int' to parameter of type
   'FileHandle'`. This is the D2 builtin-drift class one layer down — the "one table" invariant is
   enforced between the type checker and the borrow checker, but nothing checks the canonical table
   against the C runtime's signatures. Pinned as UNUSABLE in `tests/stdlib/BUILTINS.tsv`.

2. **`read_file_to_string` returns NULL on failure** (`runtime/pd_prelude.h:285`), unlike `arg_at`,
   which deliberately returns `""` so that "every string built-in assumes a non-NULL `const char*`"
   (`src/builtins.rs:180`). Passing that NULL to `string_len` would segfault. Only the success path
   is exercised by the drivers.
