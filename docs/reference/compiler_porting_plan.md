# Palladium Compiler Self-Hosting Plan

## Current Architecture Analysis

### Rust Compiler Structure

The current Rust implementation follows a traditional compiler pipeline:

```
Source Code → Lexer → Parser → Type Checker → Code Generator → C Code
                ↓        ↓           ↓              ↓
              Tokens    AST    Type-checked    Generated
                              AST             C Code
```

### Key Components

1. **Lexer** (`src/lexer/`)
   - Uses `logos` crate for tokenization
   - Token types defined in `token.rs`
   - Scanner implementation in `scanner.rs`
   - ~350 lines of code

2. **Parser** (`src/parser/`)
   - Recursive descent parser
   - Builds AST from tokens
   - Handles all language constructs
   - ~1,500 lines of code

3. **Type Checker** (`src/typeck/`)
   - Type inference and checking
   - Generic instantiation
   - Error reporting with suggestions
   - ~2,000 lines of code

4. **Code Generator** (`src/codegen/`)
   - Generates C code from AST
   - Handles runtime functions
   - Module imports
   - ~1,800 lines of code

5. **Driver** (`src/driver/`)
   - Orchestrates compilation phases
   - Error handling
   - File I/O
   - ~200 lines of code

### Dependencies Between Components

```
Driver
  ├── Lexer (independent)
  ├── Parser (depends on Token types)
  ├── Resolver (depends on AST)
  ├── Type Checker (depends on AST)
  └── Code Generator (depends on AST, Type info)
```

## Porting Strategy

### Phase 1: Minimal Compiler (Target: 1,000 lines)

Start with a minimal compiler that can compile basic Palladium programs:

#### Features to Include:
- Functions with basic types (i32, bool, String)
- Let bindings (no mutability initially)
- If/else statements
- Print statements
- Basic expressions (literals, identifiers, calls)
- String operations (concat, len, char_at)

#### Features to Defer:
- Structs and enums
- Arrays
- For/while loops
- Pattern matching
- Generics
- Module system
- Type inference

### Phase 2: Core Features (Target: 2,000 lines)

Add essential language features:
- Mutable variables
- While loops
- Arrays with fixed size
- Basic structs
- Return statements

### Phase 3: Advanced Features (Target: 3,000 lines)

Add remaining features:
- Enums with pattern matching
- For loops
- Generic functions
- Module imports
- Full type inference

## Implementation Plan

### 1. Create Basic Data Structures (100 lines)

```palladium
// token.pd
enum TokenType {
    // Literals
    IntLit(i32),
    StringLit(String),
    Ident(String),
    
    // Keywords
    Fn, Let, If, Else, Return, Print,
    
    // Operators
    Plus, Minus, Star, Slash,
    Eq, EqEq, Lt, Gt,
    
    // Delimiters
    LeftParen, RightParen,
    LeftBrace, RightBrace,
    Semicolon, Comma, Colon, Arrow,
    
    // Special
    Eof
}

struct Token {
    ty: TokenType,
    pos: i32
}
```

### 2. Implement Lexer (300 lines)

```palladium
// lexer.pd
struct Lexer {
    input: String,
    pos: i32,
    current_char: i32
}

fn lex_number(lexer: mut Lexer) -> Token {
    let mut value = 0;
    while is_digit(lexer.current_char) {
        value = value * 10 + (lexer.current_char - 48);
        lexer_advance(lexer);
    }
    return Token { ty: IntLit(value), pos: lexer.pos };
}

fn lex_string(lexer: mut Lexer) -> Token {
    // Skip opening quote
    lexer_advance(lexer);
    let mut value = "";
    
    while lexer.current_char != 34 { // "
        value = value + string_from_char(lexer.current_char);
        lexer_advance(lexer);
    }
    
    // Skip closing quote
    lexer_advance(lexer);
    return Token { ty: StringLit(value), pos: lexer.pos };
}
```

### 3. Implement Parser (400 lines)

```palladium
// parser.pd
struct Parser {
    tokens: Array<Token, 1000>,
    current: i32
}

enum Expr {
    IntLit(i32),
    StringLit(String),
    Ident(String),
    Call(String, Array<Expr, 10>),
    Binary(BinOp, Box<Expr>, Box<Expr>)
}

enum Stmt {
    Let(String, Expr),
    Expr(Expr),
    Return(Expr),
    If(Expr, Array<Stmt, 10>, Array<Stmt, 10>)
}

struct Function {
    name: String,
    params: Array<String, 10>,
    body: Array<Stmt, 100>
}
```

### 4. Implement Code Generator (200 lines)

```palladium
// codegen.pd
fn generate_expr(expr: Expr) -> String {
    match expr {
        IntLit(n) => int_to_string(n),
        StringLit(s) => "\"" + escape_string(s) + "\"",
        Ident(name) => name,
        Call(func, args) => generate_call(func, args),
        Binary(op, left, right) => 
            "(" + generate_expr(*left) + " " + 
            op_to_string(op) + " " + 
            generate_expr(*right) + ")"
    }
}
```

### 5. Create Driver (100 lines)

```palladium
// compiler.pd
fn compile(source: String) -> String {
    // Lex
    let tokens = lex(source);
    
    // Parse
    let ast = parse(tokens);
    
    // Generate code
    return generate(ast);
}

fn main() {
    let source = file_read_all("input.pd");
    let output = compile(source);
    file_write("output.c", output);
    print("Compilation complete!");
}
```

## Bootstrapping Process

1. **Stage 0**: Use existing tiny compiler (`tiny_enhanced.pd`) to compile Stage 1
2. **Stage 1**: Minimal compiler written in Palladium (compiles basic programs)
3. **Stage 2**: Stage 1 compiles itself
4. **Stage 3**: Enhanced compiler with more features
5. **Stage 4**: Full-featured compiler matching Rust version

## Testing Strategy

### Test Programs

1. **hello.pd** - Basic print statement
2. **math.pd** - Arithmetic expressions
3. **functions.pd** - Function definitions and calls
4. **control.pd** - If/else statements
5. **self_compile.pd** - Compiler compiling itself

### Validation Process

```bash
# Compile test with Rust compiler
./palladium compile test.pd -o test_rust

# Compile test with Palladium compiler
./pd_compiler test.pd -o test_pd

# Compare outputs
diff test_rust.c test_pd.c

# Run both and compare results
./test_rust > rust_output.txt
./test_pd > pd_output.txt
diff rust_output.txt pd_output.txt
```

## Timeline Estimate

- **Week 1**: Basic lexer and parser (Stage 1)
- **Week 2**: Code generator and self-compilation (Stage 2)
- **Week 3**: Add core features (Stage 3)
- **Week 4**: Full feature parity (Stage 4)

## Key Challenges

1. **Memory Management**: Palladium doesn't have dynamic allocation yet
   - Solution: Use fixed-size arrays initially
   
2. **String Handling**: Limited string operations
   - Solution: Implement essential string functions first
   
3. **Error Handling**: No exceptions or Result types
   - Solution: Use error codes and global error state
   
4. **Data Structures**: No Vec, HashMap, etc.
   - Solution: Fixed arrays and simple lookup tables

## Success Criteria

1. Palladium compiler can compile all examples in `examples/` directory
2. Generated C code is identical or functionally equivalent to Rust compiler output
3. Compiler can compile itself (true self-hosting)
4. Performance is within 2x of Rust compiler