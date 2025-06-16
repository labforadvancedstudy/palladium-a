# 🎓 Bootstrap Your Own Language: A Palladium Tutorial

> Learn how to make your programming language self-hosting, using Palladium as a case study

## Table of Contents

1. [Introduction](#introduction)
2. [Prerequisites](#prerequisites)
3. [Step 1: Design Your Language](#step-1-design-your-language)
4. [Step 2: Build Initial Compiler](#step-2-build-initial-compiler)
5. [Step 3: Implement Core Features](#step-3-implement-core-features)
6. [Step 4: Write Compiler in Your Language](#step-4-write-compiler-in-your-language)
7. [Step 5: Achieve Self-Hosting](#step-5-achieve-self-hosting)
8. [Common Pitfalls](#common-pitfalls)
9. [Testing Strategies](#testing-strategies)
10. [Celebration](#celebration)

## Introduction

Self-hosting is the holy grail of programming language development. This tutorial shows you how Palladium achieved it, so you can do the same with your language.

### What You'll Learn
- How to design a language with bootstrapping in mind
- Minimum features needed for self-hosting
- How to write a compiler in its own language
- Verification techniques for correctness

### Why Bootstrap?
```palladium
// Before bootstrap: Your language depends on another
// After bootstrap: Your language stands alone
fn compile_with_self() -> Freedom {
    return Freedom::Total;
}
```

## Prerequisites

Before starting your bootstrap journey:

1. **A Working Compiler** (in any language)
2. **Core Language Features**:
   - Functions
   - Basic types (int, string, bool)
   - Control flow (if, while/for)
   - Arrays or lists
   - Basic I/O

3. **Determination** - It's challenging but rewarding!

## Step 1: Design Your Language

### Minimal Feature Set for Bootstrap

```palladium
// You NEED these features:
struct EssentialFeatures {
    // Data types
    integers: bool,      // For array indices, AST node types
    strings: bool,       // For source code, error messages
    arrays: bool,        // For token lists, AST nodes
    structs: bool,       // For complex data structures
    
    // Control flow
    conditionals: bool,  // if/else for parsing decisions
    loops: bool,         // for/while for token iteration
    functions: bool,     // For code organization
    
    // I/O
    read_file: bool,     // Read source files
    write_file: bool,    // Write compiled output
}
```

### Example: Palladium's Minimal Design

```palladium
// Just enough to compile itself
enum MinimalPalladium {
    // Types
    I64,
    String,
    Bool,
    Array(Type, Size),
    Struct(Fields),
    
    // Expressions
    Literal(Value),
    Variable(Name),
    FunctionCall(Name, Args),
    BinaryOp(Op, Left, Right),
    
    // Statements
    Let(Name, Value),
    If(Condition, Then, Else),
    While(Condition, Body),
    Return(Value),
}
```

## Step 2: Build Initial Compiler

Start with a compiler in a language you know well:

### Example: Initial Rust Compiler

```rust
// palladium-compiler/src/main.rs
pub struct Compiler {
    source: String,
    tokens: Vec<Token>,
    ast: Option<Ast>,
}

impl Compiler {
    pub fn compile(&mut self) -> Result<String, Error> {
        self.tokenize()?;
        self.parse()?;
        self.typecheck()?;
        self.generate_code()
    }
    
    fn tokenize(&mut self) -> Result<(), Error> {
        // Convert source to tokens
    }
    
    fn parse(&mut self) -> Result<(), Error> {
        // Build AST from tokens
    }
    
    fn generate_code(&self) -> Result<String, Error> {
        // Generate target code (C, LLVM, etc.)
    }
}
```

### Keep It Simple!

```palladium
// DON'T try to implement everything at once
fn features_for_v1() -> List {
    return [
        "Basic arithmetic",
        "Function calls",
        "Simple types",
        "No generics yet!",
        "No macros yet!",
        "No async yet!",
    ];
}
```

## Step 3: Implement Core Features

### Feature Priority Order

1. **Lexer Essentials**
   ```palladium
   fn tokenize(source: String) -> [Token; 10000] {
       let mut tokens = empty_array();
       let mut i = 0;
       
       while i < string_len(source) {
           let ch = string_char_at(source, i);
           
           if is_digit(ch) {
               tokens[count] = read_number(source, i);
           } else if is_letter(ch) {
               tokens[count] = read_identifier(source, i);
           } else if ch == '"' {
               tokens[count] = read_string(source, i);
           }
           // ... more token types
       }
       return tokens;
   }
   ```

2. **Parser Fundamentals**
   ```palladium
   fn parse_expression(mut parser: Parser) -> Expr {
       let left = parse_primary(parser);
       
       while is_binary_op(peek(parser)) {
           let op = advance(parser);
           let right = parse_primary(parser);
           left = BinaryExpr { left, op, right };
       }
       
       return left;
   }
   ```

3. **Type System Basics**
   ```palladium
   fn check_types(expr: Expr) -> Type {
       match expr {
           Expr::Integer(_) => Type::I64,
           Expr::String(_) => Type::String,
           Expr::Add(left, right) => {
               let left_ty = check_types(left);
               let right_ty = check_types(right);
               if types_match(left_ty, right_ty) {
                   return left_ty;
               } else {
                   error("Type mismatch");
               }
           }
       }
   }
   ```

## Step 4: Write Compiler in Your Language

Now the fun part - rewrite your compiler in Palladium!

### Start with the Lexer

```palladium
// lexer.pd - First component to port
struct Lexer {
    input: String,
    position: i64,
    tokens: [Token; 10000],
    token_count: i64,
}

fn create_lexer(input: String) -> Lexer {
    return Lexer {
        input: input,
        position: 0,
        tokens: empty_token_array(),
        token_count: 0,
    };
}

fn next_token(mut lexer: Lexer) -> Token {
    skip_whitespace(lexer);
    
    let ch = current_char(lexer);
    
    if ch == '\0' {
        return Token { kind: TK_EOF, value: "" };
    }
    
    if is_digit(ch) {
        return read_number(lexer);
    }
    
    if is_letter(ch) {
        return read_identifier_or_keyword(lexer);
    }
    
    // ... handle other token types
}
```

### Then the Parser

```palladium
// parser.pd - Second component
struct Parser {
    tokens: [Token; 10000],
    current: i64,
}

fn parse_program(mut parser: Parser) -> Program {
    let mut functions = empty_function_array();
    let mut func_count = 0;
    
    while !is_at_end(parser) {
        functions[func_count] = parse_function(parser);
        func_count = func_count + 1;
    }
    
    return Program { functions, count: func_count };
}

fn parse_function(mut parser: Parser) -> Function {
    consume(parser, TK_FN, "Expected 'fn'");
    let name = consume_identifier(parser);
    
    consume(parser, TK_LPAREN, "Expected '('");
    let params = parse_parameters(parser);
    consume(parser, TK_RPAREN, "Expected ')'");
    
    let return_type = parse_optional_type(parser);
    let body = parse_block(parser);
    
    return Function { name, params, return_type, body };
}
```

### Type Checker

```palladium
// typechecker.pd - Ensures correctness
struct TypeChecker {
    symbols: SymbolTable,
    errors: [Error; 100],
    error_count: i64,
}

fn check_program(program: Program) -> bool {
    let mut checker = create_type_checker();
    
    // First pass: collect function signatures
    for i in 0..program.function_count {
        collect_function_signature(checker, program.functions[i]);
    }
    
    // Second pass: check function bodies
    for i in 0..program.function_count {
        check_function(checker, program.functions[i]);
    }
    
    return checker.error_count == 0;
}
```

### Code Generator

```palladium
// codegen.pd - Final component
struct CodeGen {
    output: StringBuilder,
    indent: i64,
}

fn generate_program(program: Program) -> String {
    let mut gen = create_codegen();
    
    // Generate headers
    emit_line(gen, "#include <stdio.h>");
    emit_line(gen, "#include <stdlib.h>");
    emit_line(gen, "");
    
    // Generate each function
    for i in 0..program.function_count {
        generate_function(gen, program.functions[i]);
    }
    
    return string_builder_to_string(gen.output);
}
```

## Step 5: Achieve Self-Hosting

### The Bootstrap Process

```bash
# Step 1: Use Rust compiler to compile Palladium compiler
$ rustc palladium.rs -o palladium_stage0
$ ./palladium_stage0 compiler.pd -o palladium_stage1

# Step 2: Use stage1 to compile itself
$ ./palladium_stage1 compiler.pd -o palladium_stage2

# Step 3: Verify they're identical
$ ./palladium_stage2 compiler.pd -o palladium_stage3
$ diff palladium_stage2 palladium_stage3
# No output = SUCCESS!
```

### Verification Script

```palladium
// verify_bootstrap.pd
fn main() {
    print("=== Bootstrap Verification ===\n");
    
    // Compile test programs with both compilers
    let test_files = ["hello.pd", "fibonacci.pd", "compiler.pd"];
    
    for i in 0..3 {
        let file = test_files[i];
        
        // Compile with stage1
        run_command("./palladium_stage1", file, "out1.c");
        
        // Compile with stage2  
        run_command("./palladium_stage2", file, "out2.c");
        
        // Compare outputs
        if files_identical("out1.c", "out2.c") {
            print("✅ ");
            print(file);
            print(" - outputs match!\n");
        } else {
            print("❌ ");
            print(file);
            print(" - OUTPUTS DIFFER!\n");
        }
    }
}
```

## Common Pitfalls

### 1. Circular Dependencies

```palladium
// WRONG: Can't use feature X to implement feature X
fn implement_arrays() {
    let tokens: Array<Token> = ...;  // Need arrays to implement arrays!
}

// RIGHT: Use simpler version first
fn implement_arrays_bootstrap() {
    let tokens: [Token; 10000] = ...;  // Fixed array works
}
```

### 2. Missing Features

```palladium
// Discover missing features early
fn check_bootstrap_requirements() {
    assert(has_file_io(), "Need file I/O for compiler");
    assert(has_string_ops(), "Need string manipulation");
    assert(has_arrays(), "Need collections for AST");
    assert(has_structs(), "Need complex data types");
}
```

### 3. Memory Issues

```palladium
// Be careful with memory in bootstrap
struct SafeParser {
    // Bounded buffers
    tokens: [Token; 10000],
    max_tokens: i64,
    
    // Overflow detection
    overflow: bool,
}

fn add_token_safe(mut parser: SafeParser, token: Token) {
    if parser.token_count < parser.max_tokens {
        parser.tokens[parser.token_count] = token;
        parser.token_count = parser.token_count + 1;
    } else {
        parser.overflow = true;
        report_error("Token buffer overflow");
    }
}
```

## Testing Strategies

### 1. Unit Tests for Each Component

```palladium
fn test_lexer() {
    let input = "fn main() { print(42); }";
    let tokens = tokenize(input);
    
    assert_eq(tokens[0].kind, TK_FN);
    assert_eq(tokens[1].kind, TK_IDENT);
    assert_eq(tokens[1].value, "main");
    // ... more assertions
}
```

### 2. Integration Tests

```palladium
fn test_full_compilation() {
    let programs = [
        "fn main() { return 0; }",
        "fn add(a: i64, b: i64) -> i64 { return a + b; }",
        "struct Point { x: i64, y: i64 }",
    ];
    
    for i in 0..3 {
        let result = compile(programs[i]);
        assert(result.success, "Compilation failed");
        assert(is_valid_c(result.output), "Invalid C output");
    }
}
```

### 3. Bootstrap Verification

```palladium
fn verify_self_hosting() {
    // The ultimate test
    let compiler_source = read_file("compiler.pd");
    
    // Compile with current compiler
    let output1 = compile(compiler_source);
    
    // Compile output and run it
    write_file("temp.c", output1);
    system("gcc temp.c -o new_compiler");
    
    // Use new compiler to compile something
    system("./new_compiler test.pd -o test.c");
    
    print("🎉 Self-hosting verified!");
}
```

## Celebration

Once you achieve self-hosting:

```palladium
fn celebrate_bootstrap() {
    print("🎊 CONGRATULATIONS! 🎊\n");
    print("Your language is now self-hosting!\n\n");
    
    print("You've achieved:\n");
    print("✅ Language independence\n");
    print("✅ Proof of completeness\n");
    print("✅ Compiler development in your own language\n");
    print("✅ The respect of language designers everywhere\n");
    
    print("\nWhat's next:\n");
    print("🚀 Optimize the compiler\n");
    print("🚀 Add advanced features\n");
    print("🚀 Build a community\n");
    print("🚀 Change the world!\n");
}
```

### Share Your Success!

```palladium
fn announce_bootstrap() {
    let tweet = string_concat(
        "🎉 Big announcement: ",
        MY_LANGUAGE_NAME,
        " is now self-hosting! The compiler is written in ",
        MY_LANGUAGE_NAME,
        " itself. From dream to reality! #ProgrammingLanguages #Compilers"
    );
    
    post_to_social_media(tweet);
    write_blog_post("How We Achieved Self-Hosting");
    submit_to_hacker_news("Show HN: " + MY_LANGUAGE_NAME + " - A Self-Hosting Language");
}
```

## Resources

### Further Reading
- "Bootstrapping a Compiler" by Abdulaziz Ghuloum
- "Engineering a Compiler" by Cooper & Torczon
- Palladium's bootstrap source code

### Community
- Join language designers forums
- Share your journey
- Help others achieve self-hosting

---

*"The compiler that compiles itself has transcended mere software to become a living language."*

Good luck on your bootstrap journey! 🚀