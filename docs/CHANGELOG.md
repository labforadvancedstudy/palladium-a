# Changelog

All notable changes to Palladium will be documented in this file.

## [Unreleased]

### Added
- **String Concatenation** - The `+` operator now works for string concatenation! 🎉
  - Type checker supports `String + String -> String`
  - Code generator emits `__pd_string_concat()` calls
  - Works in all contexts: variables, function returns, nested expressions
  - Example: `let greeting = "Hello, " + name + "!";`

### Changed
- AST now includes `imports` and `visibility` fields (preparation for module system)
- Type inference improved for binary expressions

### Fixed
- Variable type inference now correctly handles string concatenation results

## [v1.0-bootstrap] - 2025-01-16

### Added
- Self-hosting achieved with 37 bootstrap compilers
- 6,508 lines of Palladium bootstrap code
- Complete compiler written in Palladium (lexer, parser, type checker, code generator)

### Known Limitations
- No module system (can't organize code across files)
- No generics (limits code reuse)
- Limited file I/O (only reads first line)
- No else-if support
- No continue in loops

## [v0.1-alpha] - 2025-01-01

### Added
- Initial release with core language features
- Basic types: i32, i64, u32, u64, bool, String
- Control flow: if/else, while, for loops
- Pattern matching with exhaustiveness checking
- Structs and enums
- Fixed-size arrays
- Memory safety without GC