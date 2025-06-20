# Palladium Language Reference

Complete reference documentation for the Palladium programming language.

## 📖 Reference Sections

### [Language Reference Manual](./LANGUAGE_REFERENCE.md)
The comprehensive language reference covering:
- Syntax and grammar
- Type system
- Control flow
- Pattern matching
- Module system
- Memory model

### [Standard Library](./README.md)
Complete API documentation for:
- Built-in functions
- Standard modules (std::*)
- Extended libraries (stdlib::*)
- Code examples and best practices

### [Language Features](./features/)
Detailed specifications for language features:
- [Async as Effect System](./features/async-system/async-as-effect.md)
- [Implicit Lifetimes](./features/core-language/implicit-lifetimes.md)
- [Totality Checking](./features/advanced/totality-checking.md)
- [Feature Status](./features/status.yaml)

### Standard Library Modules

#### Core Modules
- [Built-in Functions](./builtin.md) - Global functions available without imports
- [Math Module](./math.md) - Mathematical operations
- [String Utilities](./string_utils.md) - String manipulation functions

#### Data Structures
- [Collections](./collections.md) - Vectors, HashMaps, and more
- [Error Handling](./error_handling.md) - Option and Result types

### Implementation Notes
- [Compiler Porting Plan](./compiler_porting_plan.md) - Roadmap for self-hosting

## 🔍 Quick Reference

### Basic Types
```palladium
i64         // 64-bit signed integer
bool        // Boolean (true/false)
String      // UTF-8 string
[T; N]      // Fixed-size array
```

### Common Functions
```palladium
print(s: String)                    // Print to stdout
string_len(s: String) -> i64        // String length
file_open(path: String) -> i64      // Open file
string_concat(s1: String, s2: String) -> String  // Concatenate
```

### Control Flow
```palladium
if condition { ... } else { ... }
while condition { ... }
for i in 0..10 { ... }
match value { ... }
```

## 📚 See Also

- [User Guide](../user-guide/) - Tutorials and learning materials
- [Language Specification](../specification/) - Formal language spec
- [Internals](../internals/) - Compiler implementation details