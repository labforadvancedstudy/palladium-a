# Syntax Analysis: Failing Advanced E2E Tests

## Summary of Missing Language Features

Based on the failing tests in `tests/advanced_e2e_test.rs`, here are the specific syntax issues that need to be addressed:

## 1. `test_option_enum` - `&self` Parameter Syntax

**Error**: `Expected parameter name, but found '&'`

**Location**: Line 174 in test code
```palladium
fn is_some(&self) -> bool {
```

**Issue**: The parser doesn't recognize `&self` as a valid parameter. Currently, it expects:
- Optional `mut` keyword
- Parameter name (identifier)
- Colon
- Type

**Required Changes**:
- Add `self` as a keyword token in lexer
- Modify parameter parsing to handle special cases:
  - `self` (by value)
  - `&self` (immutable reference)
  - `&mut self` (mutable reference)

## 2. `test_const_generics_arrays` - Const Generic Parameters

**Error**: `Expected type parameter name, but found 'const'`

**Location**: Line 501 in test code
```palladium
impl<T, const ROWS: int, const COLS: int> Matrix<T, ROWS, COLS> {
```

**Issue**: While the parser has code to handle const generics (it checks for `Token::Const`), it seems to be failing when parsing them in impl blocks.

**Current State**: The parser does have const generic support in `parse_generic_params()`, but it may not be working correctly in all contexts.

## 3. `test_result_error_handling` - Missing Semicolon After Expression

**Error**: `Expected ';' (Expected ';' after expression), but found '}'`

**Location**: Lines 230-234 in test code
```palladium
if s == "42" {
    Result::Ok(42)
} else {
    Result::Err("Not a valid number")
}
```

**Issue**: The parser expects semicolons after expressions in block contexts. This is likely because:
- The if-else expression is the last expression in the function
- Palladium should support implicit return (last expression without semicolon)

**Required Changes**:
- Support implicit return in blocks
- Distinguish between expressions that need semicolons and those that don't

## 4. `test_type_aliases_complex` - Tuple Type Syntax

**Error**: `Expected ')' (Expected ')' for unit type), but found identifier 'NodeId'`

**Location**: Line 552 in test code
```palladium
type Graph = [(NodeId, NodeId, Weight); 100];
```

**Issue**: The parser doesn't properly support tuple types. When it sees `(`, it expects:
- An empty tuple `()` (unit type)
- But not tuple types with multiple elements like `(T1, T2, T3)`

**Required Changes**:
- Add proper tuple type parsing
- Support for n-element tuples: `(T1)`, `(T1, T2)`, `(T1, T2, T3)`, etc.

## 5. `test_pattern_matching_guards` - Field Pattern Syntax

**Error**: `Expected ':' (Expected ':' after field name in pattern), but found ','`

**Location**: Line 449 in test code
```palladium
Message::Move { x, y } if x > 0 && y > 0 => {
```

**Issue**: The parser expects field patterns to be in the form `field: pattern`, but Rust-style shorthand field patterns `{ x, y }` (where the pattern name matches the field name) are not supported.

**Required Changes**:
- Support shorthand field patterns in match expressions
- Allow `{ field }` as shorthand for `{ field: field }`

## Priority Order for Implementation

1. **Tuple Types** - Fundamental type system feature needed for many advanced features
2. **&self Parameters** - Critical for method implementations and OOP-style code
3. **Shorthand Field Patterns** - Important for ergonomic pattern matching
4. **Implicit Return** - Nice quality of life feature for expression-oriented programming
5. **Const Generics** - Advanced feature, parser code exists but needs debugging

## Additional Observations

- The parser is well-structured and already has placeholders for many of these features
- Most issues are about recognizing specific syntax patterns rather than fundamental architectural problems
- The error messages are clear and helpful for debugging

## Recommendations

1. Start with tuple type support as it's a fundamental feature
2. Add `self` keyword and special parameter handling
3. Enhance pattern matching to support shorthand syntax
4. Debug the existing const generics implementation
5. Consider adding more comprehensive parser tests for each syntax feature