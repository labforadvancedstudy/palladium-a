#!/usr/bin/env python3
"""
Measurement driver for the Palladium benchmark suite.

Invoked by run_benchmarks.sh after every binary has been built and after
output-equivalence has been verified. Does three things:

  1. times each binary N times (round-robin passes, so any machine-load drift
     hits every contestant equally rather than penalising whichever one
     happened to run during a spike),
  2. times each compiler N times on each benchmark source,
  3. writes benchmarks/results/benchmark_<stamp>.{json,csv} plus a
     latest.json / latest.csv symlink-equivalent copy.

hyperfine is used when available; otherwise timing is done here with
time.perf_counter() around subprocess.run(), which is the same measurement
(fork+exec+wait wall clock) and reports min/median/mean/stddev likewise.
No warmup runs are discarded from the statistics; instead the first pass is
run once as a throwaway before the recorded passes begin.
"""

import json
import os
import platform
import re
import shutil
import statistics
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent  # repo root
BENCH = ROOT / "benchmarks"
BIN = BENCH / "build" / "bin"
GEN = BENCH / "build" / "gen"
RESULTS = BENCH / "results"

RUNTIME_RUNS = int(os.environ.get("BENCH_RUNTIME_RUNS", "10"))
COMPILE_RUNS = int(os.environ.get("BENCH_COMPILE_RUNS", "10"))

BENCHMARKS = ["fibonacci", "bubble_sort", "matrix_multiply", "string_concat"]

WORKLOADS = {
    "fibonacci": "fib(42), naive recursion, ~866M calls",
    "bubble_sort": "45000 i64 in reverse order, ~1.01G compare/swap",
    "matrix_multiply": "200 reps of naive 200x200 i64 matmul, 1.6G multiply-adds",
    "string_concat": "20000 iterations, quadratic immutable concat, final length 108895 B",
}

# (variant key, binary suffix, language, note)
RUNTIME_VARIANTS = {
    "fibonacci": [
        ("palladium", "_pd", "Palladium", "pdc compile -o (pdc's own pipeline; gcc invoked with NO -O flag)"),
        ("palladium_gccO2", "_pd_O2", "Palladium", "pdc-generated C, hand-compiled gcc -O2"),
        ("c", "_c", "C", "gcc -O2"),
        ("rust", "_rs", "Rust", "rustc -O"),
    ],
    "bubble_sort": [
        ("palladium", "_pd", "Palladium", "pdc compile -o (gcc with NO -O flag)"),
        ("palladium_gccO2", "_pd_O2", "Palladium", "pdc-generated C, hand-compiled gcc -O2"),
        ("c", "_c", "C", "gcc -O2, fixed stack array"),
        ("rust", "_rs", "Rust", "rustc -O, fixed stack array, bounds-checked indexing"),
        ("rust_unchecked", "_unchecked_rs", "Rust", "rustc -O, fixed stack array, get_unchecked"),
    ],
    "matrix_multiply": [
        ("palladium", "_pd", "Palladium", "pdc compile -o (gcc with NO -O flag)"),
        ("palladium_gccO2", "_pd_O2", "Palladium", "pdc-generated C, hand-compiled gcc -O2"),
        ("c", "_c", "C", "gcc -O2, fixed stack arrays"),
        ("rust", "_rs", "Rust", "rustc -O, fixed stack arrays, bounds-checked indexing"),
        ("rust_unchecked", "_unchecked_rs", "Rust", "rustc -O, fixed stack arrays, get_unchecked"),
    ],
    "string_concat": [
        ("palladium", "_pd", "Palladium", "pdc compile -o (gcc with NO -O flag); runtime arena never frees"),
        ("palladium_gccO2", "_pd_O2", "Palladium", "pdc-generated C, hand-compiled gcc -O2; arena never frees"),
        ("c", "_c", "C", "gcc -O2, same quadratic algorithm, frees the previous buffer"),
        ("rust", "_rs", "Rust", "rustc -O, same quadratic algorithm, previous String dropped"),
        ("rust_pushstr", "_pushstr_rs", "Rust", "rustc -O, IDIOMATIC amortized push_str -- different algorithm, not apples-to-apples"),
    ],
}


