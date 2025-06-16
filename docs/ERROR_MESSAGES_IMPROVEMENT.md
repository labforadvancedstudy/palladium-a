# Error Messages Improvement

## Current State

Our error messages are functional but could be more helpful:

```
❌ Compilation failed: Type mismatch: expected Int, found String
```

## Goal

Rust-like error messages with:
- Source location with line numbers
- Context showing the problematic code
- Helpful suggestions

```
error[E0308]: type mismatch
 --> examples/type_error_test.pd:3:18
  |
3 |     let x: i64 = "hello";
  |            ---   ^^^^^^^ expected `i64`, found `String`
  |            |
  |            expected due to this
  |
help: strings cannot be assigned to integer variables
```

## Implementation Plan

### Phase 1: Track Spans (Partially Done) ✅
- Parser already tracks spans for most constructs
- Need to propagate spans through type checking

### Phase 2: Error Reporter (Done) ✅
- Created `ErrorReporter` struct
- Can show source snippets with error indicators

### Phase 3: Integrate Reporter
- Modify driver to use ErrorReporter
- Convert CompileError to Diagnostic
- Show source context

### Phase 4: Add Suggestions
- Common typos (pritn → print)
- Missing imports
- Type conversion hints

## Examples of Improved Errors

### Type Mismatch
```
error: type mismatch
 --> test.pd:5:13
  |
5 |     let z = y + "world";
  |             - ^ ^^^^^^^ cannot add String to Int
  |             |
  |             Int
  |
note: the + operator works differently for numbers and strings
help: convert y to a string first, or use a number instead of "world"
```

### Undefined Variable
```
error: undefined variable
 --> test.pd:7:11
  |
7 |     print(undefined_var);
  |           ^^^^^^^^^^^^^ not found in this scope
  |
help: did you mean 'defined_var'?
```

### Wrong Argument Count
```
error: wrong number of arguments
 --> test.pd:9:5
  |
9 |     print_int();
  |     ^^^^^^^^^-- expected 1 argument
  |
note: function signature: print_int(value: i64)