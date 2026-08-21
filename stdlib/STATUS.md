# `stdlib/` — measured status

**Measured**: 2026-08-22, against `pdc` built from this tree at `cargo build --release`.

## The short version

**0 of 21 files under `stdlib/` compile, and no default configuration loads any of them.**
`stdlib/` is not a standard library. It is a sketch of one, written in a dialect this language does
not implement. A user *can* force it onto the module search path via `$PALLADIUM_PATH` — and it
still fails to load, with the same parse blockers listed below.

Reproduce with `make stdlib-gate`, which recomputes every number on this page except the 437
tail-expression count, which is a one-off heuristic scan and is flagged as such where it appears.

## Is `stdlib/` live code or dead weight?

Dead weight — but the precise claim matters, and an earlier draft of this page overreached. The
module resolver **is** live and its search path **is** user-configurable, so "the compiler can
never load `stdlib/`" would have been false. What is true is narrower, and every row was measured:

| Question | Evidence | Answer |
|---|---|---|
| Does anything in the compiler reference `stdlib/`? | `grep -rn stdlib src/` → 0 hits (the only match anywhere is `#include <stdlib.h>` at `src/codegen/mod.rs:420`) | No |
| Is `stdlib/` on any **default** search path? | `src/resolver/mod.rs:37-38` searches `.` and `examples`; `:44` adds `<exe_dir>/std`, which no install creates; `:52` adds `$PALLADIUM_PATH`. `stdlib/` is on none of them by default | No |
| Is the resolver reachable at all? | **Yes** — via the `import` keyword (`src/parser/mod.rs:176`), *not* `use`. Measured: `import mymod;` prints `Resolved 1 modules`. Resolution runs only when a program has imports (`src/driver/mod.rs:89`) | Yes |
| Can a user put `stdlib/` on the path? | **Yes** — `$PALLADIUM_PATH` is user-configurable | Yes |
| Does that make it usable? | **No.** Measured: `PALLADIUM_PATH=…/stdlib/std pdc compile` on `import option;` fails with `Expected 'fn' for method, but found 'pub'` — the same blocker recorded for that file below. `import math;` fails on the float literal. Pinned by the gate's forced-import probe | No |
| Is `stdlib/` shipped or installed? | `Cargo.toml:34` excludes `stdlib/*` from the crate. `.github/workflows/release.yml:58` and `preview.yml:82` stage **only** `runtime` (`grep -rn stdlib .github/` → 0 hits). Both Homebrew formulae in `2lab-ai/homebrew-tap` install only the runtime — `(share/"palladium").install "runtime"` in `pdc.rb`, `(lib/"palladium").install "runtime"` in `pdc-preview.rb`. The Dockerfile copies `bootstrap`, `examples`, `docs` — not `stdlib` | No |
| Is `prelude.pd` auto-injected? | No. The only `prelude` in `src/` is `runtime/pd_prelude.h`, the **C** runtime header (`src/runtime_paths.rs:5`). `stdlib/prelude.pd` is never read | No |

**The conclusion, stated exactly:** no default configuration loads anything under `stdlib/`;
nothing packages or installs it; and even when a user deliberately forces it onto the resolver's
path, every module fails to load with the blocker recorded below. A Palladium program today gets
the 38 builtins in `src/builtins.rs` and nothing else.

