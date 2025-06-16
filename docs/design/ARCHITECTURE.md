# Alan von Palladium Compiler Architecture

```
    ╔═══════════════════════════════════════════════════════════╗
    ║          Alan von Palladium Compiler Pipeline             ║
    ╚═══════════════════════════════════════════════════════════╝

    Source (.pd) → Lexer → Parser → AST → Type Checker → MIR
         ↓                                                  ↓
    Verification ← Proof Engine ← HIR ← Optimization ← Lowering
         ↓                                                  ↓
    Executable ← Linker ← Object ← Assembly ← LLVM IR ← Codegen
```

## Overview

The Alan von Palladium compiler (pdc) is designed with three core principles:
1. **Correctness First**: Every phase can be formally verified
2. **Performance Aware**: Zero-cost abstractions with hardware optimization
3. **Gradual Verification**: Start unsafe, prove incrementally

## Compilation Phases

### 1. Frontend

#### Lexer (src/lexer/)
- Token-based scanning using Logos
- Context-aware keyword recognition
- Preserves source location for error reporting

#### Parser (src/parser/)
- Recursive descent parser using Chumsky
- Produces typed AST with source spans
- Error recovery for better diagnostics

#### AST (src/ast/)
```rust
pub enum Expr {
    Literal(Literal),
    Ident(String),
    Binary(Box<Expr>, BinOp, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    Lambda(Vec<Pattern>, Box<Expr>),
    Let(Pattern, Box<Expr>, Box<Expr>),
    If(Box<Expr>, Box<Expr>, Option<Box<Expr>>),
    Match(Box<Expr>, Vec<(Pattern, Expr)>),
}
```

### 2. Middle End

#### Type Checker (src/typeck/)
- Hindley-Milner type inference with extensions
- Region inference for automatic lifetimes
- Effect tracking (async, error, total)
- Trait resolution

#### HIR - High-level IR (src/hir/)
- Desugared representation
- All types explicit
- Effects resolved
- Ready for verification

#### MIR - Mid-level IR (src/mir/)
- Control flow graph representation
- SSA form
- Borrow checking happens here
- Optimization-friendly

### 3. Verification Engine (src/verify/)

```rust
pub trait ProofBackend {
    fn verify_termination(&self, func: &MirFunction) -> Result<Proof>;
    fn verify_memory_safety(&self, func: &MirFunction) -> Result<Proof>;
    fn verify_side_channel(&self, func: &MirFunction) -> Result<LeakageBound>;
}

pub struct LeanBackend;
pub struct CoqBackend;
pub struct IsabelleBackend;
```

### 4. Backend

#### LLVM Codegen (src/codegen/)
- Uses Inkwell for LLVM bindings
- Generates optimized LLVM IR
- Supports multiple targets
- Integrates with LLVM's optimization passes

#### Runtime (runtime/)
- Minimal runtime for memory management
- Coroutine support
- FFI boundaries
- Panic handling

## Data Flow

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Source    │────▶│    Lexer    │────▶│   Parser    │
│    .pd      │     │   Tokens    │     │     AST     │
└─────────────┘     └─────────────┘     └─────────────┘
                                               │
                                               ▼
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  Optimized  │◀────│     MIR     │◀────│    Type     │
│    MIR      │     │   Graph     │     │   Checker   │
└─────────────┘     └─────────────┘     └─────────────┘
       │                    │                    │
       │                    ▼                    │
       │            ┌─────────────┐              │
       │            │   Verify    │◀─────────────┘
       │            │   Engine    │
       │            └─────────────┘
       ▼
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  LLVM IR    │────▶│  Assembly   │────▶│   Binary    │
│  CodeGen    │     │    .s       │     │    .out     │
└─────────────┘     └─────────────┘     └─────────────┘
```

## Module Structure

```
palladium/
├── src/
│   ├── main.rs          # CLI entry point
│   ├── lib.rs           # Compiler library root
│   ├── driver/          # Compilation driver
│   │   ├── mod.rs
│   │   └── session.rs   # Compiler session management
│   ├── lexer/           # Tokenization
│   │   ├── mod.rs
│   │   ├── token.rs
│   │   └── scanner.rs
│   ├── parser/          # Parsing
│   │   ├── mod.rs
│   │   ├── expr.rs
│   │   ├── stmt.rs
│   │   └── decl.rs
│   ├── ast/             # Abstract Syntax Tree
│   │   ├── mod.rs
│   │   ├── node.rs
│   │   └── visitor.rs
│   ├── typeck/          # Type checking
│   │   ├── mod.rs
│   │   ├── infer.rs
│   │   ├── unify.rs
│   │   └── traits.rs
│   ├── hir/             # High-level IR
│   │   ├── mod.rs
│   │   └── lower.rs
│   ├── mir/             # Mid-level IR
│   │   ├── mod.rs
│   │   ├── build.rs
│   │   └── optimize.rs
│   ├── verify/          # Verification
│   │   ├── mod.rs
│   │   ├── lean.rs
│   │   ├── coq.rs
│   │   └── isabelle.rs
│   ├── codegen/         # Code generation
│   │   ├── mod.rs
│   │   ├── llvm.rs
│   │   └── abi.rs
│   └── errors/          # Error handling
│       ├── mod.rs
│       └── diagnostic.rs
├── runtime/             # Runtime library
│   ├── Cargo.toml
│   └── src/
├── std/                 # Standard library
│   ├── Cargo.toml
│   └── src/
├── tests/               # Integration tests
├── examples/            # Example programs
└── proofs/              # Formal proofs
    ├── lean/
    ├── coq/
    └── isabelle/
