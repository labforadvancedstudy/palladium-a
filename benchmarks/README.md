# Palladium Benchmarks

Runtime and compile-speed comparison of **Palladium vs Rust vs C**, on identical
algorithms with byte-identical output.

## One command

```bash
bash benchmarks/run_benchmarks.sh
```

It wipes `benchmarks/build/`, rebuilds every implementation from source, refuses
to time anything until all implementations of a benchmark print identical bytes,
then writes raw data to `benchmarks/results/benchmark_<stamp>.json` (+ `.csv`,
+ `latest.json` / `latest.csv`).

Knobs: `BENCH_RUNTIME_RUNS` (default 10), `BENCH_COMPILE_RUNS` (default 10).

## Layout

```
benchmarks/
├── palladium/     4 benchmarks in the PBS-1 subset (docs/specification/bootstrap-subset.md)
├── c/             hand-written C, same algorithm       -> gcc -O2
├── rust/          hand-written Rust, same algorithm    -> rustc -O
│                  (+ *_unchecked / *_pushstr fairness variants)
│                  compiler_bench.rs is unrelated: it is the criterion
│                  [[bench]] target declared in Cargo.toml, run by `cargo bench`
├── build/         generated, gitignored, wiped on every run
├── results/       raw data (JSON + CSV)
├── measure.py     timing + statistics + JSON/CSV emission
└── run_benchmarks.sh
```

## The four benchmarks

| Benchmark | Workload | Exercises |
|---|---|---|
| `fibonacci` | naive recursive fib(42), ~866M calls | function-call overhead, integer arithmetic |
| `bubble_sort` | 45000 i64 reverse-ordered, ~1.01G compare/swap | array indexing, branches, swaps |
| `matrix_multiply` | 200 reps of naive 200x200 i64 matmul, 1.6G MACs | nested loops, strided access |
| `string_concat` | 20000 quadratic immutable concats, final length 108895 B | string builtins, allocation strategy |

Sizes were chosen so that **every** variant runs >=200ms, including the fastest
one, so that process startup is not what is being measured. The one exception is
noted below.

## Variants, and why each exists

| Variant | What it is |
|---|---|
| `palladium` | `pdc compile X.pd -o Y` — **what a Palladium user actually gets today.** pdc forks gcc with *no* optimization flag (`src/main.rs:99-105`), so this is an `-O0` binary. `pdc -O` is parsed and then ignored (`src/main.rs:76`). |
| `palladium_gccO2` | the *same* pdc-generated C, hand-recompiled at `gcc -O2`. The ceiling of the C backend; pdc cannot produce this today. |
| `c` | hand-written C, `gcc -O2`. |
| `rust` | hand-written Rust, `rustc -O`, fixed-size stack arrays (not `Vec`), bounds-checked. |
| `rust_unchecked` | same with `get_unchecked`, since Palladium emits no bounds checks at all. |
| `rust_pushstr` | idiomatic amortized `push_str`. **A different algorithm** — context only, not a like-for-like number. |

## Reading the numbers honestly

- **Use `min_ms`.** This host has 6 performance + 12 efficiency cores; under load
  the scheduler will drop a process onto an efficiency core and the same binary
  measures ~2x slower. `mean`/`max`/`stddev` are reported so contention is
  visible, but they are contention, not language behaviour.
- The machine was not quiesced; `environment.load_average_start/end` is recorded
  in every result file.
- `gcc` on macOS is Apple clang. Palladium and C therefore share a backend
  compiler, which is exactly the point of the `palladium` vs `c` column.
- The C sources are deliberately **not** `static`, to match the external linkage
  Palladium's codegen emits — `static` alone was worth 4.9% on `matrix_multiply`.
- `string_concat` is the one case where a fair >=200ms workload was impossible
  without a memory blowup: Palladium's `__pd_alloc_string` never frees before
  exit, so total allocation tracks total bytes copied. At 20000 iterations
  Palladium peaks at ~2.2GB RSS while C and Rust stay under 3MB. Compare the
  `peak_rss_bytes` column, not just the time.

Every caveat is also carried inside each result file under `caveats`, so the raw
data cannot be quoted without them.

## Note on `analyze_results.py`

Superseded by `measure.py`. It predates this harness, only models C vs Palladium,
and is not called by anything.
