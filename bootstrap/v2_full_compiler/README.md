# Bootstrap2 - Palladium Self-Hosting Compiler

This is the Palladium compiler written in Palladium itself! This represents the critical step toward self-hosting.

## Structure

- `lexer.pd` - Tokenizer that breaks source into tokens
- `ast.pd` - Abstract Syntax Tree definitions (simplified)
- `parser.pd` - Recursive descent parser (simplified) 
- `codegen.pd` - C code generator (simplified)
- `pdc.pd` - Main compiler driver

## Current Status

This is a **minimal bootstrap compiler** that supports:
- Basic types (i64, bool, String)
- Functions with parameters and returns
- Let bindings and assignments
- If/else statements
- While loops
- Basic expressions
- Function calls

## Limitations

To achieve bootstrap, we simplified many things:
- No generics
- No pattern matching (except basic)
- No modules/imports (simplified)
- Fixed-size arrays for tokens
- No proper error handling
- No type checking phase

## Usage

```bash
# First compile the bootstrap compiler with current Rust compiler
cargo run -- compile bootstrap2/pdc.pd

# Then use the compiled pdc to compile itself!
./pdc pdc.pd
```

This will achieve true self-hosting!

## Note

This bootstrap compiler is intentionally minimal. Once we achieve self-hosting, we can use this simpler compiler to build progressively more complex versions, eventually reaching feature parity with the Rust implementation.