# The Palladium Programming Language Specification

**Version**: 1.0.0-alpha  
**Date**: January 19, 2025  
**Authors**: The Palladium Team

## Table of Contents

1. [Introduction](#introduction)
2. [Notation](#notation)
3. [Lexical Structure](#lexical-structure)
4. [Syntax](#syntax)
5. [Type System](#type-system)
6. [Memory Model](#memory-model)
7. [Execution Model](#execution-model)
8. [Standard Library](#standard-library)
9. [Appendices](#appendices)

## 1. Introduction

Palladium is a systems programming language designed to combine the correctness guarantees of functional programming with the performance characteristics of systems languages. This specification defines the syntax, semantics, and behavior of the Palladium programming language.

### 1.1 Design Goals

- **Memory Safety**: Prevent common memory errors without garbage collection
- **Type Safety**: Strong static typing with inference
- **Performance**: Zero-cost abstractions and predictable performance
- **Expressiveness**: Modern language features for productive programming
- **Simplicity**: Clear and consistent language design

### 1.2 Terminology

- **shall**: Indicates a requirement
- **should**: Indicates a recommendation
- **may**: Indicates permission
- **undefined behavior**: Behavior for which this specification imposes no requirements

## 2. Notation

### 2.1 Grammar Notation

The syntax is specified using Extended Backus-Naur Form (EBNF):

```ebnf
|   alternation
()  grouping
[]  option (0 or 1 times)
{}  repetition (0 or more times)
+   repetition (1 or more times)
```

### 2.2 Lexical Notation

- `'x'` - literal character
- `"xyz"` - literal string
- `a-z` - character range
- `\x` - escape sequence

## 3. Lexical Structure

### 3.1 Source Code Representation

Palladium source code is Unicode text encoded in UTF-8. A source file consists of a sequence of Unicode code points.

### 3.2 Whitespace

```ebnf
whitespace = ' ' | '\t' | '\n' | '\r'
```

Whitespace is generally insignificant except for separating tokens.

### 3.3 Comments

```ebnf
line_comment = "//" {any_char_except_newline}
block_comment = "/*" {any_char} "*/"
```

Comments are treated as whitespace.

### 3.4 Identifiers

```ebnf
identifier = (letter | '_') {letter | digit | '_'}
letter = 'a'-'z' | 'A'-'Z'
digit = '0'-'9'
```

Identifiers are case-sensitive.

### 3.5 Keywords

Reserved words that cannot be used as identifiers:

```
async     await     break     const     continue
effect    else      enum      false     fn
for       if        impl      let       match
mod       mut       pub       return    self
struct    trait     true      type      unsafe
use       while     Self      as        in
```

### 3.6 Literals

#### 3.6.1 Integer Literals

```ebnf
integer_literal = decimal_literal | hex_literal | binary_literal
decimal_literal = digit+
hex_literal = "0x" hex_digit+
binary_literal = "0b" binary_digit+
hex_digit = digit | 'a'-'f' | 'A'-'F'
binary_digit = '0' | '1'
```

#### 3.6.2 String Literals

```ebnf
string_literal = '"' {string_char | escape_sequence} '"'
string_char = any_unicode_char_except_quote_or_backslash
escape_sequence = '\' ('n' | 'r' | 't' | '\' | '"' | '0')
```

#### 3.6.3 Boolean Literals

```ebnf
boolean_literal = "true" | "false"
```

### 3.7 Operators and Punctuation

```
+    -    *    /    %    =    ==   !=
<    >    <=   >=   &&   ||   !    &
|    ^    <<   >>   +=   -=   *=   /=
%=   &=   |=   ^=   <<=  >>=  ->   ::
.    ,    ;    :    (    )    [    ]
{    }    ?    @    #    ~    ..   ...
```

## 4. Syntax

### 4.1 Program Structure

```ebnf
program = {import} {item}
import = "use" path ["as" identifier] ";"
path = identifier {"::" identifier}
```

### 4.2 Items

```ebnf
item = function
     | struct_def
     | enum_def
     | trait_def
     | impl_block
     | type_alias
     | macro_def

visibility = ["pub"]
```

### 4.3 Functions

```ebnf
function = visibility ["async"] "fn" identifier 
           [generic_params] "(" [param_list] ")" 
           ["->" type] [effect_clause] block

param_list = param {"," param} [","]
param = ["mut"] identifier ":" type
effect_clause = "!" "[" effect_list "]"
effect_list = identifier {"," identifier}
```

### 4.4 Types

```ebnf
type = primitive_type
     | array_type
     | reference_type
     | generic_type
     | function_type
     | future_type
     | path_type

primitive_type = "i32" | "i64" | "u32" | "u64" 
               | "bool" | "String" | "()"

array_type = "[" type ";" array_size "]"
array_size = integer_literal | identifier | expression

reference_type = "&" ["'" identifier] ["mut"] type

generic_type = identifier "<" generic_args ">"
generic_args = generic_arg {"," generic_arg}
generic_arg = type | integer_literal | identifier

function_type = "fn" "(" [type_list] ")" ["->" type]
future_type = "Future" "<" type ">"
path_type = path
```

### 4.5 Statements

```ebnf
statement = let_statement
          | expression_statement
          | return_statement
          | assignment_statement
          | if_statement
          | while_statement
          | for_statement
          | match_statement
          | break_statement
          | continue_statement
          | unsafe_block

let_statement = "let" ["mut"] identifier [":" type] "=" expression ";"
expression_statement = expression ";"
return_statement = "return" [expression] ";"
assignment_statement = assign_target "=" expression ";"
```

### 4.6 Expressions

```ebnf
expression = literal_expr
           | identifier_expr
           | array_expr
           | call_expr
           | field_expr
           | index_expr
           | unary_expr
           | binary_expr
           | if_expr
           | match_expr
           | struct_expr
           | block_expr
           | await_expr
           | unsafe_expr

precedence (highest to lowest):
1. Postfix: () [] . ?
2. Unary: ! - & &mut *
3. Multiplicative: * / %
4. Additive: + -
5. Shift: << >>
6. Relational: < > <= >=
7. Equality: == !=
8. Bitwise AND: &
9. Bitwise XOR: ^
10. Bitwise OR: |
11. Logical AND: &&
12. Logical OR: ||
13. Assignment: = += -= *= /= %= etc.
```

## 5. Type System

### 5.1 Type Categories

Palladium types fall into several categories:

1. **Scalar Types**: Integers, booleans
2. **Compound Types**: Arrays, structs, enums
3. **Reference Types**: Shared and mutable references
4. **Function Types**: Function pointers and closures
5. **Pointer Types**: Raw pointers (unsafe)

### 5.2 Type Inference

Palladium performs bidirectional type inference:

- **Top-down**: Expected types flow from context
- **Bottom-up**: Types synthesized from expressions

### 5.3 Lifetime System

References have lifetimes that ensure memory safety:

```palladium
fn example<'a>(x: &'a i32) -> &'a i32 {
    x  // Lifetime 'a flows through
}
```

### 5.4 Generic Types

```ebnf
generic_params = "<" generic_param {"," generic_param} ">"
generic_param = lifetime_param
              | type_param
              | const_param

lifetime_param = "'" identifier
type_param = identifier [":" type_bounds]
const_param = "const" identifier ":" type
```

### 5.5 Trait System

Traits define shared behavior:

```palladium
trait Display {
    fn fmt(&self) -> String;
}

impl Display for i32 {
    fn fmt(&self) -> String {
        int_to_string(*self)
    }
}
```

### 5.6 Type Safety Rules

1. **No implicit conversions** between types
2. **No null pointers** - use Option<T> instead
3. **No uninitialized values** - all variables must be initialized
4. **No data races** - enforced by ownership system

## 6. Memory Model

### 6.1 Ownership

Every value has a single owner:

1. Each value has exactly one owner
2. When the owner goes out of scope, the value is dropped
3. Values can be moved between owners

### 6.2 Borrowing

References allow temporary access:

1. **Shared references** (`&T`): Multiple allowed, read-only
2. **Mutable references** (`&mut T`): Exactly one allowed, read-write

### 6.3 Memory Layout

```
Primitive types: Stored inline
Arrays: Contiguous memory, known size
Structs: Fields in declaration order
Enums: Tag + largest variant
References: Pointer-sized
```

### 6.4 Memory Safety Guarantees

1. **No use-after-free**: Lifetime checking prevents dangling references
2. **No double-free**: Ownership system ensures single owner
3. **No null pointer dereferences**: No null pointers in safe code
4. **No data races**: Borrowing rules prevent concurrent mutation

## 7. Execution Model

### 7.1 Program Execution

1. Program starts at `main` function
2. Static initialization occurs before `main`
3. Program terminates when `main` returns

### 7.2 Function Calls

- Arguments evaluated left-to-right
- Pass-by-value semantics (move or copy)
- Stack-based activation records

### 7.3 Control Flow

```ebnf
if_statement = "if" expression block ["else" (if_statement | block)]
while_statement = "while" expression block
for_statement = "for" identifier "in" expression block
match_statement = "match" expression "{" match_arm+ "}"
match_arm = pattern "=>" (expression | block) [","]
```

### 7.4 Pattern Matching

```ebnf
pattern = literal_pattern
        | identifier_pattern
        | wildcard_pattern
        | struct_pattern
        | enum_pattern
        | array_pattern

wildcard_pattern = "_"
struct_pattern = path "{" field_pattern {"," field_pattern} "}"
enum_pattern = path ["(" pattern {"," pattern} ")"]
```

### 7.5 Effects System

Effects track computational effects:

```palladium
fn read_file(path: String) -> String ![io] {
    // Function has IO effect
}

async fn fetch_data(url: String) -> String {
    // Async functions have implicit async effect
}
```

## 8. Standard Library

### 8.1 Prelude

Automatically imported items:

```palladium
// Basic types
type String = String;
type Vec<T> = Vec<T>;
type Option<T> = enum { Some(T), None };
type Result<T, E> = enum { Ok(T), Err(E) };

// Essential functions
fn print(s: String);
fn print_int(n: i64);
```

### 8.2 Core Modules

- `std::math` - Mathematical functions
- `std::string` - String manipulation
- `std::vec` - Dynamic arrays
- `std::io` - Input/output operations
- `std::mem` - Memory utilities

### 8.3 Runtime Functions

Built-in functions provided by runtime:

```c
// Memory management
void* pd_alloc(size_t size);
void pd_free(void* ptr);

// String operations
int pd_string_len(const char* s);
char* pd_string_concat(const char* a, const char* b);

// IO operations
void pd_print(const char* s);
void pd_print_int(int64_t n);
```

## 9. Appendices

### 9.1 Grammar Summary

Complete EBNF grammar available in `grammar.ebnf`.

### 9.2 Reserved for Future Use

These identifiers are reserved for potential future use:

```
abstract  become   box      do       final
macro     move     override priv     pure
static    typeof   virtual  where    yield
```

### 9.3 Undefined Behavior

The following operations result in undefined behavior:

1. Dereferencing null or dangling pointers
2. Data races in unsafe code
3. Integer overflow in unsafe code
4. Out-of-bounds array access in unsafe code

### 9.4 Implementation Limits

Implementations shall support at least:

- Identifiers: 255 characters
- String literals: 65,535 characters
- Function parameters: 255
- Nested blocks: 127 levels
- Array size: 2^31 - 1 elements

### 9.5 Compatibility

This specification defines Palladium 1.0. Future versions shall maintain backward compatibility with well-formed programs.

---

## Change Log

- **v1.0.0-alpha** (2025-01-19): Initial specification