# Palladium Benchmarks

Performance benchmarks comparing Palladium with C and Rust.

## Directory Structure

```
benchmarks/
├── palladium/     # Palladium implementations
├── c/            # Equivalent C implementations
├── rust/         # Equivalent Rust implementations
└── results/      # Benchmark results and analysis
```

## Benchmark Suite

### Core Benchmarks
1. **Fibonacci** - Recursive and iterative implementations
2. **Matrix Multiplication** - Nested loops and memory access
3. **Binary Trees** - Allocation and pointer manipulation
4. **String Processing** - String operations and concatenation
5. **Hash Table** - Data structure implementation

### System Benchmarks
1. **File I/O** - Reading and writing files
2. **Network Echo** - TCP server performance
3. **Threading** - Parallel computation
4. **Memory Allocation** - Allocation patterns

## Running Benchmarks

```bash
# Run all benchmarks
make bench

# Run specific benchmark
make bench-fibonacci

# Compare results
make compare
```

## Metrics

- **Execution Time**: Wall clock time for completion
- **Memory Usage**: Peak memory consumption
- **Binary Size**: Size of compiled executable
- **Compilation Time**: Time to compile from source

## Benchmark Suite

### 1. Fibonacci (fibonacci)
- **Type**: CPU-intensive, recursive
- **Description**: Calculates fibonacci(40) recursively
- **Tests**: Function call overhead, integer arithmetic

### 2. Matrix Multiplication (matrix_multiply)
- **Type**: Memory-intensive, nested loops
- **Description**: Multiplies two 100x100 matrices
- **Tests**: Array access patterns, loop optimization

### 3. String Concatenation (string_concat)
- **Type**: Memory allocation, string operations
- **Description**: Concatenates 1000 strings
- **Tests**: String handling, memory management

### 4. Bubble Sort (bubble_sort)
- **Type**: Array manipulation, comparisons
- **Description**: Sorts 1000 elements (worst case)
- **Tests**: Array operations, conditional branches

## Running Benchmarks

```bash
# Run all benchmarks
make bench

# Run specific benchmarks
make bench-quick  # Only fibonacci and bubble_sort

# Analyze results
make bench-analyze

# Manual execution
cd benchmarks
./run_benchmarks.sh [benchmark_name]
```

## Current Results

Results are automatically saved to `benchmarks/results/` directory.
View the latest report: `benchmarks/results/latest_report.md`

## Goals

- Palladium performance within 10% of C
- Zero-cost abstractions verified
- Predictable performance characteristics