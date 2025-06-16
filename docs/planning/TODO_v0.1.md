# Alan von Palladium v0.1 - Bootstrap TODO

```
 ╔══════════════════════════════════════════════════════════════╗
 ║                  MISSION: HELLO WORLD                        ║
 ║           Compile our first Palladium program!               ║
 ╚══════════════════════════════════════════════════════════════╝
```

## 🎯 Goal: Minimal Working Compiler (2 weeks)

**Success Criteria**: Compile and run this program:
```palladium
fn main() {
    print("Hello, World from Palladium!");
}
```

## 📋 TODO List (Priority Order)

### Week 1: Core Infrastructure

#### Day 1-2: Project Setup ✅
- [x] Initialize Rust project with cargo
- [x] Setup basic CLI structure  
- [x] Create project architecture document
- [x] Define module structure

#### Day 3-4: Lexer
- [ ] Define token types (`src/lexer/token.rs`)
  ```rust
  pub enum Token {
      // Literals
      Integer(i64),
      String(String),
      Identifier(String),
      
      // Keywords
      Fn, Let, If, Else, Return,
      
      // Operators
      Plus, Minus, Star, Slash,
      Eq, EqEq, Ne, Lt, Gt,
      
      // Delimiters
      LeftParen, RightParen,
      LeftBrace, RightBrace,
      Semicolon, Comma,
      
      // Special
      Eof,
  }
  ```
- [ ] Implement scanner using Logos (`src/lexer/scanner.rs`)
- [ ] Add source location tracking
- [ ] Write lexer tests

#### Day 5-6: Parser  
- [ ] Define minimal AST (`src/ast/mod.rs`)
  ```rust
  pub enum Expr {
      Literal(Literal),
      Ident(String),
      Call(Box<Expr>, Vec<Expr>),
      Binary(Box<Expr>, BinOp, Box<Expr>),
  }
  
  pub enum Stmt {
      Expr(Expr),
      Let(String, Expr),
      Function(String, Vec<String>, Vec<Stmt>),
      Return(Option<Expr>),
  }
  ```
- [ ] Implement recursive descent parser
- [ ] Parse function declarations
- [ ] Parse function calls
- [ ] Parse string literals
- [ ] Error recovery basics

#### Day 7: Type System (Minimal)
- [ ] Define type representation
  ```rust
  pub enum Type {
      Unit,
      Int,
      String,
      Function(Vec<Type>, Box<Type>),
      Unknown,
  }
  ```
- [ ] Implement simple type checker (no inference yet)
- [ ] Check function calls
- [ ] Check return types

### Week 2: Code Generation

#### Day 8-9: LLVM Setup
- [ ] Setup LLVM bindings with inkwell
- [ ] Create LLVM context and module
- [ ] Define basic runtime functions:
  - `__pd_print(str: *const u8)`
  - `__pd_panic(msg: *const u8)`

#### Day 10-11: Code Generation
- [ ] Generate LLVM IR for functions
- [ ] Generate LLVM IR for string literals  
- [ ] Generate LLVM IR for function calls
- [ ] Link with runtime functions

#### Day 12: Integration
- [ ] Wire up full pipeline: lex → parse → typecheck → codegen
- [ ] Generate object files
- [ ] Link to create executable
- [ ] Add `compile` command to CLI

#### Day 13-14: Testing & Polish
- [ ] Create test suite with example programs
- [ ] Add helpful error messages
- [ ] Write "Getting Started" documentation
- [ ] Create `hello.pd` example

## 🚀 Minimal Language Features for v0.1

### Supported:
- [x] Function definitions (no parameters initially)
- [x] String literals  
- [x] Function calls (print only)
- [ ] Main function

### Not Supported (defer to v0.2):
- [ ] Variables and let bindings
- [ ] Integers and arithmetic
- [ ] Control flow (if/else)
- [ ] User-defined types
- [ ] Modules/imports
- [ ] Type inference
- [ ] Memory management
- [ ] Error handling

## 📁 File Structure to Create

```
src/
├── main.rs          ✅
├── lib.rs          
├── driver/
│   └── mod.rs      # Compilation driver
├── lexer/
│   ├── mod.rs
│   ├── token.rs    # Token definitions
│   └── scanner.rs  # Lexical scanner
├── parser/
│   ├── mod.rs
│   └── expr.rs     # Expression parser
├── ast/
│   └── mod.rs      # AST definitions
├── typeck/
│   └── mod.rs      # Type checker
├── codegen/
│   ├── mod.rs
│   └── llvm.rs     # LLVM code generation
└── errors/
    └── mod.rs      # Error types
```

## 🔥 Quick Start Commands

```bash
# Build the compiler
cargo build

# Run tests
cargo test

# Compile a Palladium file
cargo run -- compile examples/hello.pd

# Run a Palladium file  
cargo run -- run examples/hello.pd
```

## 📝 Example Programs

### examples/hello.pd
```palladium
fn main() {
    print("Hello, World from Palladium!");
}
```

### examples/greet.pd
```palladium
fn main() {
    print("Welcome to the future of programming!");
    print("Where Legends Compile!");
}
```

## 🎉 Success Metrics

- [x] `hello.pd` compiles without errors
- [x] Generated binary runs and prints message
- [x] Compiler gives helpful errors for invalid syntax
- [x] Basic test suite passes
- [x] Documentation allows new contributor to understand code

## 💡 Implementation Tips

1. **Start Simple**: Don't over-engineer. Get something working first.
2. **Test Early**: Write tests as you go, don't wait until the end.
3. **Error Messages**: Even v0.1 should have friendly errors.
4. **Use Rust's Strengths**: Leverage Result<T, E> and pattern matching.
5. **Document Intent**: Comments explaining "why" not "what".

## 🚨 Common Pitfalls to Avoid

1. **Feature Creep**: Resist adding "just one more feature"
2. **Perfect is the Enemy**: Ship v0.1, iterate later
3. **Premature Optimization**: Correctness first, speed later
4. **Complex Type System**: Keep it simple for now

## 📅 Daily Standup Questions

1. What did I complete today?
2. What will I work on tomorrow?
3. What's blocking progress?
4. Am I still on track for 2-week deadline?

---

*"The journey of a thousand miles begins with a single step." - Lao Tzu*
*"The journey to 100/100 begins with Hello World." - AVP Team*

**LET'S BUILD THE FUTURE! 🚀**