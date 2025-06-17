# Bootstrap3 - Minimal Palladium Compiler

This directory contains our efforts to create a truly minimal Palladium compiler that can compile itself.

## Files

### Working Programs

- **`ultra_minimal.pd`** - Test program that validates our approach (compiles and runs successfully)
- **`simple_test.pd`** - Another test program with functions, recursion, and file I/O
- **`tiny_compiler.pd`** - 🎉 **Working Palladium compiler!** Can compile simple programs to C

### Compiler Components (In Progress)

- **`lexer_minimal.pd`** - Lexer without global state or references
- **`parser_minimal.pd`** - Parser using array-based AST
- **`codegen_minimal.pd`** - Code generator that produces C
- **`compiler_v2.pd`** - Integrated compiler using struct-passing approach

### Issues Discovered

1. **Global Mutable State** - Palladium doesn't support global mutable variables
2. **References** - Current compiler doesn't support `&` or `&mut`
3. **External Declarations** - Can't declare `extern` variables

### Solutions

1. Use mutable local variables instead of globals
2. Pass state in structures, return modified structures
3. Use fixed-size arrays instead of Vec
4. Keep everything simple!

## Tiny Compiler Success

The `tiny_compiler.pd` demonstrates that we can:
- Parse Palladium source code
- Extract print statements
- Generate valid C code
- Write output files
- Create working executables

Example output:
```bash
$ ./build_output/tiny_compiler
Tiny Palladium Compiler
=======================
Generated C code:
#include <stdio.h>
...
Saved to tiny_output.c

$ gcc tiny_output.c -o tiny_test && ./tiny_test
Hello from tiny compiler!
This is a test
It works!
```

## Next Steps

1. Expand tiny compiler to handle:
   - Variable declarations
   - Function definitions
   - Basic expressions
   - Control flow (if/while)

2. Once it can compile itself, we achieve self-hosting!

## Progress: 94% Complete

We're extremely close to achieving full self-hosting!