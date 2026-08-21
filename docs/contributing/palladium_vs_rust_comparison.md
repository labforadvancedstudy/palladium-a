# Palladium vs Rust — measured

**Date**: 2026-08-22 · **Data**: [`benchmarks/results/latest.json`](../../benchmarks/results/latest.json)
· **Reproduce**: `bash benchmarks/run_benchmarks.sh`

The previous version of this document compared a splay-tree implementation written in a language
that did not exist — `pdc translate --from-rust`, `pdc --verify-total`, implicit smart pointers,
none of which are implemented — and concluded that Palladium won. This version reports what four
benchmarks actually did on one machine, with the ways the comparison is unfair stated rather than
buried.

## The short version

On numeric code, **Palladium's runtime performance is the C compiler's performance**. The
generated C, when optimized, produces machine code byte-identical to hand-written C. That is the
honest ceiling, and Palladium reaches it.

Two things sit between that ceiling and what a user gets today:

1. **`pdc` forks `gcc` with no `-O` flag**, so the shipped binary is `-O0` — a 2–6× loss that has
   nothing to do with the language.
2. **String-heavy code is genuinely slow**, and that one *is* Palladium's fault: the string arena
   never frees, so a quadratic concatenation benchmark peaked at **2.2 GB** where C used 2.7 MB.

## Runtime