```

## Key Design Decisions

### 1. Three-Tier Compilation
- **Tier 0**: Total functions with full verification
- **Tier 1**: Safe partial functions (default)
- **Tier 2**: Unsafe code with explicit boundaries

### 2. Effect System
```palladium
effect async { await<T>(Future<T>) -> T }
effect error { throw<E>(E) -> ! }
effect total { metric<M: Nat>(M) }

fn server() with async + error {
    let conn = await(accept())?;
    handle(conn).await
}
```

### 3. Implicit Smart Pointers
The compiler automatically chooses between:
- Stack allocation
- Reference counting (Rc)
- Arena allocation
- Box allocation

Based on escape analysis and usage patterns.

### 4. Verification Strategy
- Lean for core language semantics
- Coq for concurrency proofs
- Isabelle for hardware models

## Bootstrap Plan

### Phase 0: Minimal Compiler (v0.1)
1. Basic lexer and parser
2. Simple type checker (no inference)
3. Direct LLVM IR generation
4. Compile basic programs

### Phase 1: Core Features (v0.2-0.4)
1. Type inference
2. Pattern matching
3. Modules and namespaces
4. Basic optimizations

### Phase 2: Advanced Features (v0.5-0.7)
1. Effect system
2. Verification integration
3. Advanced optimizations
4. Self-hosting preparation

### Phase 3: Self-Hosting (v0.8-1.0)
1. Compiler written in Palladium
2. Full verification of compiler
3. Production ready

## Performance Targets

- Compilation: O(n²) worst case, O(n log n) typical
- Memory: < 100MB for 100K LOC project
- Runtime: Within 5% of C for systems code
- Verification: < 1s per function for basic proofs

## Testing Strategy

1. **Unit Tests**: Each module has comprehensive tests
2. **Integration Tests**: Full compilation pipeline tests
3. **Property Tests**: QuickCheck-style testing
4. **Fuzzing**: AFL++ for parser robustness
5. **Verification Tests**: Proof obligations are tested

## Error Handling Philosophy

- Errors are first-class values
- Rich diagnostics with suggestions
- Error recovery in parser
- Incremental compilation friendly

```rust
#[derive(Error, Debug)]
pub enum CompileError {
    #[error("Type mismatch: expected {expected}, found {found}")]
    TypeMismatch { expected: Type, found: Type, span: Span },
    
    #[error("Cannot prove termination for function {name}")]
    TerminationUnproven { name: String, suggestion: String },
}
```

## Future Considerations

1. **Incremental Compilation**: Track dependencies for fast rebuilds
2. **Language Server**: Full LSP implementation
3. **Package Manager**: Cargo-compatible package system
4. **Cross Compilation**: Support for multiple targets
5. **JIT Compilation**: For REPL and dynamic scenarios

---

*"Architecture is the art of how to waste space productively." - Philip Johnson*
*"In Palladium, we waste nothing - not space, not time, not correctness." - AVP Team*