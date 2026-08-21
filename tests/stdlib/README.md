# `tests/stdlib/` — coverage for what a standard library would rest on

These are **driver programs**, not library modules. `scripts/conformance.sh` skips any `.pd`
without a `fn main` (`SKIP_NO_MAIN`), so a library module contributes nothing to any gate; the only
way to prove a function behaves is to call it from a program and assert the result.

They exist because `stdlib/` cannot be tested — 0 of its 21 files compile and the compiler never
loads it (see [`../../stdlib/STATUS.md`](../../stdlib/STATUS.md)). So instead of faking a gate over
uncompilable files, these drivers cover the layer underneath: the language surface and the 38
builtins in `src/builtins.rs` that a real standard library would be built from.

| Driver | Covers |
|---|---|
| `stdlib_tail_return.pd` | D3 — every shape of tail-expression return, plus explicit `return` and the unit-tail case that must *not* be lowered |
| `stdlib_builtins_string.pd` | the 8 string builtins + `print` / `print_int` |
| `stdlib_builtins_char_args.pd` | `char_is_*`, `arg_count`, `arg_at` |
| `stdlib_builtins_file.pd` | the 16 usable file and path builtins |
| `stdlib_vec_i64.pd` | a working port of `stdlib/std/collections/vec_i64.pd`, the one stdlib file that is a single construct away from compiling |

## Two rules these drivers follow

**1. Every check prints its actual value, and the whole transcript is diffed against `<name>.expected`.**

This is not belt-and-braces; it is the only thing that works. The tail-return defect makes the
generated C undefined, and gcc at `-O2` may delete the assertion that would catch it. Measured with
the D3 fix reverted, `stdlib_tail_return.pd` printed `8261746944` instead of `42` **and exited 0** —
the `if (r != 42) panic(...)` guard was folded away. An exit-code-only gate cannot see this;
`scripts/conformance.sh` is exactly such a gate. `make stdlib-gate` compares transcripts.

**2. A failed assertion must exit non-zero.**

`panic` calls `abort()` (`runtime/pd_prelude.h:68`), so `make conformance` reports `RUN_FAIL` as
well. Both gates therefore see these files, by two independent mechanisms.

## Running them

```bash
make stdlib-gate     # transcript diff + the stdlib/ manifest + builtin accounting
make conformance     # picks these up automatically via find(1) over tests/
```

## If you change a driver

Regenerate its golden transcript only after you have checked the new values by hand against the
runtime source — the point of the `.expected` file is to notice changes, so blindly refreshing it
defeats it:

```bash
./target/release/pdc compile tests/stdlib/<name>.pd -o <name>
./build_output/<name> > tests/stdlib/<name>.expected
```

Adding a builtin to `src/builtins.rs` without adding it to `BUILTINS.tsv` fails the gate, and
marking one `COVERED` without any driver calling it fails too.
