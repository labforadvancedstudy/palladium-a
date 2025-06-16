# Alan von Palladium v0.2 - Language Foundations TODO

```
 ╔══════════════════════════════════════════════════════════════╗
 ║              MISSION: REAL PROGRAMMING LANGUAGE              ║
 ║         Variables, Arithmetic, Control Flow & LLVM           ║
 ╚══════════════════════════════════════════════════════════════╝
```

## 🎯 Goal: Complete Basic Language Features (3 weeks)

**Success Criteria**: Compile and run this program:
```palladium
fn main() -> i32 {
    let x = 10;
    let y = 20;
    let sum = x + y;
    
    if sum > 25 {
        print("Sum is large: ");
        print_int(sum);
    } else {
        print("Sum is small");
    }
    
    return sum;
}
```

## 📋 TODO List (Priority Order)

### Week 1: Variables and Expressions

#### Day 1-2: Variable Support
- [ ] Add `let` statement to AST
  ```rust
  Stmt::Let {
      name: String,
      ty: Option<Type>,
      init: Option<Expr>,
  }
  ```
- [ ] Update parser to handle let statements
- [ ] Add symbol table to type checker
- [ ] Implement variable resolution in expressions
- [ ] Generate code for variable declarations
- [ ] Add tests for variable binding

#### Day 3-4: Integer Arithmetic
- [ ] Add integer literal tokens
- [ ] Add arithmetic operators to lexer (+, -, *, /, %)
- [ ] Update expression parser for binary operators
- [ ] Implement operator precedence
- [ ] Add integer type to type system
- [ ] Type check arithmetic expressions
- [ ] Generate code for arithmetic operations
- [ ] Add `print_int` built-in function

#### Day 5-6: Boolean Operations
- [ ] Add boolean type and literals (true/false)
- [ ] Add comparison operators (<, >, <=, >=, ==, !=)
- [ ] Add logical operators (&&, ||, !)
- [ ] Update parser and type checker
- [ ] Generate code for boolean operations
- [ ] Add tests for boolean expressions

### Week 2: Control Flow

#### Day 7-8: If/Else Statements
- [ ] Add if/else to AST
  ```rust
  Stmt::If {
      condition: Expr,
      then_branch: Box<Stmt>,
      else_branch: Option<Box<Stmt>>,
  }
  ```
- [ ] Update parser for if/else syntax
- [ ] Type check conditions (must be bool)
- [ ] Generate code with proper branching
- [ ] Handle nested if statements
- [ ] Add comprehensive tests

#### Day 9-10: Block Statements and Scope
- [ ] Add block statement to AST
- [ ] Implement lexical scoping
- [ ] Update symbol table for nested scopes
- [ ] Handle variable shadowing
- [ ] Generate code for blocks
- [ ] Test scope resolution

#### Day 11-12: While Loops
- [ ] Add while loop to AST
- [ ] Parse while loop syntax
- [ ] Type check loop conditions
- [ ] Generate loop code with proper labels
- [ ] Add break/continue support (stretch goal)
- [ ] Test various loop patterns

### Week 3: LLVM Integration

#### Day 13-14: LLVM Setup
- [ ] Re-enable inkwell dependency
- [ ] Create LLVM context and module
- [ ] Set up LLVM builder
- [ ] Define target triple and data layout
- [ ] Create main function in LLVM IR
- [ ] Set up basic runtime functions

#### Day 15-16: LLVM Code Generation
- [ ] Translate AST to LLVM IR
- [ ] Generate LLVM functions
- [ ] Generate LLVM basic blocks
- [ ] Implement phi nodes for control flow
- [ ] Generate LLVM arithmetic operations
- [ ] Link with C runtime for print functions

#### Day 17-18: LLVM Optimization & Output
- [ ] Add optimization passes
- [ ] Generate object files
- [ ] Link to create executables
- [ ] Add JIT execution option
- [ ] Benchmark vs C backend
- [ ] Update CLI for LLVM options

#### Day 19-21: Testing & Polish
- [ ] Port all existing tests to new features
- [ ] Add integration tests for complex programs
- [ ] Update documentation
- [ ] Performance testing
- [ ] Error message improvements
- [ ] Update examples directory

## 🚀 New Language Features for v0.2

### Supported:
- [x] Variables with let bindings
- [x] Integer literals and arithmetic
- [x] Boolean literals and operations
- [x] If/else conditionals
- [x] While loops
- [x] Block statements and scoping
- [x] Type annotations (optional)
- [x] Multiple built-in functions

### Still Deferred (v0.3+):
- [ ] For loops
- [ ] Arrays/Vectors
- [ ] User-defined types (structs)
- [ ] Pattern matching
- [ ] Modules/imports
- [ ] Generics
- [ ] Traits/interfaces
- [ ] Memory management

## 📁 New Files to Create

```
src/
├── symbol_table/
│   └── mod.rs      # Symbol table implementation
├── llvm/
│   ├── mod.rs      # LLVM backend entry point
│   ├── context.rs  # LLVM context management
│   └── codegen.rs  # LLVM IR generation
└── optimizer/
    └── mod.rs      # Basic optimization passes
```

## 🔧 Files to Modify

- `src/lexer/token.rs` - Add integer literals, new operators
- `src/parser/mod.rs` - Add let, if/else, while, blocks
- `src/ast/mod.rs` - Extend with new statement/expression types
- `src/typeck/mod.rs` - Add symbol table, handle new types
- `src/codegen/mod.rs` - Extend for new constructs (or replace with LLVM)
- `Cargo.toml` - Re-enable inkwell dependency

## 📊 Success Metrics

- [ ] All v0.1 tests still pass
- [ ] Variables can be declared and used
- [ ] Arithmetic expressions work correctly
- [ ] If/else branches execute correctly
- [ ] While loops iterate properly
- [ ] LLVM backend generates working executables
- [ ] Performance is within 2x of equivalent C code
- [ ] Error messages remain helpful and clear

## 💡 Implementation Strategy

1. **Incremental Development**: Add features one at a time
2. **Test-Driven**: Write tests before implementing features
3. **Backwards Compatible**: Keep v0.1 programs working
4. **Performance Aware**: Measure impact of new features
5. **Clean Architecture**: Refactor as needed

## 🎨 Example Programs to Support

### Fibonacci
```palladium
fn main() -> i32 {
    let n = 10;
    let a = 0;
    let b = 1;
    let i = 0;
    
    while i < n {
        let temp = a + b;
        a = b;
        b = temp;
        i = i + 1;
    }
    
    print("Fibonacci result: ");
    print_int(b);
    return 0;
}
```

### Number Guessing
```palladium
fn main() -> i32 {
    let secret = 42;
    let guess = 50;
    
    if guess > secret {
        print("Too high!");
    } else if guess < secret {
        print("Too low!");
    } else {
        print("Correct!");
    }
    
    return 0;
}
```

## 🚨 Potential Challenges

1. **LLVM Learning Curve**: Inkwell API complexity
2. **Type Inference**: Balancing inference with explicit types
3. **Error Recovery**: Parser should handle more cases gracefully
4. **Performance**: Ensure compilation remains fast
5. **Debugging**: Need better tools for debugging generated code

## 📅 Daily Check-ins

Continue asking:
1. What feature was implemented today?
2. What tests were added?
3. What challenges were encountered?
4. Is the timeline still realistic?

---

*"Move fast and break things, then fix them even faster."* - Silicon Valley Wisdom
*"But with formal verification, we won't break things in the first place."* - AVP Team

**Let's give Palladium its wings! 🚀**