def sh(cmd, **kw):
    return subprocess.run(cmd, shell=isinstance(cmd, str), capture_output=True, text=True, **kw)


def stats(samples_ms):
    s = sorted(samples_ms)
    return {
        "runs": len(s),
        "min_ms": round(s[0], 3),
        "median_ms": round(statistics.median(s), 3),
        "mean_ms": round(statistics.fmean(s), 3),
        "max_ms": round(s[-1], 3),
        "stddev_ms": round(statistics.stdev(s), 3) if len(s) > 1 else 0.0,
        "samples_ms": [round(x, 3) for x in s],
    }


def time_once(argv, cwd=None):
    t0 = time.perf_counter()
    p = subprocess.run(argv, cwd=cwd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    t1 = time.perf_counter()
    if p.returncode != 0:
        raise SystemExit(f"FAILED (exit {p.returncode}): {' '.join(map(str, argv))}")
    return (t1 - t0) * 1000.0


def peak_rss_bytes(argv):
    """macOS /usr/bin/time -l reports 'maximum resident set size' in bytes."""
    p = subprocess.run(["/usr/bin/time", "-l"] + [str(a) for a in argv],
                       capture_output=True, text=True)
    m = re.search(r"(\d+)\s+maximum resident set size", p.stderr)
    return int(m.group(1)) if m else None


def sha256_of_output(argv):
    p = subprocess.run([str(a) for a in argv], capture_output=True)
    import hashlib
    return hashlib.sha256(p.stdout).hexdigest()


def loadavg():
    return list(os.getloadavg())


def collect_env():
    def out(cmd):
        r = sh(cmd)
        return r.stdout.strip() if r.returncode == 0 else None

    pdc_ver = out(f"{ROOT}/target/release/pdc --version")
    if pdc_ver:
        pdc_ver = pdc_ver.strip().splitlines()[-1].strip()
    cc_ver = out("gcc --version")
    cc_ver = cc_ver.splitlines()[0] if cc_ver else None
    return {
        "date_local": datetime.now().astimezone().isoformat(timespec="seconds"),
        "date_utc": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "hardware": {
            "uname_m": platform.machine(),
            "cpu_brand": out("sysctl -n machdep.cpu.brand_string"),
            "logical_cpus": out("sysctl -n hw.ncpu"),
            "perf_cores": out("sysctl -n hw.perflevel0.logicalcpu"),
            "efficiency_cores": out("sysctl -n hw.perflevel1.logicalcpu"),
            "memory_bytes": out("sysctl -n hw.memsize"),
            "os": f"{platform.system()} {platform.release()}",
            "macos_product_version": out("sw_vers -productVersion"),
        },
        "toolchains": {
            "pdc": pdc_ver,
            "pdc_git_commit": out(f"git -C {ROOT} rev-parse --short HEAD"),
            "rustc": out("rustc --version"),
            "cc_invoked_as_gcc": cc_ver,
            "hyperfine": out("hyperfine --version") if shutil.which("hyperfine") else None,
        },
        "measurement": {
            "tool": "python time.perf_counter around fork/exec/wait",
            "hyperfine_available": bool(shutil.which("hyperfine")),
            "runtime_runs_per_target": RUNTIME_RUNS,
            "compile_runs_per_target": COMPILE_RUNS,
            "scheduling": "round-robin passes across all targets, 1 discarded warmup run per target",
            "reported": "min/median/mean/max/stddev of wall-clock ms, all samples retained in JSON",
        },
    }


def measure_runtime():
    targets = []
    for b in BENCHMARKS:
        for key, suffix, lang, note in RUNTIME_VARIANTS[b]:
            path = BIN / f"{b}{suffix}"
            if not path.exists():
                raise SystemExit(f"missing binary: {path}")
            targets.append({"benchmark": b, "variant": key, "language": lang,
                            "note": note, "path": path, "samples": []})

    print(f"  timing {len(targets)} binaries x {RUNTIME_RUNS} runs (round-robin)...", flush=True)
    for t in targets:  # warmup, discarded
        time_once([t["path"]])
    for p in range(RUNTIME_RUNS):
        for t in targets:
            t["samples"].append(time_once([t["path"]]))
        print(f"    pass {p + 1}/{RUNTIME_RUNS}", flush=True)

    rows = []
    for t in targets:
        row = {
            "benchmark": t["benchmark"],
            "language": t["language"],
            "variant": t["variant"],
            "note": t["note"],
            "workload": WORKLOADS[t["benchmark"]],
            "binary": t["path"].name,
            "binary_size_bytes": t["path"].stat().st_size,
            "peak_rss_bytes": peak_rss_bytes([t["path"]]),
            "stdout_sha256": sha256_of_output([t["path"]]),
        }
        row.update(stats(t["samples"]))
        rows.append(row)
    return rows


def compile_targets():
    """Each entry: (benchmark, toolchain label, argv, note, artifact path or None)."""
    pdc = str(ROOT / "target" / "release" / "pdc")
    out = BENCH / "build" / "compiletest"
    out.mkdir(parents=True, exist_ok=True)
    t = []
    for b in BENCHMARKS:
        pd = str(BENCH / "palladium" / f"{b}.pd")
        t.append((b, "pdc_full", [pdc, "compile", pd, "-o", str(out / f"{b}_pd")],
                  "pdc frontend + C emission + fork gcc to compile and link (gcc gets NO -O flag)",
                  out / f"{b}_pd"))
        t.append((b, "pdc_emit_c_only", [pdc, "compile", pd],
                  "pdc frontend + C emission only, no gcc fork (isolates pdc's own cost)", None))
        t.append((b, "rustc_O", ["rustc", "-O", str(BENCH / "rust" / f"{b}.rs"), "-o", str(out / f"{b}_rs")],
                  "rustc -O, single file, no cargo", out / f"{b}_rs"))
        t.append((b, "gcc_O2", ["gcc", "-O2", str(BENCH / "c" / f"{b}.c"), "-o", str(out / f"{b}_c")],
                  "gcc -O2 on the hand-written C", out / f"{b}_c"))
        t.append((b, "gcc_O2_on_pdc_output", ["gcc", "-O2", str(GEN / f"{b}.c"),
                                              str(ROOT / "runtime" / "palladium_runtime.c"),
                                              "-o", str(out / f"{b}_pdgen")],
                  "gcc -O2 on the C that pdc emitted (shows how much of pdc_full is really gcc)",
                  out / f"{b}_pdgen"))
    return t


def measure_compile():
    targets = [{"benchmark": b, "toolchain": tc, "argv": argv, "note": note,
                "artifact": art, "samples": []}
               for (b, tc, argv, note, art) in compile_targets()]

    print(f"  timing {len(targets)} compile commands x {COMPILE_RUNS} runs (round-robin)...", flush=True)
    for t in targets:
        time_once(t["argv"], cwd=ROOT)  # warmup, discarded
    for p in range(COMPILE_RUNS):
        for t in targets:
            t["samples"].append(time_once(t["argv"], cwd=ROOT))
        print(f"    pass {p + 1}/{COMPILE_RUNS}", flush=True)

    rows = []
    for t in targets:
        src_map = {
            "pdc_full": BENCH / "palladium" / f"{t['benchmark']}.pd",
            "pdc_emit_c_only": BENCH / "palladium" / f"{t['benchmark']}.pd",
            "rustc_O": BENCH / "rust" / f"{t['benchmark']}.rs",
            "gcc_O2": BENCH / "c" / f"{t['benchmark']}.c",
            "gcc_O2_on_pdc_output": GEN / f"{t['benchmark']}.c",
        }
        src = src_map[t["toolchain"]]
        row = {
            "benchmark": t["benchmark"],
            "toolchain": t["toolchain"],
            "command": " ".join(str(a) for a in t["argv"]).replace(str(ROOT) + "/", ""),
            "note": t["note"],
            "source": str(src.relative_to(ROOT)),
            "source_bytes": src.stat().st_size,
            "source_lines": len(src.read_text(errors="replace").splitlines()),
            "artifact_size_bytes": t["artifact"].stat().st_size if t["artifact"] and t["artifact"].exists() else None,
        }
        row.update(stats(t["samples"]))
        rows.append(row)
    return rows


CSV_RUNTIME_COLS = ["benchmark", "language", "variant", "workload", "runs",
                    "min_ms", "median_ms", "mean_ms", "max_ms", "stddev_ms",
                    "binary_size_bytes", "peak_rss_bytes", "stdout_sha256", "note"]
CSV_COMPILE_COLS = ["benchmark", "toolchain", "source", "source_lines", "source_bytes",
                    "runs", "min_ms", "median_ms", "mean_ms", "max_ms", "stddev_ms",
                    "artifact_size_bytes", "note"]


def write_csv(path, runtime_rows, compile_rows):
    import csv
    with path.open("w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["section"] + CSV_RUNTIME_COLS)
        for r in runtime_rows:
            w.writerow(["runtime"] + [r.get(c, "") for c in CSV_RUNTIME_COLS])
        w.writerow([])
        w.writerow(["section"] + CSV_COMPILE_COLS)
        for r in compile_rows:
            w.writerow(["compile"] + [r.get(c, "") for c in CSV_COMPILE_COLS])


def main():
    RESULTS.mkdir(parents=True, exist_ok=True)
    env = collect_env()
    env["load_average_start"] = loadavg()

    equiv = json.loads((BENCH / "build" / "equivalence.json").read_text())

    print("== runtime")
    runtime_rows = measure_runtime()
    print("== compile")
    compile_rows = measure_compile()

    env["load_average_end"] = loadavg()

    doc = {
        "schema": "palladium-benchmark-v1",
        "environment": env,
        "output_equivalence": equiv,
        "caveats": [
            "The machine was NOT quiesced. See environment.load_average_start/end; on a loaded "
            "box the min is the least-contaminated estimate and mean/max carry the contention. "
            "stddev_ms is reported for every row so the noise is visible.",
            "This host has ASYMMETRIC cores (6 performance + 12 efficiency). Under load the "
            "scheduler will place a benchmark process on an efficiency core, and the SAME binary "
            "then measures ~2x slower: matrix_multiply_c was observed at 318ms and at 740ms in "
            "the same session. Min-of-N is the only statistic that reliably reflects a "
            "performance-core run; treat mean/max as contention, not as language behaviour.",
            "The C sources deliberately do NOT mark the benchmark functions `static`. Palladium's "
            "codegen emits every user function with external linkage; letting clang specialise a "
            "`static` C helper into main was measured to be worth 4.9% on matrix_multiply "
            "(318.2ms static vs 334.7ms extern, min of 12 interleaved runs). Matching linkage "
            "keeps the comparison about code generation rather than about one C keyword.",
            "`gcc` on this host is Apple clang, not GNU gcc (see toolchains.cc_invoked_as_gcc). "
            "pdc forks whatever `gcc` resolves to, so Palladium and C are going through the same "
            "backend compiler.",
            "variant=palladium is pdc's own pipeline: src/main.rs:99-105 forks gcc with NO "
            "optimization flag, so the binary a Palladium user actually gets is -O0. pdc's "
            "-O/--optimize flag is parsed and then dropped on the floor (src/main.rs:76, the "
            "parameter is bound as `_optimize` and never read).",
            "variant=palladium_gccO2 is the SAME pdc-generated C recompiled by hand with gcc -O2. "
            "It is not something pdc can produce today; it is the ceiling of the C backend.",
            "Rust here uses fixed-size stack arrays, not Vec, to match Palladium's [i64; N] "
            "memory model. Both a bounds-checked and a get_unchecked variant are measured.",
            "Palladium emits no bounds checks at all; that is a safety difference, not just a "
            "performance one.",
            "string_concat: Palladium's runtime arena (__pd_alloc_string) never frees before "
            "exit, so its RSS is ~2.3GB for this workload while C/Rust stay near zero. Compare "
            "the peak_rss_bytes column, not just the time.",
            "string_concat variant=rust_pushstr is a DIFFERENT algorithm (amortized append vs "
            "quadratic copy). It is reported for context and must not be quoted as a "
            "like-for-like Palladium-vs-Rust number. It also finishes in ~3ms, far below the "
            "~200ms floor the other workloads were sized to clear, so that row is mostly "
            "process startup and its run-to-run spread (~7%) is correspondingly large.",
            "On fibonacci the pdc-generated `fibonacci` and the hand-written C `fibonacci` "
            "compile to BYTE-IDENTICAL AArch64 machine code (verified with objdump "
            "--disassemble-symbols=_fibonacci; only the absolute branch-target addresses "
            "printed by the disassembler differ). The residual ~3% gap between variant=c and "
            "variant=palladium_gccO2 is therefore code placement -- the two symbols land at "
            "different offsets modulo the 64-byte fetch block (16 vs 32) -- and not a "
            "difference in generated code quality.",
            "pdc writes its generated C to the repo-global build_output/<stem>.c, keyed only by "
            "file stem. Concurrent compiles of a same-named file elsewhere in the repo will "
            "clobber it; run_benchmarks.sh copies and content-verifies immediately after each "
            "compile to detect that.",
        ],
        "runtime": runtime_rows,
        "compile": compile_rows,
    }

    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    jpath = RESULTS / f"benchmark_{stamp}.json"
    cpath = RESULTS / f"benchmark_{stamp}.csv"
    jpath.write_text(json.dumps(doc, indent=2) + "\n")
    write_csv(cpath, runtime_rows, compile_rows)
    (RESULTS / "latest.json").write_text(json.dumps(doc, indent=2) + "\n")
    write_csv(RESULTS / "latest.csv", runtime_rows, compile_rows)

    print()
    print(f"wrote {jpath.relative_to(ROOT)}")
    print(f"wrote {cpath.relative_to(ROOT)}")
    print(f"wrote {(RESULTS / 'latest.json').relative_to(ROOT)} / latest.csv")
    print()
    print_tables(doc)


def print_tables(doc):
    print("=" * 100)
    print("RUNTIME  (wall-clock ms, lower is better)")
    print("=" * 100)
    hdr = f"{'benchmark':<17}{'variant':<18}{'min':>9}{'median':>9}{'mean':>9}{'stddev':>9}{'bin KB':>9}{'peak RSS MB':>13}"
    for b in BENCHMARKS:
        print(f"\n-- {b}: {WORKLOADS[b]}")
        print(hdr)
        base = None
        for r in doc["runtime"]:
            if r["benchmark"] != b:
                continue
            if base is None:
                base = r["min_ms"]
            rss = r["peak_rss_bytes"]
            print(f"{r['benchmark']:<17}{r['variant']:<18}{r['min_ms']:>9.1f}{r['median_ms']:>9.1f}"
                  f"{r['mean_ms']:>9.1f}{r['stddev_ms']:>9.1f}"
                  f"{r['binary_size_bytes'] / 1024:>9.0f}"
                  f"{(rss / 1048576 if rss else 0):>13.1f}")

    print()
    print("=" * 100)
    print("COMPILE  (wall-clock ms, lower is better)")
    print("=" * 100)
    hdr = f"{'benchmark':<17}{'toolchain':<24}{'min':>9}{'median':>9}{'mean':>9}{'stddev':>9}{'src lines':>11}"
    for b in BENCHMARKS:
        print(f"\n-- {b}")
        print(hdr)
        for r in doc["compile"]:
            if r["benchmark"] != b:
                continue
            print(f"{r['benchmark']:<17}{r['toolchain']:<24}{r['min_ms']:>9.1f}{r['median_ms']:>9.1f}"
                  f"{r['mean_ms']:>9.1f}{r['stddev_ms']:>9.1f}{r['source_lines']:>11}")

    la = doc["environment"]
    print()
    print(f"load average start={['%.2f' % x for x in la['load_average_start']]} "
          f"end={['%.2f' % x for x in la['load_average_end']]}  "
          "-- the machine was not quiesced; min is the least-contaminated estimate")


if __name__ == "__main__":
    main()
