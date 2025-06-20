# Palladium Examples

This directory contains example programs demonstrating various features of the Palladium programming language.

## Tutorial Examples

Step-by-step introduction to Palladium features:

1. **[01_variables.pd](tutorial/01_variables.pd)** - Variable declarations, mutability, and basic types
2. **[02_functions.pd](tutorial/02_functions.pd)** - Function definitions, parameters, and return values
3. **[03_ownership.pd](tutorial/03_ownership.pd)** - Ownership model, moves, and borrowing
4. **[04_structs.pd](tutorial/04_structs.pd)** - Struct definitions and usage
5. **[05_control_flow.pd](tutorial/05_control_flow.pd)** - If/else, loops, and control flow
6. **[06_arrays.pd](tutorial/06_arrays.pd)** - Array operations and iteration

## Practical Examples

Complete programs demonstrating real-world usage:

- **[calculator.pd](practical/calculator.pd)** - Basic arithmetic calculator with error handling
- **[bubble_sort.pd](practical/bubble_sort.pd)** - Bubble sort algorithm implementation
- **[fibonacci.pd](practical/fibonacci.pd)** - Fibonacci sequence generator (recursive and iterative)
- **[prime_checker.pd](practical/prime_checker.pd)** - Prime number checker and generator

## Running Examples

To run any example:

```bash
pdc run examples/tutorial/01_variables.pd
```

Or compile first:

```bash
pdc compile examples/tutorial/01_variables.pd
./build_output/01_variables
```

## Language Features Demonstrated

### Working Features ✅
- Variable declarations (immutable and mutable)
- Basic types (i64, i32, bool, String)
- Functions with parameters and returns
- Structs with fields
- Arrays (1D, fixed size)
- For loops with ranges and arrays
- While loops with break/continue
- If/else conditionals
- Basic ownership and borrowing
- References (&T, &mut T)

### Limitations ⚠️
- No string concatenation with `+`
- No else-if syntax (use nested if)
- No match expressions (statements only)
- No generic types
- No traits
- No modules/imports
- Arrays are fixed-size only
- Enums have limited support

## Key Differences from Rust

- **Simpler type system**: No lifetimes, simpler strings
- **No trait system**: Functions instead of impl blocks
- **Fixed arrays only**: No Vec<T> or dynamic collections
- **Basic pattern matching**: Limited to enum variants
- **Explicit returns**: No implicit returns, must use `return`

## Contributing

When adding new examples:
1. Test with `pdc run` before committing
2. Add comments explaining the concepts
3. Keep examples focused on specific features
4. Update this README with new examples