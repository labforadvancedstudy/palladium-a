# Palladium Language Tests

This directory contains comprehensive tests for the Palladium programming language, organized by language features according to the official specification.

## Test Categories

### 01. Lexical Structure
- `01_lexical_literals.pd` - Integer, string, and boolean literals
- `01_lexical_comments.pd` - Single-line and block comments

### 02. Type System  
- `02_types_primitives.pd` - Primitive types (i32, i64, bool, String)
- `02_types_arrays.pd` - Array types and operations
- `02_types_structs.pd` - Struct definitions and field access
- `02_types_enums.pd` - Enum variants and data

### 03. Functions
- `03_functions_basic.pd` - Function definitions, parameters, returns

### 04. Control Flow
- `04_control_flow_if.pd` - If/else statements and conditions
- `04_control_flow_while.pd` - While loops, break, continue
- `04_control_flow_for.pd` - For loops with ranges and arrays

### 05. Ownership & Borrowing
- `05_ownership_basic.pd` - Move semantics and ownership transfer
- `05_borrowing_references.pd` - Shared and mutable references

### 06. Pattern Matching
- `06_pattern_matching.pd` - Match expressions with various patterns

### 07. Traits
- `07_traits_basic.pd` - Trait definitions and implementations

### 08. Generics
- `08_generics_basic.pd` - Generic structs, enums, and functions

### 09. Effects System
- `09_effects_system.pd` - Effect annotations (io, fs, net, unsafe)

### 10. Async/Await
- `10_async_await.pd` - Asynchronous functions and futures

### 11. Unsafe
- `11_unsafe_blocks.pd` - Unsafe operations and raw pointers

### 12. Modules
- `12_modules_imports.pd` - Module system and imports

## Running Tests

To run a test file:
```bash
pdc run tests/01_lexical_literals.pd
```

Or compile first:
```bash
pdc compile tests/01_lexical_literals.pd
./build_output/01_lexical_literals
```

## Current Limitations

Based on compiler testing, the following features are not yet implemented:
- Hexadecimal and binary integer literals (0xFF, 0b1010)
- Very large integer literals (u64 max value)
- Unit type `()`
- Nested block comments `/* /* */ */`
- Inline block comments `print(/* comment */ "text")`
- 2D/nested arrays `[[i64; 3]; 2]`

## Test Status

✅ Working:
- Basic literals (decimal integers, strings, booleans)
- Single-line and block comments
- Primitive types (i32, i64, bool, String)
- 1D arrays with basic operations
- Structs with field access
- Functions with parameters and returns
- If/else statements (no else-if)
- While loops with break/continue
- For loops with ranges and arrays (integers only)
- Basic ownership (moves for non-Copy types)
- Function references (&T, &mut T)

⚠️  Partially Working:
- Integer literals (decimal only, no hex/binary)
- Arrays (1D only, no nested arrays, no string arrays in for loops)
- Comments (no nested blocks, no inline blocks)
- Pattern matching (enum variants only, no literals/ranges/wildcards)
- References (no direct reference variables, only in function calls)
- Mutable parameters (passed by reference implicitly)

❌ Not Yet Implemented:
- Unit type `()`
- Unit structs
- Else-if chains
- If expressions
- Match expressions (statement only)
- Literal patterns in match
- Range patterns
- Wildcard patterns `_`
- Nested patterns
- Traits (syntax exists but not functional)
- Generics (partial implementation with issues)
- Effects system (tracked internally but no syntax)
- Async/await (no runtime)
- Unsafe blocks and raw pointers
- Module system and imports
- String concatenation with `+`