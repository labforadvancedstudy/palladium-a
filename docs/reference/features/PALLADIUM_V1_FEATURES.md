> **NORMATIVE — this is what Palladium is defined to be.** It is not a description of what
> `pdc` implements today. What is implemented, partial, or unimplemented is recorded per
> specification section in the
> [implementation status annex](../../specification/language-spec.md#part-ii-implementation-status-annex),
> and per feature in [`status.yaml`](status.yaml). Palladium blocks below are fenced `no-compile`:
> the syntax is normative, the compiler does not accept all of it yet, and
> `scripts/check-docs.sh` counts each fence rather than hiding it.

# Palladium v1.0 Feature List

> The feature definition for the v1.0 release.
> Originally written 2025-01-20; status claims removed and normative framing added 2026-08-22.

## Overview

Palladium is a systems programming language that combines Turing's correctness with von Neumann's performance. This document defines the features that constitute v1.0, by area.

## How to read this document

This file names and describes features. It deliberately carries **no completion percentages and no
per-feature status marks**. The earlier version carried both — "Ownership System (95% Complete)",
"Generics (85% Complete)", "Traits (70% Complete)", "Overall: ~60% complete" — and none of them
came from a measurement. They are deleted, not adjusted.

Status lives in exactly two places, both of which cite the compiler:

- [`status.yaml`](status.yaml) — one row per feature: the spec section that defines it, and the
  evidence for what `pdc` does with it (a conformance test, a source location, or `unimplemented`).
- [The implementation status annex](../../specification/language-spec.md#part-ii-implementation-status-annex)
  — the same information organised by specification section, with the failure mode of each partial
  construct.

---

## 1. Core Language Features

### 1.1 Memory Management

#### Ownership System
- Rust-compatible ownership model
- Move semantics by default
- Borrowing rules enforced at compile time
- Zero runtime overhead

#### Reference Syntax
- `ref` and `ref mut` instead of `&` and `&mut`
- Cleaner syntax: `fn process(data: ref Data)` rather than `fn process(data: &Data)`

#### Implicit Lifetimes
- Lifetimes inferred; no `'a` parameter lists on functions, structs or impls
- Explicit `ref<'a> T` only where inference reports ambiguity, and ambiguity is an error rather
  than a guess
- Definition: [implicit-lifetimes.md](core-language/implicit-lifetimes.md)

#### Unsafe Blocks
- Restricted unsafe with side-channel protection
- Compile-time verification of unsafe invariants
- Not permitted inside a `#![total(strict)]` crate

### 1.2 Type System

#### Type Inference
- Hindley-Milner type inference with extensions
- Local type inference within functions
- Minimal type annotations required
- Example: `let x = 42;` infers `i64`

#### Primitive Types
- Integers: `i32`, `i64`, `u32`, `u64`
- Boolean: `bool`
- String: `String` (heap-allocated, UTF-8)
- Unit type: `()` for void returns
- Arrays: `[T; N]` with compile-time size

#### Compound Types
- Structs with named fields
- Enums with unit, tuple, and struct variants
- Tuples (unnamed product types)
- Arrays with fixed size

#### Generics
- Monomorphization-based generics
- Type parameters: `fn identity<T>(x: T) -> T`
- Generic structs and enums
- Where clauses for constraints
- Design: [`docs/design/generics.md`](../../design/generics.md)

#### Traits
- Simplified trait system
- Trait implementations
- Trait bounds on generics
- Associated types
- Design: [`docs/design/trait_system_design.md`](../../design/trait_system_design.md)

#### Const Generics
- Compile-time generic parameters
- Arrays with generic sizes: `struct Buffer<const N: usize> { data: [u8; N] }`

### 1.3 Control Flow

#### Pattern Matching
- Match expressions with exhaustiveness checking
- Enum destructuring
- Struct pattern matching
- Wildcard patterns `_`
- Guard clauses in patterns

#### Loops
- `for` loops with iterator protocol
- `while` loops with conditions
- `loop` for infinite loops
- `break` and `continue` statements

#### Conditionals
- `if`/`else` expressions
- Pattern matching as primary branching mechanism
- No ternary operator (use if expressions)

### 1.4 Functions

#### Function Definitions
- Named parameters with types
- Optional return type annotation
- Expression-oriented (implicit returns)
- Nested function definitions

#### Closures
- Anonymous functions with capture
- Automatic capture mode inference
- Move closures for ownership transfer

### 1.5 Modules & Imports

#### Module System
- File-based modules
- Nested module paths
- Public/private visibility
- Import statements: `import std::math;`
- Design: [`docs/design/module-system.md`](../../design/module-system.md)

#### Module Resolution
- Path-based imports
- Wildcard imports
- Selective imports: `import std::io::{read, write};`
- Module aliasing: `import std::collections as col;`

---

## 2. Error Handling

#### Result Type
- Built-in `Result<T, E>` type
- Ok/Err variants for success/failure
- Composable error handling

#### Question Mark Operator
- `?` for error propagation
- Automatic conversion between error types
- Early return on errors

#### Try Blocks
- `try { ... }` expressions for scoped error handling
- Catch and transform errors locally

---

## 3. Async & Effects System

Definition: [async-as-effect.md](async-system/async-as-effect.md).

#### Async as Effect
- Async is an algebraic effect, not a function color
- No `async` keyword and no `.await` operator exist in the language
- Effects are inferred from a body and propagated transitively to callers
- Effects compose; independent effectful operations are parallel by default

#### No Await Syntax
- The compiler places async boundaries; sync and async code interoperate without ceremony
- No function coloring problem

#### Effect Contexts
- `with_timeout(5.seconds) { with_retry(3) { ... } }` — timeout, retry and tracing scope over a
  block instead of being threaded through every call
- Escape hatches: `effect::sync { }` forces sequencing, `-> async T` pins a boundary

#### Structured Concurrency
- Scoped task management
- Automatic cancellation
- No orphaned tasks

#### Effect System (beyond async)
- Pluggable effects: IO, memory, panic, unsafe
- Pure function guarantees
- Effect polymorphism

---

## 4. Advanced Features

### 4.1 Verification

#### Totality Checking
- Prove functions terminate
- Structural recursion verification
- Well-founded recursion with `#[decreases(expr)]` measures
- Fuel-based termination for complex cases
- `#[total]` per function, `#![total(strict)]` per crate
- Definition: [totality-checking.md](advanced/totality-checking.md)

#### Refinement Types
- Types with predicates
- Compile-time constraint checking
- Example: `type PositiveInt = i32 where self > 0`

#### Proof Generation
- Export proofs to Lean/Coq
- Formal verification integration
- Machine-checkable correctness proofs

#### Side-Channel Safety
- Constant-time guarantees
- No timing attacks possible
- Cryptography-safe operations

### 4.2 Metaprogramming

#### Unified Macro System
- Single macro system (no `macro_rules!`/proc-macro split)
- Hygienic by default
- Pattern-based macros
- Syntax extensions

#### Compile-Time Execution
- Const functions
- Compile-time evaluation
- Static assertions

---

## 5. Standard Library

#### Core Module
- Basic types and traits
- Memory utilities
- Primitive operations
- Iterator protocol

#### Collections
- `Vec<T>` — dynamic arrays
- `HashMap<K, V>` — hash tables
- `String` — UTF-8 strings
- `Option<T>` — optional values

#### IO Module
- File I/O abstractions
- Network operations
- Buffered I/O
- Async I/O support

#### Math Module
- Basic math functions
- Trigonometry
- Power operations
- Min/max utilities

#### String Module
- String manipulation
- Pattern matching
- UTF-8 operations
- String builders

---

## 6. Compilation & Optimization

#### Incremental Compilation
- Function-level incremental builds
- Dependency tracking
- Fast recompilation

#### Parallel Compilation
- Multi-threaded compilation pipeline
- Parallel type checking
- Concurrent code generation

#### LLVM Backend
- LLVM IR generation
- Optimization passes
- Native code generation
- Link-time optimization

#### C Backend
- C code generation for bootstrapping
- Portable C output
- Integration with C toolchains

---

## 7. Developer Tools

### 7.1 Compiler

#### pdc — Palladium Compiler
- The compiler
- Self-hosting: written in Palladium, compiling itself to a fixed point
- Multi-backend support
- Comprehensive error messages

### 7.2 Development Tools

#### pdfmt — Code Formatter
- Automatic code formatting
- Configurable style rules
- IDE integration

#### pls — Language Server
- LSP protocol implementation
- Code completion
- Go to definition
- Real-time diagnostics

#### Debugger Support
- GDB/LLDB integration
- Source-level debugging
- Async debugging support

### 7.3 Package Management

#### Cargo Compatibility
- Read Cargo.toml files
- Compatible dependency resolution
- Rust crate interop

#### Package Registry
- Central package repository
- Version management
- Dependency resolution

---

## 8. Interoperability

#### Rust FFI
- Call Rust code from Palladium
- Share data structures
- Zero-cost interop

#### C FFI
- C ABI compatibility
- Call C libraries
- Export Palladium functions to C

#### WebAssembly Target
- Compile to WASM
- Browser integration
- WASI support

---

## 9. Unique Palladium Features

### 9.1 Syntax Improvements

#### Cleaner Error Propagation
```palladium no-compile
// Natural ? operator usage
let root = self.root.take()?;
```

#### Simplified References
```palladium no-compile
// ref keyword instead of & and &mut
fn process(data: ref Data) -> ref str {
    return data.name;
}
```

#### Direct Pattern Matching
```palladium no-compile
// No need to import enum variants
match result {
    Ok(value) => println!("Success: {}", value),
    Err(msg) => println!("Error: {}", msg),
}
```

### 9.2 Compiler Intelligence

#### Automatic Memory Strategy
- Compiler infers when to use stack vs heap
- No explicit `Box<T>` needed in most cases
- Smart pointer inference

#### Effect Inference
- Automatic async propagation
- Pure function detection
- Side effect tracking

### 9.3 Performance Features

#### Automatic Parallelization
- Compiler detects independent operations
- Parallel execution without explicit threading
- Data parallelism for collections

#### Compile-Time Optimization
- Aggressive inlining
- Const propagation
- Dead code elimination

---

## 10. Philosophy & Design Principles

### Core Principles

1. **No Compromise**: Safety, speed, and elegance coexist
2. **Proofs Over Tests**: Correct by construction
3. **Zero Cost**: Abstractions compile away completely
4. **Explicit Over Magic**: No hidden allocations
5. **Learn From Giants**: Best ideas from Rust, OCaml, Haskell, C

### Design Goals

- **Memory Safety**: Without garbage collection
- **Type Safety**: Strong static typing with inference
- **Performance**: Match or exceed C performance
- **Ergonomics**: Reduce boilerplate and cognitive load
- **Correctness**: Optional formal verification

---

## Unique Advantages Over Rust

These are the differentiators — the reasons for the language to exist rather than to be a Rust
dialect. The first three each have their own normative document.

1. **Implicit lifetimes** — [definition](core-language/implicit-lifetimes.md); no `'a` parameter
   lists, and inference failure is an error rather than a guess
2. **Async without coloring** — [definition](async-system/async-as-effect.md); no `async`, no
   `.await`, effects inferred and propagated
3. **Totality checking** — [definition](advanced/totality-checking.md); `#[total]`,
   `#[decreases(expr)]`, `#![total(strict)]`
4. Unified macro system
5. Cleaner syntax overall
6. Built-in verification support
7. Automatic parallelization
8. Effect system

---

## What was removed from this document, and why

Three sections are gone. Each asserted progress rather than defining a feature, and each was false
when it was written:

- **"Feature Status Legend"**, and the per-heading ✅/⏳/🔲 marks and percentages that used it.
  Superseded by [`status.yaml`](status.yaml) and the annex, which carry evidence.
- **"Implementation Roadmap"** ("Phase 1: Core Language (✅ Complete)", "Phase 2: Self-Hosting
  (✅ Complete)", …). Sequencing belongs in
  [`MILESTONES.md`](../../contributing/MILESTONES.md), where each milestone's exit criterion is a
  command.
- **"Summary Statistics"** ("Core Language: 85% complete", "Overall: ~60% complete", "Implemented:
  35 features"). No counting procedure produced those numbers.

**"Version History"** is gone for the same reason: it listed "v0.6: Self-hosting achieved" against
a period in which no Palladium compiler had ever compiled itself. The real history, including the
retraction of that claim, is in [`CHANGELOG.md`](../../CHANGELOG.md).

---

This feature list defines a systems programming language that does not compromise on safety,
performance, or developer experience. What exists of it today is recorded, with evidence,
elsewhere — and the gap between the two is the project.
