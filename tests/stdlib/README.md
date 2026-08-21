# `tests/stdlib/` — coverage for what a standard library would rest on

These are **driver programs**, not library modules: each has a `fn main`, so each is an ordinary
conformance fixture. A library module contributes nothing to any gate (a `.pd` without `main` is
classed `skip`); the only way to prove a function behaves is to call it from a program and assert
the result.

They exist because `stdlib/` cannot be tested — 0 of its 21 files compile and no default
configuration loads it (see [`../../stdlib/STATUS.md`](../../stdlib/STATUS.md)). Rather than fake a
gate over uncompilable files, these drivers cover the layer underneath: the language surface and
the 38 builtins in `src/builtins.rs` that a real standard library would be built from.

| Driver | Covers |
|---|---|
| `stdlib_tail_return.pd` | D3 — every shape of tail-expression return, plus explicit `return` and the unit tail that must *not* be lowered |
| `stdlib_builtins_string.pd` | the 8 string builtins + `print` / `print_int` |
| `stdlib_builtins_char_args.pd` | `char_is_*`, `arg_count`, `arg_at` |
| `stdlib_builtins_file.pd` | the 16 usable file and path builtins |
| `stdlib_vec_i64.pd` | a working port of `stdlib/std/collections/vec_i64.pd`, the one stdlib file a single construct away from compiling |

Each has a sibling `<name>.expected` golden transcript.

## Which gate checks what

Two gates, two questions, no overlap:

| | `make conformance` | `make stdlib-gate` |
|---|---|---|
| runs the programs | **yes** | no |
| diffs stdout against `<name>.expected` | **yes** | no |
| pins the fixture inventory | yes (closed inventory over `tests/`) | yes (`DRIVERS.tsv`, plus golden↔driver set equality) |
| inspects the generated C | no | **yes** |
| accounts for every builtin | no | **yes** (`BUILTINS.tsv`) |

Duplicating execution in both would ship two semantic standards for one question. A driver added
here must be declared in **both** inventories.

## Three rules these drivers follow

**1. Every check prints its actual value, and the transcript is diffed against `<name>.expected`.**

Exit status alone cannot see a wrong answer. `panic` calls `abort()` (`runtime/pd_prelude.h:68`),
so a failed assertion is *also* a non-zero exit — but the value is what the diff catches.

**2. The generated C is checked structurally, because the defect this exists for is UB.**

With the D3 fix reverted, `fn add(a,b) -> i64 { a + b }` returns garbage and exits 0 at **both**
`-O0` (`8264595040`) and `-O2` (`8261746944`). No runtime observation of undefined behaviour is
stable — on another libc the garbage could even equal the expected value and the diff would pass.
So `scripts/check-generated-c.sh` requires every non-void function in `build_output/*.c` to contain
a `return` (Net A) and to survive `-Werror=return-type` (Net B). It never runs anything.

**3. Every builtin exercised prints `@builtin <name>`.**

That line lands in the golden, so `BUILTINS.tsv` can demand *runtime* evidence that the section
ran, not just a source grep that a dead branch would satisfy. Both are required.

## If you change a driver

The `.expected` file is a **contract**, not a snapshot. Regenerate it only after checking the new
values by hand against the runtime source, and update the provenance comment that explains why the
value is correct — that comment, not the golden, is the reason anyone should believe it:

```bash
./target/release/pdc compile tests/stdlib/<name>.pd -o <name>
./build_output/<name> > tests/stdlib/<name>.expected
```

Review a golden diff the way you would review a change to an assertion. Transcript comparison
cannot defend against someone editing the implementation and the golden together; only the
provenance comments and a reviewer can.

Adding a builtin to `src/builtins.rs` without adding it to `BUILTINS.tsv` fails the gate; marking
one `COVERED` without a driver calling it *and* an `@builtin` line in a golden fails too; and
deleting a driver fails the inventory check.
