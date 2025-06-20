# Palladium Standard Library Documentation Index

Welcome to the Palladium Standard Library documentation. This index provides quick access to all standard library documentation.

## Quick Links

- [Complete Overview](README.md) - Full standard library overview with examples
- [Built-in Functions](builtin.md) - Global functions available without imports
- [Collections](collections.md) - VecInt, HashMap, and StringBuilder
- [Error Handling](error_handling.md) - Option and Result types
- [String Utilities](string_utils.md) - String manipulation functions

## Documentation by Category

### Core Functionality

#### [Built-in Functions](builtin.md)
- **I/O**: `print`, `print_int`
- **Strings**: `string_len`, `string_concat`, `string_char_at`, etc.
- **Type Conversion**: `int_to_string`, `string_to_int`
- **Character Classification**: `char_is_digit`, `char_is_alpha`, `char_is_whitespace`
- **File I/O**: `file_open`, `file_read_all`, `file_write`, etc.

### Data Structures

#### [Collections](collections.md)
- **VecInt**: Dynamic array for integers
  - Creation, modification, querying
  - Sorting, reversal, aggregation
- **HashMap**: String-to-integer mapping
  - Insert, get, remove operations
  - Key existence checking
- **StringBuilder**: Efficient string building
  - Append operations for strings, chars, integers
  - Conversion to final string

### Error Handling

#### [Option and Result Types](error_handling.md)
- **Option Types**: For nullable values
  - `OptionInt`, `OptionString`, `OptionBool`
  - Safe unwrapping with defaults
- **Result Types**: For fallible operations
  - `IntResult`, `StringResult`, `FileResult`
  - Error propagation patterns

### String Processing

#### [String Utilities](string_utils.md)
- **Basic Operations** (std::string)
  - Trimming: `trim`, `trim_start`, `trim_end`
  - Matching: `starts_with`, `ends_with`, `contains`
- **Extended Operations** (stdlib::string_utils)
  - Splitting: `string_split_first`, `string_split_rest`
  - Case conversion: `string_to_upper`, `string_to_lower`
  - Transformation: `string_replace_first`, `string_reverse`
  - Padding: `string_pad_left`, `string_pad_right`

## Quick Reference

### Most Used Functions

```palladium
// I/O
print("Hello, World!");
print_int(42);

// Strings
let len = string_len("hello");
let concat = "Hello, " + "World!";
let ch = string_char_at("hello", 0);

// Files
if file_exists("data.txt") {
    let handle = file_open("data.txt");
    let content = file_read_all(handle);
    file_close(handle);
}

// Collections
let mut vec = vec_int_new();
vec = vec_int_push(vec, 42);

// Error Handling
match parse_int_safe("123") {
    OptionInt::Some(n) => print_int(n),
    OptionInt::None => print("Invalid")
}
```

### Import Statements

```palladium
// Standard modules
import std::math;
import std::string;

// Extended libraries
import stdlib::string_utils;
import stdlib::option;
import stdlib::result;
import stdlib::vec_simple;
import stdlib::string_builder;
import stdlib::hashmap_simple;
```

## Module Organization

The standard library is organized into two main categories:

1. **Built-in Functions**: Available globally, implemented in the runtime
2. **Standard Modules**: Must be imported, implemented in Palladium

### Module Hierarchy

```
Built-in (Global)
├── I/O Functions
├── String Operations
├── Character Classification
└── File I/O

std::
├── math           - Mathematical operations
└── string         - Basic string utilities

stdlib::
├── string_utils   - Extended string operations
├── option         - Optional values
├── result         - Error handling
├── vec_simple     - Dynamic arrays
├── string_builder - String concatenation
└── hashmap_simple - Hash maps
```

## Common Tasks

### Reading a Configuration File
See: [Error Handling Examples](error_handling.md#complete-examples)

### Building Complex Strings
See: [StringBuilder Examples](collections.md#stringbuilder---efficient-string-building)

### Parsing User Input
See: [Option Type Examples](error_handling.md#robust-number-input)

### Processing Text Files
See: [String Utilities Examples](string_utils.md#complete-examples)

## Best Practices

1. **Import only what you need** - Don't import entire modules if you only need one function
2. **Handle errors properly** - Use Option/Result types instead of error-prone defaults
3. **Use appropriate data structures** - StringBuilder for concatenation, Vec for lists
4. **Check function return values** - Especially for file operations
5. **Validate input** - Use safe parsing functions and check bounds

## Future Additions

The standard library is actively being developed. Planned additions include:

- Generic collections (when generics are added to the language)
- Network I/O operations
- Regular expressions
- JSON/XML parsing
- Concurrent programming primitives
- Advanced mathematical functions
- Date and time handling

## Contributing

To contribute to the standard library:

1. Follow the existing naming conventions
2. Provide comprehensive documentation
3. Include usage examples
4. Consider bootstrap compiler compatibility
5. Write tests for new functionality

## Version Information

This documentation covers the Palladium Standard Library as of the 100% bootstrap achievement (June 2025).

For the latest updates, check the [main repository](https://github.com/palladium-lang/palladium).