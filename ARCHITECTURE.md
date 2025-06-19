# Palladium Architecture

## Overview

Palladium is a systems programming language designed to combine Turing's correctness with von Neumann's performance. This document describes the project architecture and organization.

## Directory Structure

```
palladium/
├── compiler/           # Compiler implementations
│   ├── rust/          # Production Rust compiler
│   ├── palladium/     # Self-hosting Palladium compiler
│   └── bootstrap/     # Historical bootstrap compilers
├── stdlib/            # Standard library
├── examples/          # Example programs
├── tests/            # Test suite
├── benchmarks/       # Performance benchmarks
├── tools/           # Development tools
├── docs/            # Documentation
├── reports/         # Project status reports
├── scripts/         # Build and utility scripts
└── build/           # Build outputs (git-ignored)
```

## Core Components

### 1. Compiler Pipeline

```
Source Code → Lexer → Parser → Type Checker → Code Generator → Output
     ↓          ↓        ↓           ↓              ↓            ↓
   .pd file   Tokens    AST     Typed AST      C/LLVM IR   Executable
```

### 2. Type System

- **Primitive Types**: i8-i64, u8-u64, f32, f64, bool, char, String
- **Compound Types**: Arrays, Tuples, Structs, Enums
- **Advanced Types**: References (&T, &mut T), Generics, Traits
- **Type Inference**: Hindley-Milner style with bidirectional checking

### 3. Memory Management

- **Ownership**: Single owner principle
- **Borrowing**: Immutable and mutable references
- **Lifetimes**: Explicit and inferred
- **No GC**: Deterministic memory management

### 4. Compilation Targets

- **C Backend**: Portable C code generation (current)
- **LLVM Backend**: Native code via LLVM (planned)
- **WASM**: WebAssembly support (future)

## Key Design Decisions

### 1. Bootstrap Strategy

Three successful approaches demonstrate flexibility:
- **Full Compiler**: Complete implementation (1,220 lines)
- **Incremental**: Step-by-step feature addition (760 lines)
- **Self-Hosting**: Production compiler in Palladium (5,947 lines)

### 2. Syntax Choices

- **Rust-like**: Familiar to systems programmers
- **Expression-based**: Everything returns a value
- **Pattern Matching**: Exhaustive by default
- **Explicit Mutability**: `let mut` for mutable bindings

### 3. Error Handling

- **Result Type**: `Result<T, E>` for recoverable errors
- **Panic**: For unrecoverable errors
- **? Operator**: Error propagation sugar
- **Exhaustive Matching**: Compiler enforces handling

### 4. Async Model

- **Effects System**: Async as an effect
- **No Function Coloring**: Regular functions can call async
- **Structured Concurrency**: Automatic cancellation
- **Zero-Cost**: No runtime overhead

## Development Workflow

### 1. Building

```bash
# Build Rust compiler
cd compiler/rust && cargo build --release

# Build with self-hosting compiler
compiler/palladium/pdc input.pd -o output.c
gcc output.c -o output

# Run tests
make test
```

### 2. Testing

- **Unit Tests**: In each module
- **Integration Tests**: In tests/
- **Examples**: Serve as documentation and tests
- **Benchmarks**: Track performance

### 3. Contributing

1. Fork the repository
2. Create feature branch
3. Write tests first
4. Implement feature
5. Update documentation
6. Submit pull request

## Performance Goals

- **Compilation Speed**: < 10K LOC/second
- **Runtime Performance**: Within 5% of C
- **Memory Usage**: Predictable and minimal
- **Binary Size**: Comparable to C

## Future Architecture

### Phase 1: Performance (Q1 2025)
- LLVM backend implementation
- Optimization passes
- Native intrinsics

### Phase 2: Expressiveness (Q2 2025)
- Complete trait system
- Generic specialization
- Const generics

### Phase 3: Revolutionary (Q3 2025)
- Effects system
- Async without coloring
- Compile-time verification

### Phase 4: Production (Q4 2025)
- Package manager
- IDE support
- 1.0 release

## Technical Debt

1. **C Backend Limitations**: Need LLVM for optimizations
2. **Fixed Arrays**: Dynamic allocation needed
3. **Module System**: Import resolution incomplete
4. **Error Messages**: Need improvement

## Security Considerations

- **Memory Safety**: Enforced by type system
- **No Undefined Behavior**: In safe code
- **Unsafe Blocks**: Explicit and auditable
- **Supply Chain**: Planned security for package manager

## Conclusion

Palladium's architecture prioritizes correctness, performance, and developer experience. The successful bootstrap demonstrates the soundness of the design, while the roadmap shows the path to production readiness.