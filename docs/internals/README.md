# Palladium Compiler Internals

This directory contains detailed documentation about the internal architecture and implementation of the Palladium compiler.

## 📊 Compiler Pipeline Overview

```
Source Code (.pd)
      ↓
┌─────────────┐
│   Lexer     │ → Tokens
├─────────────┤
│   Parser    │ → AST
├─────────────┤
│ Type Check  │ → Typed AST
├─────────────┤
│  Resolver   │ → Resolved AST
├─────────────┤
│  Codegen    │ → C/LLVM IR
└─────────────┘
      ↓
Executable
```

## 📚 Documentation Structure

### Core Components
- **[Architecture Overview](./ARCHITECTURE.md)** - High-level system design

### Language Features
- **[Generics Design](../design/generics.md)** - Generic type system
- **[Module System Design](../design/module-system.md)** - Module resolution
- **[Error Messages](./ERROR_MESSAGES_IMPROVEMENT.md)** - Diagnostic system
- **[Performance Optimization](./PERFORMANCE_OPTIMIZATION.md)** - Optimization strategies

### Bootstrap Documentation
Complete documentation of Palladium's self-hosting journey:
- **[Bootstrap Overview](./bootstrap/README.md)** - Introduction to bootstrapping
- **[Bootstrap Strategy](./bootstrap/BOOTSTRAP_STRATEGY.md)** - Approach and methodology
- **[Bootstrap Internals](./bootstrap/BOOTSTRAP_INTERNALS.md)** - Technical details
- **[Bootstrap Tutorial](./bootstrap/BOOTSTRAP_TUTORIAL.md)** - Step-by-step guide
- **[Bootstrap FAQ](./bootstrap/BOOTSTRAP_FAQ.md)** - Common questions
- **[Self-Hosting Guide](./bootstrap/SELF_HOSTING_GUIDE.md)** - How to use bootstrap compilers

## 🔧 Compiler Architecture

### Phase 1: Lexical Analysis (Lexer)
```rust
// Token representation
pub enum Token {
    // Literals
    Integer(i64),
    String(String),
    Bool(bool),
    
    // Keywords
    Fn, Let, If, Else, While, For,
    Struct, Enum, Match, Return,
    
    // Operators
    Plus, Minus, Star, Slash,
    Eq, Ne, Lt, Gt, Le, Ge,
    
    // Delimiters
    LeftParen, RightParen,
    LeftBrace, RightBrace,
    Semicolon, Comma,
}
```

**Implementation**: Uses `logos` crate for fast lexing
- Location: `src/lexer/scanner.rs`
- Lines: ~200
- Performance: ~50MB/s on typical code

### Phase 2: Parsing
```rust
// AST Node types
pub enum Expr {
    Literal(Literal),
    Binary(Box<Expr>, BinOp, Box<Expr>),
    Unary(UnOp, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    If(Box<Expr>, Box<Expr>, Option<Box<Expr>>),
    Block(Vec<Stmt>),
    // ... more variants
}

pub enum Stmt {
    Let(Pattern, Option<Type>, Option<Expr>),
    Expr(Expr),
    Return(Option<Expr>),
    // ... more variants
}
```

**Implementation**: Recursive descent parser
- Location: `src/parser/mod.rs`
- Approach: Pratt parsing for expressions
- Error recovery: Synchronization at statement boundaries

### Phase 3: Type Checking
```rust
// Type representation
pub enum Type {
    Unit,
    Bool,
    Int(IntType),
    String,
    Array(Box<Type>, usize),
    Slice(Box<Type>),
    Ref(Box<Type>, Mutability),
    Fn(Vec<Type>, Box<Type>),
    Struct(StructId),
    Enum(EnumId),
    Generic(GenericId),
}
```

**Features**:
- Hindley-Milner type inference
- Bidirectional type checking
- Implicit lifetime inference (partial)
- Generic monomorphization

