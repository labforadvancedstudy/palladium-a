# Palladium Examples

This directory contains example Palladium programs organized by category.

## Directory Structure

### 📚 basic/
Basic language features and syntax examples:
- `hello.pd` - Classic Hello World
- `function_params.pd` - Function parameter examples
- `function_returns.pd` - Function return value examples
- `arrays_basic.pd` - Basic array usage
- `mutability.pd` - Mutable vs immutable variables
- `while_loop.pd` - While loop examples
- `arithmetic.pd` - Basic arithmetic operations
- `conditions.pd` - If/else conditions

### 🏗️ data_structures/
Data structure implementations and patterns:
- `struct_design.pd` - Struct definitions and usage
- `enum_design.pd` - Enum types and variants
- `pattern_matching.pd` - Pattern matching examples
- `string_design.pd` - String manipulation
- `result_design.pd` - Result type for error handling
- `file_io_design.pd` - File I/O operations

### 🔧 algorithms/
Algorithm implementations:
- `bubble_sort.pd` - Bubble sort implementation
- `fibonacci_iterative.pd` - Iterative Fibonacci
- `binary_search_simulation.pd` - Binary search
- `prime_checker.pd` - Prime number checking
- `splay_tree_palladium.pd` - Splay tree data structure

### 🧪 testing/
Test examples and verification:
- `test_result.pd` - Result type tests
- `test_option.pd` - Option type tests
- `test_strings.pd` - String manipulation tests
- `test_vec.pd` - Vector implementation tests
- `test_file_io.pd` - File I/O tests
- And more...

### 🎮 demo/
Complete demo applications:
- `demo_calculator.pd` - Simple calculator
- `demo_calculator_simple.pd` - Simplified calculator
- `demo_text_processor.pd` - Text processing demo

### 🚀 bootstrap/
Bootstrap compiler components written in Palladium:
- Lexer, parser, and AST implementations
- Self-hosting compiler components

### 📚 stdlib/
Standard library implementations:
- Core data structures
- Utility functions

## Running Examples

To compile and run an example:

```bash
# Compile an example
pdc compile examples/basic/hello.pd

# Run the compiled program
./build_output/hello
```

## Contributing

When adding new examples:
1. Place them in the appropriate category directory
2. Include comments explaining the concepts demonstrated
3. Keep examples focused on a single concept when possible
4. Add a brief description to this README

## Version Notes

Some examples may require specific Palladium versions:
- Files ending in `_v0_2.pd` are from v0.2 and may need updates
- Current examples target v0.3+ features