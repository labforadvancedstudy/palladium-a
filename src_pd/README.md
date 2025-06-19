# Palladium Self-Hosting Compiler

This directory contains the Palladium compiler written in Palladium itself - a major milestone in the language's development!

## Overview

The self-hosting compiler consists of several modules:

1. **lexer.pd** (878 lines) - Tokenizes Palladium source code
2. **ast.pd** (652 lines) - Defines the Abstract Syntax Tree
3. **parser.pd** (1,461 lines) - Recursive descent parser
4. **typeck.pd** (744 lines) - Type checking and inference
5. **codegen.pd** (714 lines) - C code generation backend
6. **main.pd** (176 lines) - Compiler driver

Total: ~4,625 lines of pure Palladium code!

## Features Implemented

### Lexical Analysis
- All Palladium keywords and operators
- String and character literals with escape sequences
- Integer and floating-point numbers
- Comments (single-line and multi-line)
- Proper line/column tracking for error reporting

### Parsing
- Function declarations with parameters and return types
- Variable declarations with type annotations
- Control flow: if/else, while loops
- Expressions with proper precedence
- Arrays and array indexing
- Struct declarations and field access
- Pattern matching in let bindings
- Block expressions

### Type System
- Primitive types: i8-i64, u8-u64, f32, f64, bool, char, String
- Array types with compile-time sizes
- Reference types (&T and &mut T)
- Function types
- Struct and enum types
- Type inference for let bindings
- Type checking for all expressions

### Code Generation
- Generates portable C code
- Runtime functions for strings and I/O
- Proper name mangling
- Support for all implemented features

## Building the Self-Hosting Compiler

```bash
# Step 1: Use bootstrap compiler to compile the self-hosting compiler
./build_self_hosting.sh

# Step 2: Use the self-hosting compiler to compile a program
./pdc input.pd -o output.c

# Step 3: Compile the generated C code
gcc output.c -o program

# Step 4: Run the program
./program
```

## Testing

The `tests/` directory contains comprehensive test programs:

- **test_basic.pd** - Basic functions and arithmetic
- **test_control_flow.pd** - If/else, loops, recursion
- **test_arrays.pd** - Array operations
- **test_structs.pd** - Struct declarations and usage
- **test_strings.pd** - String manipulation
- **test_compiler_features.pd** - Comprehensive feature test

Run all tests:
```bash
cd tests
./run_tests.sh
```

## Architecture

### Compilation Pipeline

1. **Lexical Analysis**: Source text → Tokens
2. **Parsing**: Tokens → AST
3. **Type Checking**: AST → Typed AST
4. **Code Generation**: Typed AST → C code

### Key Design Decisions

- **Fixed-size arrays**: Due to current language limitations
- **C backend**: For maximum portability
- **Recursive descent parser**: Simple and effective
- **Structural type system**: With nominal types for structs/enums

## Current Limitations

The self-hosting compiler currently has some limitations due to missing language features:

1. **No dynamic memory allocation** - Uses fixed-size arrays
2. **No match expressions** - Uses if/else chains
3. **No for..in loops** - Uses while loops with counters
4. **No traits** - Uses structural typing
5. **Limited error recovery** - Basic synchronization

## Future Improvements

1. Implement missing language features:
   - Pattern matching for enums
   - For loops with iterators
   - Trait system
   - Generic types

2. Optimize generated code:
   - Better register allocation
   - Constant folding
   - Dead code elimination

3. Improve error messages:
   - Better error recovery
   - Suggestions for fixes
   - Colored output

4. Add more backends:
   - LLVM IR generation
   - Direct machine code
   - WebAssembly

## Contributing

To contribute to the self-hosting compiler:

1. Ensure your code is written in pure Palladium
2. Follow the existing code style
3. Add tests for new features
4. Update documentation

## License

The Palladium self-hosting compiler is part of the Palladium project.