Wall-clock, minimum of 12 runs, milliseconds. Lower is better. Minimum is the reported statistic
because the machine was not quiesced — see [Methodology](#methodology).

| Benchmark | Palladium (as shipped) | Palladium C, `-O2` | C `gcc -O2` | Rust `rustc -O` |
|---|---:|---:|---:|---:|
| fibonacci — `fib(42)`, naive recursion | 732.1 | **383.7** | 374.1 | 374.9 |
| bubble_sort — 45 000 reversed `i64` | 1686.6 | **280.4** | 280.2 | 279.3 |
| matrix_multiply — 200 × (200×200 `i64`) | 1785.9 | **329.9** | 329.9 | 329.4 |
| string_concat — 20 000 concatenations | 245.4 | 242.7 | 55.6 | 36.5 |

Read the third and fourth columns together: on the three numeric benchmarks, Palladium's
optimized number and C's number are identical to within noise, and Rust matches both. For
`fibonacci` this was confirmed at the instruction level — the `fibonacci` function in the
Palladium-derived binary and in the C binary is **byte-identical AArch64 machine code**; the
residual 3% is code placement relative to the 64-byte fetch block.

That is the whole result. Palladium does not make numeric code faster than C, and it does not
make it slower. It emits C.

### Bounds checking is not the difference

Rust's bounds-checked indexing and `get_unchecked` were measured separately: 279.3 ms vs 281.5 ms
on bubble_sort, 329.4 vs 328.6 on matrix_multiply. LLVM hoists the checks out of these loops
entirely. Palladium emits no bounds checks at all and gains nothing measurable for it — so the
safety asymmetry here is real, but it is not a performance argument in either direction.

### Where Palladium actually loses

`string_concat` is the one benchmark where the gap is Palladium's own doing. Same quadratic
algorithm in all three languages, and `-O2` barely helps (245.4 → 242.7 ms) because the workload
is memory-bound, not compute-bound:

| | peak RSS |
|---|---:|
| Palladium | **2 212.6 MB** |
| C | 2.7 MB |

`__pd_alloc_string` bump-allocates from a 64 KiB arena, falls back to `malloc`, tracks only the
first 1 024 allocations, and frees nothing until `atexit`. Every intermediate string in the loop
is retained. (Rust's `String::push_str` variant finishes in 3.1 ms, but that is a *different
algorithm* and must not be quoted as a like-for-like number.)

This is the clearest signal in the data: Palladium's memory model, not its code generation, is
what needs work.

## Compile speed

| Benchmark | `pdc` end-to-end | `pdc` front end only | `rustc -O` | `gcc -O2` |
|---|---:|---:|---:|---:|
| fibonacci | 67.4 | **4.0** | 64.1 | 43.8 |
| bubble_sort | 80.7 | **5.2** | 70.6 | 50.8 |
| matrix_multiply | 99.1 | **6.6** | 70.9 | 49.6 |
| string_concat | 66.2 | **4.2** | 80.1 | 48.6 |

Palladium's own front end is **10–16× faster than rustc**. End-to-end it is *slower* than rustc
on three of four, because ~93% of the time is the forked C compiler — and because `pdc` hands
that compiler a bloated file. An array initializer expands to a literal element list, so
`[0; 40000]` becomes forty thousand zeros: matrix_multiply's 2 KB of Palladium becomes 370 KB of
C.

Binary size: Palladium 55 KB, C 34 KB, Rust 432 KB.

## Feature comparison

Performance is the flattering half. This is the other half.

| | Palladium | Rust |
|---|---|---|
| Memory safety | ownership + borrow checking, no GC | same, plus a type system that models references |
| Reference types | `&T` parses, but the type checker has none — `&T` and `T` are indistinguishable to it | fully typed, with lifetimes |
| Traits | parse and emit **no code**; no dispatch mechanism exists | complete |
| Generics | partial monomorphisation; `Foo<T>` is misparsed as a const argument | complete |
| Closures | none | complete |
| Method syntax `x.f()` | rejected — call `Type::f(x)` | complete |
| `Option` / `Result` | not built in; `?` emits C that does not compile | core to the language |
| Collections | fixed-size arrays only; no `Vec` | full standard library |
| Floats, `char` | none | complete |
| Pattern matching | three pattern forms; cannot match an integer literal | exhaustive, with guards |
| `async` | emits a call to a function that is never generated | mature ecosystem |
| Self-hosting | ✅ verified byte-identical fixed point | ✅ |
| Ecosystem | none | crates.io |

Palladium is an alpha language with a working compiler, a verified self-hosting fixed point, and
roughly the surface area of early C. Rust is a production language. The interesting claim in this
table is not a performance row — it is that a ~1 000-line compiler written in Palladium
reproduces itself byte for byte.

## Methodology

- **Output equivalence is checked before any timing.** Every variant of a benchmark must produce
  byte-identical stdout; the sha256 of each is recorded in the JSON under `output_equivalence`.
  A benchmark whose implementations disagree is void, and the harness refuses to time it.
- **Workloads are sized past 200 ms** so process startup is not what is being measured — except
  `string_concat`, where reaching 200 ms would have required a multi-gigabyte allocation.
- 12 timed runs per target, round-robin across targets, one warm-up discarded. All samples are
  retained in the JSON.
- Rust uses **fixed-size stack arrays, not `Vec`**, to match Palladium's arrays; `get_unchecked`
  variants were measured separately.
- The C sources deliberately do **not** mark benchmark functions `static`. Palladium emits every
  function with external linkage; letting C use `static` was worth 4.9% on matrix_multiply and
  would have been an unearned advantage.

### Caveats that matter

- **The machine was not quiesced.** Load average ran 24–72 during the run. On this
  asymmetric-core CPU the *same binary* measured 318 ms and 740 ms depending on whether it landed
  on a performance or an efficiency core. Minimum is the only statistic worth quoting; three full
  runs agree within 1.1% on every row except `string_concat` (4.2%).
- **`gcc` on this host is Apple clang 21**, not GNU gcc. `pdc` forks whatever `gcc` resolves to,
  so Palladium and C shared a backend — which is what makes the byte-identical-machine-code
  result meaningful, and also means these numbers will not transfer unchanged to a GNU toolchain.
- `hyperfine` was not installed; timing is `perf_counter` around fork/exec/wait.

**Environment**: Apple M5 Max (arm64, 6P + 12E, 128 GB), macOS 26.5.2 · rustc 1.96.1 ·
Apple clang 21.0.0 · pdc at commit `6ffabf2`.

## What this measurement changed

Two defects were found by benchmarking rather than by testing, which is the argument for keeping
benchmarks alongside the other gates:

1. `pdc` never passed `-O` to gcc, and `pdc compile -O` was parsed into a variable named
   `_optimize` that nothing read — a flag that accepted the request and silently ignored it.
2. The string arena's retain-everything behaviour is invisible at test-suite scale and an ~800×
   memory difference at benchmark scale.

Neither would have been caught by `make conformance`, which asks only whether a program produces
the right answer.
