# Palladium vs Rust — measured

**Date**: 2026-08-22 · **Data**: [`benchmarks/results/latest.json`](../../benchmarks/results/latest.json)
· **Reproduce**: `bash benchmarks/run_benchmarks.sh` · **Measured at** `e72c39b`

The previous version of this document compared a splay-tree implementation written in a language
that did not exist — `pdc translate --from-rust`, `pdc --verify-total`, implicit smart pointers,
none of which are implemented — and concluded that Palladium won. This version reports what four
benchmarks actually did on one machine, with the ways the comparison is unfair stated rather than
buried.

## The short version

On numeric code, **Palladium's runtime performance is the C compiler's performance** — it matches
C and Rust to within measurement noise. The generated C produces machine code byte-identical to
hand-written C. That is the honest ceiling, and Palladium reaches it.

On string-heavy code Palladium is **4× slower than C**, and that one *is* Palladium's fault: the
string arena never frees, so a quadratic concatenation benchmark peaks at 2.2 GB where C uses
2.7 MB.

> These numbers were taken after fixing a defect this benchmark suite found: `pdc` forked `gcc`
> with no `-O` flag, so every shipped binary was `-O0`, and `pdc compile -O` was parsed into a
> variable nothing read. That was a 2–6× tax with nothing to do with the language. The
> before/after is in [What this measurement changed](#what-this-measurement-changed).

## Runtime

Wall-clock, minimum of 12 runs, milliseconds. Lower is better. Minimum is the reported statistic
because the machine was not quiesced — see [Methodology](#methodology).

| Benchmark | Palladium | C `gcc -O2` | Rust `rustc -O` | Rust unchecked |
|---|---:|---:|---:|---:|
| fibonacci — `fib(42)`, naive recursion | 395.9 | 380.7 | 380.9 | — |
| bubble_sort — 45 000 reversed `i64` | **282.4** | 285.2 | 283.8 | 284.5 |
| matrix_multiply — 200 × (200×200 `i64`) | 333.3 | 331.8 | **316.0** | 315.7 |
| string_concat — 20 000 concatenations | 230.8 | 57.0 | 38.1 | 2.9 † |

† `rust_pushstr` is a *different algorithm* (amortised append, not quadratic concatenation) and
sits below the process-startup floor. It is here to show what the workload costs when you are not
required to allocate a fresh string per step — not as a like-for-like number.

On the three numeric benchmarks Palladium, C and Rust are the same to within noise — Palladium is
1.0% *faster* than C on bubble_sort and 4% slower on fibonacci, which is the spread you get from
re-running any of them. For `fibonacci` the identity was confirmed at the instruction level: the
`fibonacci` function in the Palladium-derived binary and in the C binary is **byte-identical
AArch64 machine code**, and the residual difference is code placement relative to the 64-byte
fetch block.

That is the whole result. Palladium does not make numeric code faster than C, and it does not
make it slower. It emits C. Rust's 5% edge on matrix_multiply is the one place LLVM's optimizer
beat clang's on identical arithmetic.

### Bounds checking is not the difference

Rust's bounds-checked indexing and `get_unchecked` were measured separately: 283.8 ms vs 284.5 ms
on bubble_sort, 316.0 vs 315.7 on matrix_multiply. LLVM hoists the checks out of these loops
entirely. Palladium emits no bounds checks at all and gains nothing measurable for it — so the
safety asymmetry here is real, but it is not a performance argument in either direction.

### Where Palladium actually loses

`string_concat` is the one benchmark where the gap is Palladium's own doing: **230.8 ms against
C's 57.0 ms**, on the same quadratic algorithm. Optimization does not help it — the workload is
memory-bound, not compute-bound:

| | peak RSS |
|---|---:|
| Palladium | **2 212.6 MB** |
| C | 2.7 MB |

`__pd_alloc_string` bump-allocates from a 64 KiB arena, falls back to `malloc`, tracks only the
first 1 024 allocations, and frees nothing until `atexit`. Every intermediate string in the loop
is retained. (Rust's `String::push_str` variant finishes in 2.9 ms, but that is a *different algorithm* —
see the footnote on the runtime table.)

This is the clearest signal in the data: Palladium's memory model, not its code generation, is
what needs work.

## Compile speed

| Benchmark | `pdc` end-to-end | `pdc` front end only | `rustc -O` | `gcc -O2` |
|---|---:|---:|---:|---:|
| fibonacci | 92.1 | **3.9** | 66.7 | 45.0 |
| bubble_sort | 111.5 | **5.4** | 74.5 | 52.5 |
| matrix_multiply | 130.6 | **6.7** | 75.2 | 51.0 |
| string_concat | 93.0 | **4.3** | 87.4 | 49.5 |

Palladium's own front end is **11–19× faster than rustc** (3.9–6.7 ms against 66.7–87.4 ms).
End-to-end it is *slower* than rustc on all four, because ~95% of the time is the forked C
compiler — and because `pdc` hands that compiler a bloated file. An array initializer expands to a
literal element list, so `[0; 40000]` becomes forty thousand zeros: matrix_multiply's 2 KB of
Palladium becomes 370 KB of C, which takes gcc 125 ms to chew through.

Compile time went *up* with the `-O2` default, which is the expected trade and is visible here:
gcc doing real optimization is most of the wall clock.

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
| `Option` / `Result` | not built in; `?` rejected as unimplemented | core to the language |
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
Apple clang 21.0.0 · pdc at commit `e72c39b`. Load average 47–55 throughout.

## What this measurement changed

Two defects were found by benchmarking rather than by testing, which is the argument for keeping
benchmarks alongside the other gates:

1. `pdc` never passed `-O` to gcc, and `pdc compile -O` was parsed into a variable named
   `_optimize` that nothing read — a flag that accepted the request and silently ignored it.
   Fixing it is what moved these numbers, and the size of the effect is the reason to state it
   plainly:

   | Benchmark | before (`-O0`) | after (`-O2` default) |
   |---|---:|---:|
   | bubble_sort | 1686.6 | 282.4 |
   | matrix_multiply | 1785.9 | 333.3 |
   | fibonacci | 732.1 | 395.9 |

   Self-hosting was proven invariant under the change: stage 1 built at `-O0`, `-O2` and `-O3`
   emits byte-identical C.
2. The string arena's retain-everything behaviour is invisible at test-suite scale and an ~800×
   memory difference at benchmark scale.

Neither would have been caught by `make conformance`, which asks only whether a program produces
the right answer.
