# Bootstrap Strategy for Palladium

## Current Status (June 16, 2025)

We've successfully:
1. ✅ Implemented generic monomorphization 
2. ✅ Created a self-hosting compiler in `bootstrap2/` (uses Vec/Box/match)
3. ✅ Verified basic Palladium compilation works (ultra_minimal.pd)
4. ✅ All file I/O runtime functions are implemented

## The Challenge

The current bootstrap compiler uses advanced features that it cannot compile:
- `Vec<T>` - Dynamic arrays
- `Box<T>` - Heap allocation
- `match` expressions - Pattern matching
- `&` and `&mut` - References

## The Solution: Three-Stage Bootstrap

### Stage 1: Ultra-Minimal Compiler (Current Focus)
A compiler written in the most basic subset of Palladium that can compile itself.

**Features:**
- Fixed-size arrays only
- No references (use global state or return updated structures)
- Simple if/else instead of match
- Basic types only (i64, bool, String)
- No generics
- No modules/imports

**Architecture:**
```palladium
// Global state approach
let mut TOKENS: [Token; 10000] = [...];
let mut TOKEN_COUNT: i64 = 0;
let mut CURRENT_TOKEN: i64 = 0;

// Or functional approach - return updated state
struct LexerState {
    tokens: [Token; 10000],
    count: i64,
    pos: i64,
}

fn lex_token(state: LexerState) -> LexerState {
    // Return updated state
}
```

### Stage 2: Enhanced Compiler
Once we can compile the ultra-minimal compiler with itself, we use it to compile a more advanced version.

**Additional Features:**
- Dynamic arrays (implement Vec)
- Pattern matching
- Module system
- Better error handling

### Stage 3: Full Compiler
The enhanced compiler compiles the full-featured compiler.

**Additional Features:**
- Generics
- Traits/interfaces
- Optimization passes
- LLVM backend

## Implementation Plan

### 1. Ultra-Minimal Lexer (bootstrap3/lexer.pd)
```palladium
const MAX_TOKENS: i64 = 10000;
const MAX_INPUT: i64 = 100000;

struct Token {
    type: i64,
    start: i64,
    len: i64,
    value: i64,
}

struct LexerState {
    input: [i64; MAX_INPUT],
    input_len: i64,
    pos: i64,
    tokens: [Token; MAX_TOKENS],
    token_count: i64,
}

fn lex(input: String) -> LexerState {
    // Return complete lexer state
}
```

### 2. Ultra-Minimal Parser (bootstrap3/parser.pd)
```palladium
struct ASTNode {
    type: i64,
    // Fixed-size fields for different node types
    name: [i64; 100],
    value: i64,
    children: [i64; 10], // Indices into node array
    child_count: i64,
}

struct ParserState {
    tokens: [Token; MAX_TOKENS],
    token_count: i64,
    current: i64,
    nodes: [ASTNode; 1000],
    node_count: i64,
}

fn parse(tokens: [Token; MAX_TOKENS], count: i64) -> ParserState {
    // Return complete parser state with AST
}
```

### 3. Ultra-Minimal CodeGen (bootstrap3/codegen.pd)
```palladium
fn generate(ast: ParserState) -> String {
    // Generate C code from AST
}
```

### 4. Main Driver (bootstrap3/pdc.pd)
```palladium
fn main() {
    let filename = "input.pd";
    let source = file_read_all(file_open(filename));
    
    let lexer_state = lex(source);
    let parser_state = parse(lexer_state.tokens, lexer_state.token_count);
    let c_code = generate(parser_state);
    
    let out_file = file_open("output.c");
    file_write(out_file, c_code);
    file_close(out_file);
}
```

## Key Design Decisions

1. **No Mutable References**: Instead of `&mut`, we either:
   - Use global state (simpler but less clean)
   - Return updated structures (functional style)

2. **Fixed-Size Arrays**: All collections use fixed maximum sizes.

3. **Simple Error Handling**: Just print errors and return early.

4. **Integer-Based Strings**: Store strings as arrays of integers (ASCII values).

5. **Minimal Type System**: Only support what's needed to compile itself.

## Estimated Timeline

- **Day 1**: Ultra-minimal lexer ✅
- **Day 2**: Ultra-minimal parser (in progress)
- **Day 3**: Ultra-minimal codegen and integration
- **Day 4**: Self-compilation and testing
- **Day 5**: Enhanced compiler development

## Success Criteria

1. The ultra-minimal compiler can compile ultra_minimal.pd ✅
2. The ultra-minimal compiler can compile itself
3. The self-compiled compiler produces identical output
4. No dependency on Rust compiler for future development

## Current Blockers

1. ~~References in function parameters~~ → Use global state or return values
2. ~~Dynamic arrays (Vec)~~ → Use fixed-size arrays
3. ~~Pattern matching~~ → Use if/else chains
4. ~~Complex enums~~ → Use simple integer tags

We are **very close** to achieving self-hosting!