That matches the status table in `docs/contributing/MILESTONES.md` — "Standard library — none" —
and contradicts the old `stdlib/README.md`, which claimed ✅ for collections, strings, math, I/O,
memory, traits and the prelude. None of those were true; see
[Correcting the record](#correcting-the-record).

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

That is measured, not asserted: `tests/stdlib/stdlib_vec_i64.pd` is that port. `make conformance`
runs it and diffs its transcript; `make stdlib-gate` checks its generated C. If the language ever
grows uninitialised `let`, the manifest entry for the original goes XPASS and must be updated.

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

The first sentence was true. The second is not.

Nothing under `stdlib/` was ever miscompiled, because nothing under `stdlib/` was ever compiled.
All 21 files are rejected at lex or parse time — they never reach the codegen pass where D3 lived —
and no default configuration loads them. A file that cannot be parsed cannot be miscompiled.

What is true is a weaker, counterfactual statement: `stdlib/` contains **437** functions that end
in a tail expression, so *if* it had ever compiled, D3 would have miscompiled all of them. The
defect's real victims were ordinary user programs.

> **Two caveats on the 437 figure.**
>
> It is **not** recomputed by `make stdlib-gate`, unlike every other number on this page. It comes
> from a one-off heuristic scan (a non-blank, non-comment line not ending in `;{},` immediately
> followed by a line containing only `}`), which approximates "function ends in a tail expression"
> without parsing. Treat it as an order-of-magnitude claim, reproducible with the awk script in
> the commit that added this file, not as a pinned measurement.
>
> It also **understates** the counterfactual. The heuristic requires a bare expression immediately
> before the closing brace, so a function ending in a tail `if` — whose last line is the `}` of the
> `else` — is *not* counted. Those functions are miscompiled too, and still are today (see
> [D3 is only half fixed](#d3-is-only-half-fixed-tail-if-is-never-lowered)). The same scan finds
> **369** further sites in `stdlib/` where a `}` is preceded by a `}`, an upper bound on the
> tail-`if` shape. So the true counterfactual blast radius is wider than 437, not narrower.

**What this branch corrected.** Each keeps the original wording with a correction adjacent to it,
rather than deleting the record:

| File | Corrected by |
|---|---|
| `docs/contributing/MILESTONES.md` | this branch — retraction on the page |
| `docs/CHANGELOG.md` | this branch — note under the D3 entry |
| `docs/specification/bootstrap-subset.md` | this branch — note under the defect table |
| `CLAUDE.md` | this branch — note on the D3 line |
| `docs/specification/language-spec.md` | **not this branch.** Corrected on `docs/restore-design-corpus` (commit `93573f3`), which owns that file's restructuring. It also found the line reference given in this branch's first hand-off was stale: the claim is in A6.6 "Tail expressions", not at `:295`/`:607`. It re-measured independently rather than accepting the hand-off, and attributes the two Homebrew formula paths to this unit |
| commit message `191f8c1` | immutable history; corrected by this branch's commit messages and by this table |

This table is a statement about *where the correction was made*, not a claim that the set is closed.
A later grep may find occurrences neither unit has seen; the honest position is that these are the
ones that were found and fixed, not that none remain.

The distinction matters because the false version misdirects the fix. It says "gate `stdlib/`",
which would produce a gate over 21 files that cannot compile — a gate that can only ever report
what it already knows, i.e. one that cannot fail for the right reason. The true version says: gate
the *language surface* that a standard library would rest on, starting with tail returns. That is
what `tests/stdlib/` does.

## What the gate does instead

`make stdlib-gate` (`scripts/stdlib-gate.sh`) has four phases:

- **Phase 0 — negative control.** Proves the harness can fail before any later "ok" is believed.
  Each control must *reach* its comparison — compile and run successfully — and only then is the
  comparison required to fail; and each requires the *specific* failure, never merely "something
  went wrong":
  - a planted transcript mismatch must be detected;
  - `panic()` must die from **SIGABRT specifically** (exit 134 = 128 + signal 6) *and* its
    caller-supplied message must reach stderr. A generic non-zero exit is rejected: a missing
    binary exits 127, and that used to count as proof. The message check is against the payload
    this gate supplies, not fixed wording, so a handler that returns and lets `abort()` re-raise
    does not neutralise it;
  - the generated-C checker must exit **1 with a well-formed `FINDING` line** — not merely
    non-zero. Exit 2 means the checker itself malfunctioned, which proves nothing.

  The signal number is hard-coded because it is verified on this project's targets. It must not be
  relaxed to "non-zero": that is the defect this control exists to prevent.
- **Phase 1 — pin this page.** Recompiles all 21 files and diffs verdict *and* blocker against
  `stdlib/MANIFEST.tsv`. Fails on REGRESSION, XPASS, VERDICT_CHANGED, BLOCKER_CHANGED, file-set
  drift, or a new symlink. Also runs the forced-`import` reachability probe described above.
- **Phase 2 — driver inventory and the generated-C invariant.** Three-way set equality between
  `tests/stdlib/DRIVERS.tsv`, the `.pd` files and the `.expected` files, then the structural check
  below.
- **Phase 3 — builtin accounting.** Every builtin in `src/builtins.rs` must be COVERED/PARTIAL
  (called by a driver **and** evidenced by an `@builtin <name>` line in that driver's golden) or
  UNUSABLE (re-proved to still fail at a pinned stage with a pinned diagnostic) in
  `tests/stdlib/BUILTINS.tsv`.

### Why the gate checks the generated C, not just the output

D3 makes the emitted C undefined, and UB has no stable manifestation. Measured with the fix
reverted, `fn add(a,b) -> i64 { a + b }` returned:

```
-O2 -> 8261746944, exit 0
-O0 -> 8264595040, exit 0
```

Garbage at both levels, exit 0 at both. So an exit-code gate cannot see D3 — and pinning an
optimisation level would not help either, because the garbage is garbage everywhere. Worse, a
transcript diff is not a guarantee in principle: on another libc or another compiler the garbage
could equal the expected value by accident and the diff would pass.

The only stable statement about D3 is **structural**, so `gate_probe.py generated-c` inspects
`build_output/*.c` and never runs anything. It uses two nets, and the invariant is the *combination*
of them — neither is a proof on its own:

- **Net A** (`scripts/check-c-returns.py`) — every non-void function's body must **return on every
  path**. This is a terminator analysis, not "contains a `return`" (which passed `classify`, above)
  and not "the last line is a `return`" (which would wrongly flag a legitimate
  `if (c) { return 1; } else { return 2; }`, whose last line is `}`). An if/else terminates iff both
  arms do; an `if` with no `else` never does; `while (1)` terminates only if nothing `break`s out of
  it. Its value is that it needs no C compiler to have an opinion.
- **Net B** — `-Werror=return-type`: the same question answered by a real compiler's control-flow
  graph over the real grammar. A frontend diagnostic, verified identical at `-O0`, `-O2` and `-O3`.
  A non-zero exit is only accepted as a Net B finding if the diagnostic is actually the return-type
  one; an unrelated failure (a missing header, say) is reported as "could not run", not as a defect.

Both nets share one exit taxonomy with **every other process this gate reads as evidence** —
`pdc` included — because they share one boundary, `scripts/gate_probe.py`:

| exit | meaning |
|---|---|
| 0 | the experiment ran and reached a normal conclusion |
| 1 | a reportable `FINDING` |
| 2 | **malfunction** — a signal, an unpinned exit code, a missing producer, an unreadable input, an analyser that raised, or a compiler that failed for an unrelated reason |

The rule the boundary enforces, which four review rounds converged on:

> **Diagnostic text is never sufficient evidence of a verdict.** The exit code says whether the
> experiment ran; the text only says what it found, and may be read only afterwards.

It is enforced structurally rather than by discipline: `classify()` returns `None` for the text
unless the process reached a pinned rejection code, so a caller *cannot* grep the output of a
process that did not finish. Sub-classification — verdict, blocker category, diagnostic — happens
inside the boundary, after that check.

This was not theoretical. Measured:

```
$ sh -c 'echo "error: No main function found" >&2; kill -9 $$'
exit 137, expected diagnostic already on stderr
```

The old shell code classified that `ACCEPTED_NO_MAIN` — a green verdict from a killed process — and
the same shape was reachable in the forced-import probe, the UNUSABLE probes, and both nets.
`make test-gate-probe` fault-injects exactly that case against **every** producer (17 cases),
including both signal conventions: `subprocess` reports `-9`, a POSIX shell reports `137`, and a
check written for one silently never fires on the other.

### What the boundary does and does not cover

An earlier report of mine claimed every remaining exit reader shares one
`case $?` tri-state form. **That was not true**, and it is the third structural
claim in this branch that measurement did not support. Counted:

| shape | count | what it reads |
|---|---|---|
| `case $?` over `gate_probe.py` | 6 | every producer treated as evidence: pdc verdicts, the forced-import probe, the UNUSABLE probes, driver compilation, the generated-C nets, the registry reconciliation |
| direct `if [ $? ]` / `[ -x ]` | 11 | Phase 0 controls and file-system predicates |

The residue is deliberate, not overlooked. Phase 0 runs the **compiled
artifacts** — it must observe that a planted transcript mismatch is detected and
that `panic()` dies from SIGABRT with its payload on stderr. That is a different
kind of evidence from "did the producer conclude", and routing it through a
process-conclusion boundary would not make it safer. What matters is that no
site outside the boundary turns a producer's *diagnostic text* into a verdict,
and that is now enforced by construction: the malfunction path prints no
producer text at all.

### Seam with `make conformance`

Transcript verification of `tests/stdlib/` belongs to **`make conformance`**, not here. All six
drivers listed in `tests/stdlib/DRIVERS.tsv` are programs with `fn main` — ordinary conformance
fixtures — and the conformance runner has an expected-output verdict class plus its own closed
inventory over `tests/`. Running and diffing them in both gates would ship two semantic standards
for one question. (Five of the six exercise builtins or the language surface; the sixth,
`stdlib_tail_if_defect`, is a codegen fixture whose `main` prints a constant.)

`stdlib/` itself stays in this gate: those are library modules with no `main`, where the only
pinnable thing is a compile verdict and its blocker. Different question, different gate.

## D3 is only half fixed: tail `if` is never lowered

The retraction above is about *where* D3 struck. This is about whether it is over. It is not.

`src/parser/mod.rs:536` lowers a tail **expression** to `Stmt::Return`. A tail `if` is not an
`Stmt::Expr`, so it is never lowered, and every function whose body ends in an `if`/`else` — the
natural shape for a recursive base case — still miscompiles exactly the way the original D3 did.

Measured 2026-08-22 against this tree, with D3 nominally fixed:

```
fn fib(n: i64) -> i64 { if n <= 1 { n } else { fib(n - 1) + fib(n - 2) } }
```

compiles with no diagnostic and emits

```c
long long fib(long long n) {
    if ((n <= 1)) {
    n;                                     // bare expression, no return
    } else {
    (fib((n - 1)) + fib((n - 2)));         // bare expression, no return
    }
}
```

`fib(10)` printed **8261746944** instead of 55, exit 0.

This is pinned, not fixed: `tests/stdlib/stdlib_tail_if_defect.pd` carries `fib` plus a second
shape, `classify`, which has an early `return` *and* a tail `if`. `tests/stdlib/DRIVERS.tsv`
records it as `known_violation:fib,classify`, so the gate requires exactly those two functions to
violate the invariant. If the violation spreads, moves, or disappears, the gate goes red. The
parser fix is a separate work unit on another branch.

### Why `classify` is in the fixture

It is the case that killed the first version of this invariant. The original Net A asked "does the
function contain at least one `return`?" — a question about the source construct the parser already
handles. `classify` emits an early `return 0;` and then a bare tail `if`, so that rule **passed**
it while `classify(5)` returned 0 instead of 10.

The invariant is now phrased over the emitted body — *every non-void function must return on every
path* — which is what makes it catch constructs nobody has looked at yet. Demonstrated: appending
an unrelated tail-`if` function to an existing clean driver turns the gate red with no manifest
change and no new test.

## Known defects this measurement surfaced (reported, not fixed)

0. **D3 is only half fixed** — see the section above. Pinned by the gate; parser fix not in scope
   for this branch.

1. **Six builtins are registered but cannot be called.** `file_flush`, `file_seek`,
   `file_open_ex`, `file_close_ex`, `file_read_ex` and `file_write_ex` are declared in
   `src/builtins.rs` over an `I64` handle, while `runtime/pd_prelude.h:229-251` declares the same
   functions over `FileHandle` (`void*`). `fix/m1-builtin-registry` enumerates **eleven** distinct
   mismatch dimensions across the six — beyond the handle, `whence` narrows `i64 -> u8` (so 256
   silently becomes 0), lengths convert to `size_t`, and `file_read_ex` wants a writable `char*`
   where Palladium can only supply an immutable `String`. All eleven were re-verified against this
   tree's runtime.

   **Status changed 2026-08-22.** That branch has landed. The type checker now refuses these calls
   with `Built-in <name> is registered but not callable: …`, so they fail at **compile**, not in
   gcc. `tests/stdlib/BUILTINS.tsv` pins the new stage and diagnostic; the reconciliation check
   against `Support::Unsupported` is now **active** rather than dormant. Worth recording how that
   transition was noticed: the gate went red on the first run after rebasing, with
   `NOT THE RECORDED DEFECT: expected rejection at link, got compile` for all six. The pin refused
   to absorb a changed world silently, which is the only reason it is a pin.

2. **`read_file_to_string` returns NULL on failure** (`runtime/pd_prelude.h:285`), unlike `arg_at`,
   which deliberately returns `""` so that "every string built-in assumes a non-NULL `const char*`"
   (`src/builtins.rs:180`). Passing that NULL to `string_len` would segfault. Only the success path
   is exercised by the drivers.

3. **Two further silent miscompiles, reported by another agent and not touched by these fixtures.**
   Checked: none of the six drivers or the six UNUSABLE probes uses a C keyword as an identifier,
   `self`, or a generic `impl`, so this gate neither exercises nor pins them.
   - **No C-keyword mangling of identifiers.** `fn double(x: int)` emits
     `long long double(long long x)`, which is not valid C.
   - **`self` and generic impls emit undefined C.** `fn area(self)` emits `struct Self self`, a type
     never defined; `impl<T> V<T>` emits `__pd_V<T>_get`, which is not an identifier.

   Both are codegen-shape defects of the same family as D3, and the generated-C checker is the
   natural place to pin them once someone owns the fix.