### Phase 4: Module Resolution
```rust
// Module system
pub struct Module {
    name: String,
    items: HashMap<String, Item>,
    imports: Vec<Import>,
    exports: Vec<Export>,
}
```

**Current Status**: Basic implementation
- Single-file modules work
- Import resolution implemented
- Cross-module type checking in progress

### Phase 5: Code Generation

**Current Backend**: C
- Clean, readable C output
- Runtime functions for memory management
- Plans for LLVM backend

**Example Output**:
```c
// Palladium: fn add(x: i64, y: i64) -> i64 { x + y }
long long add(long long x, long long y) {
    return x + y;
}
```

## 🎯 Key Design Decisions

### 1. **Bootstrap-First Development**
- Every feature must work in bootstrap compiler
- Simplicity over complexity initially
- Gradual feature addition

### 2. **C Backend for Bootstrap**
- Portable across platforms
- Easy to debug and understand
- No complex runtime required

### 3. **Implicit Lifetimes**
- Infer lifetimes in 90% of cases
- Reduce annotation burden
- Maintain safety guarantees

### 4. **Effect System for Async**
- No function coloring
- Automatic parallelization
- Composable effects

### 5. **Totality Checking**
- Opt-in with `#[total]`
- Multiple proof strategies
- Performance benefits

## 🔬 Implementation Details

### Memory Management
- **Current**: Reference counting (in C backend)
- **Planned**: Ownership with borrowing
- **Goal**: Zero-cost abstractions

### Error Handling
- **Current**: Basic error messages
- **Planned**: Rust-like diagnostics with suggestions
- **Future**: IDE integration with fixes

### Optimization
- **Current**: Minimal (relies on C compiler)
- **Planned**: SSA form, inlining, constant folding
- **Future**: LLVM integration for advanced opts

## 📈 Performance Targets

| Metric | Current | Target | Notes |
|--------|---------|--------|-------|
| Compile Speed | 10K LOC/s | 50K LOC/s | With full type checking |
| Memory Usage | 100MB/file | 50MB/file | For typical source files |
| Binary Size | +20% vs C | +5% vs C | After optimization |
| Runtime Speed | 80% of C | 95% of C | For systems code |

## 🚧 Current Limitations

1. **Type System**
   - No higher-kinded types yet
   - Limited trait system
   - Basic generics only

2. **Error Messages**
   - Minimal context
   - No suggestions
   - Line numbers only

3. **Optimization**
   - No inlining
   - No dead code elimination
   - Basic constant folding only

4. **Platform Support**
   - Only generates C
   - No direct assembly
   - Limited OS integration

## 🔮 Future Architecture

### Planned Compiler Phases
1. **HIR** (High-level IR) - Desugared AST
2. **MIR** (Mid-level IR) - Control flow graph
3. **LIR** (Low-level IR) - Close to machine code

### Planned Backends
- **LLVM** - Primary optimization backend
- **Cranelift** - Fast compilation backend
- **WebAssembly** - Web target
- **Native** - Direct machine code

### Planned Features
- **Incremental Compilation** - Function-level caching
- **Parallel Compilation** - Multi-threaded pipeline
- **Query-based Architecture** - Like rustc
- **Language Server** - Full IDE support

## 🤝 Contributing to Internals

### Getting Started
1. Read the architecture overview
2. Pick a small component to study
3. Try adding a simple feature
4. Write tests for your changes

### Code Organization
```
src/
├── ast/        # AST definitions
├── lexer/      # Tokenization
├── parser/     # Parsing logic
├── typeck/     # Type checking
├── resolver/   # Module resolution
├── codegen/    # Code generation
├── errors/     # Error reporting
└── driver/     # Compiler driver
```

### Testing Strategy
- Unit tests for each phase
- Integration tests for full pipeline
- Bootstrap tests for self-hosting
- Fuzzing for parser robustness

---

*"Understanding the compiler is understanding the language."*