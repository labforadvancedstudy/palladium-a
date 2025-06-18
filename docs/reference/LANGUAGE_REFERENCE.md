# Palladium Language Reference

## Table of Contents

1. [Overview](#overview)
2. [Basic Syntax](#basic-syntax)
   - [Comments](#comments)
   - [Variables](#variables)
   - [Functions](#functions)
   - [Control Flow](#control-flow)
3. [Types](#types)
   - [Primitive Types](#primitive-types)
   - [Arrays](#arrays)
   - [Structs](#structs)
   - [Enums](#enums)
   - [Pattern Matching](#pattern-matching)
4. [String Operations](#string-operations)
5. [Operators](#operators)
6. [Built-in Functions](#built-in-functions)
7. [Generic Types and Functions](#generic-types-and-functions)
8. [Visibility Modifiers](#visibility-modifiers)
9. [Module System](#module-system)
10. [Memory Management](#memory-management)
11. [Error Handling](#error-handling)
12. [Compilation](#compilation)
13. [Advanced Examples](#advanced-examples)
14. [Language Philosophy](#language-philosophy)
15. [Standard Library](#standard-library)
16. [Language Grammar](#language-grammar)
17. [Self-Hosting](#self-hosting)

## Overview

Palladium is a systems programming language that combines the safety of Rust with the simplicity of Go and the proofs of functional languages.

## Basic Syntax

### Comments
```palladium
// Single-line comment

/*
 * Multi-line comment
 */
```

### Variables
```palladium
let x = 42;              // Immutable by default
let mut y = 100;         // Mutable variable
let name: String = "Ada"; // Explicit type annotation
let nums: [i64; 5] = [1, 2, 3, 4, 5]; // Array
```

### Functions
```palladium
fn add(x: i64, y: i64) -> i64 {
    return x + y;
}

fn greet(name: String) {
    print("Hello, " + name);
}

fn main() {
    let result = add(5, 3);
    print_int(result);
}
```

### Control Flow

#### If/Else
```palladium
if x > 0 {
    print("Positive");
} else if x < 0 {
    print("Negative");
} else {
    print("Zero");
}
```

#### While Loop
```palladium
let mut i = 0;
while i < 10 {
    print_int(i);
    i = i + 1;
}
```

#### For Loop
```palladium
let arr = [1, 2, 3, 4, 5];
for x in arr {
    print_int(x);
}

// With break and continue
for x in numbers {
    if x < 0 {
        continue;  // Skip negative numbers
    }
    if x > 100 {
        break;     // Stop at numbers over 100
    }
    print_int(x);
}
```

#### Break and Continue
```palladium
while true {
    if condition {
        break;    // Exit loop
    }
    if other_condition {
        continue; // Skip to next iteration
    }
}
```

### Types

#### Primitive Types
- `i32` - 32-bit signed integer
- `i64` - 64-bit signed integer
- `u32` - 32-bit unsigned integer
- `u64` - 64-bit unsigned integer
- `bool` - Boolean (true/false)
- `String` - UTF-8 string

#### Arrays
```palladium
let arr: [i64; 3] = [1, 2, 3];     // Array literal
let zeros: [i64; 10] = [0; 10];    // Array repeat syntax
let first = arr[0];                // Array indexing
arr[1] = 5;                        // Array element assignment
```

#### Structs
```palladium
struct Point {
    x: i64,
    y: i64
}

fn main() {
    // Struct literal syntax
    let p = Point { x: 10, y: 20 };
    
    // Field access
    let x_val = p.x;
    
    // Field assignment (requires mut)
    let mut p2 = Point { x: 0, y: 0 };
    p2.x = 30;
    p2.y = 40;
}
```

#### Enums
```palladium
// Unit variants
enum Color {
    Red,
    Green,
    Blue
}

// Tuple variants
enum Option {
    Some(i64),
    None
}

// Struct variants
enum Shape {
    Circle { radius: i64 },
    Rectangle { width: i64, height: i64 },
    Point
}

// Using enums
let opt = Option::Some(42);
let shape = Shape::Circle { radius: 10 };
let color = Color::Red;
```

#### Pattern Matching
```palladium
match value {
    Option::Some(x) => print_int(x),
    Option::None => print("No value"),
}

match shape {
    Shape::Circle { radius } => {
        print("Circle with radius: ");
        print_int(radius);
    }
    Shape::Rectangle { width, height } => {
        print("Rectangle: ");
        print_int(width * height);
    }
    Shape::Point => print("Point"),
    _ => print("Unknown shape")  // Wildcard pattern
}
```

### String Operations

```palladium
let s1 = "Hello";
let s2 = "World";
let s3 = s1 + " " + s2;        // String concatenation

let len = string_len(s3);      // Get length
let ch = string_char_at(s3, 0); // Get character at index
let sub = string_substring(s3, 0, 5); // Substring
```

### Operators

#### Arithmetic Operators
- `+` Addition
- `-` Subtraction
- `*` Multiplication
- `/` Division
- `%` Modulo (remainder)

#### Comparison Operators
- `==` Equal to
- `!=` Not equal to
- `<` Less than
- `>` Greater than
- `<=` Less than or equal to
- `>=` Greater than or equal to

#### Logical Operators
- `&&` Logical AND
- `||` Logical OR
- `!` Logical NOT (unary)

#### Unary Operators
- `-` Negation
- `!` Logical NOT

#### Range Operator
```palladium
let range = 0..10;  // Creates a range from 0 to 9
```

### Built-in Functions

#### I/O Functions
- `print(s: String)` - Print string with newline
- `print_int(n: i64)` - Print integer with newline

#### String Functions
- `string_len(s: String) -> i64` - Get string length
- `string_char_at(s: String, i: i64) -> i64` - Get character code at index
- `string_from_char(c: i64) -> String` - Create string from character code
- `string_concat(s1: String, s2: String) -> String` - Concatenate strings
- `string_substring(s: String, start: i64, end: i64) -> String` - Extract substring
- `int_to_string(n: i64) -> String` - Convert integer to string

#### Type Conversion Functions
- `int_to_string(n: i64) -> String` - Convert integer to string
- `string_to_int(s: String) -> i64` - Parse string to integer (returns 0 on error)

#### Character Classification Functions
- `char_is_digit(c: i64) -> bool` - Check if character is a digit
- `char_is_alpha(c: i64) -> bool` - Check if character is alphabetic
- `char_is_whitespace(c: i64) -> bool` - Check if character is whitespace

#### File I/O Functions
- `file_open(path: String) -> i64` - Open file, returns handle
- `file_read_all(handle: i64) -> String` - Read entire file
- `file_write(handle: i64, content: String) -> bool` - Write to file
- `file_close(handle: i64) -> bool` - Close file
- `file_exists(path: String) -> bool` - Check if file exists

### Generic Types and Functions

```palladium
// Generic function with one type parameter
fn identity<T>(value: T) -> T {
    return value;
}

// Generic function with multiple type parameters
fn swap<T, U>(first: T, second: U) -> (U, T) {
    return (second, first);
}

// Using generic functions
fn main() {
    let x = identity(42);        // T is inferred as i64
    let s = identity("hello");   // T is inferred as String
    
    let (b, a) = swap(10, "text");  // Returns ("text", 10)
}
```

### Visibility Modifiers

```palladium
// Public function - can be accessed from other modules
pub fn public_function() {
    print("I'm public!");
}

// Private function - only accessible within this module
fn private_function() {
    print("I'm private!");
}

// Public struct
pub struct PublicPoint {
    x: i64,
    y: i64
}

// Public enum
pub enum PublicResult {
    Ok(i64),
    Err(String)
}
```

### Module System

```palladium
// math.pd
pub fn abs(x: i64) -> i64 {
    if x < 0 { return -x; }
    return x;
}

pub fn max(a: i64, b: i64) -> i64 {
    if a > b { return a; }
    return b;
}

// main.pd
import std::math;
import std::string;

fn main() {
    let x = math::abs(-5);
    let y = math::max(10, 20);
    
    let s = string::concat("Hello", " World");
}
```

## Memory Management

Palladium uses automatic memory management with ownership rules similar to Rust:
- Values have a single owner
- When the owner goes out of scope, the value is dropped
- References borrow values temporarily

## Error Handling

Palladium uses Result types for error handling (future feature):

```palladium
fn divide(x: i64, y: i64) -> Result<i64, String> {
    if y == 0 {
        return Err("Division by zero");
    }
    return Ok(x / y);
}
```

## Compilation

Palladium compiles to C, which is then compiled to native code:

```bash
pdc compile program.pd    # Generates program.c
gcc program.c -o program  # Compile C to executable
./program                 # Run the program
```

## Advanced Examples

### Option Type Implementation
```palladium
enum Option {
    Some(i64),
    None
}

fn unwrap_or(opt: Option, default: i64) -> i64 {
    match opt {
        Option::Some(value) => value,
        Option::None => default
    }
}
```

### Result Type for Error Handling
```palladium
enum Result {
    Ok(i64),
    Err(String)
}

fn divide(x: i64, y: i64) -> Result {
    if y == 0 {
        return Result::Err("Division by zero");
    }
    return Result::Ok(x / y);
}

fn main() {
    match divide(10, 2) {
        Result::Ok(value) => {
            print("Result: ");
            print_int(value);
        }
        Result::Err(msg) => {
            print("Error: ");
            print(msg);
        }
    }
}
```

### Working with Arrays and Loops
```palladium
fn sum_array(arr: [i64; 5]) -> i64 {
    let mut sum = 0;
    for x in arr {
        sum = sum + x;
    }
    return sum;
}

fn find_max(arr: [i64; 10]) -> i64 {
    let mut max = arr[0];
    let mut i = 1;
    while i < 10 {
        if arr[i] > max {
            max = arr[i];
        }
        i = i + 1;
    }
    return max;
}
```

## Language Philosophy

Palladium aims to combine:
- **Memory Safety**: Ownership-based memory management without garbage collection
- **Type Safety**: Strong static typing with type inference
- **Simplicity**: Clear, readable syntax inspired by Go
- **Performance**: Compiles to C for maximum performance
- **Self-Hosting**: The compiler is written in Palladium itself

## Standard Library

Palladium provides a comprehensive standard library with built-in functions and importable modules.

### Built-in Functions (Global)

These functions are available without any imports:

#### I/O Operations
- `print(s: String)` - Print string with newline
- `print_int(n: i64)` - Print integer with newline

#### String Operations
- `string_len(s: String) -> i64` - Get string length
- `string_char_at(s: String, i: i64) -> i64` - Get character code at index
- `string_from_char(c: i64) -> String` - Create string from character code
- `string_concat(s1: String, s2: String) -> String` - Concatenate strings
- `string_substring(s: String, start: i64, end: i64) -> String` - Extract substring
- `int_to_string(n: i64) -> String` - Convert integer to string
- `string_to_int(s: String) -> i64` - Parse string to integer (returns 0 on error)

#### Character Classification
- `char_is_digit(c: i64) -> bool` - Check if character is a digit
- `char_is_alpha(c: i64) -> bool` - Check if character is alphabetic
- `char_is_whitespace(c: i64) -> bool` - Check if character is whitespace

#### File I/O
- `file_open(path: String) -> i64` - Open file, returns handle
- `file_read_all(handle: i64) -> String` - Read entire file
- `file_write(handle: i64, content: String) -> bool` - Write to file
- `file_close(handle: i64) -> bool` - Close file
- `file_exists(path: String) -> bool` - Check if file exists

### Standard Library Modules

#### Collections (stdlib::vec_simple, stdlib::hashmap_simple)
```palladium
import stdlib::vec_simple;

// Dynamic integer array
let mut vec = vec_int_new();
vec = vec_int_push(vec, 42);
vec = vec_int_push(vec, 100);
let len = vec_int_len(vec);
let first = vec_int_get(vec, 0);
```

#### Error Handling (stdlib::option, stdlib::result)
```palladium
import stdlib::option;

// Safe integer parsing
match parse_int_safe("123") {
    OptionInt::Some(n) => print_int(n),
    OptionInt::None => print("Invalid number")
}
```

#### String Utilities (stdlib::string_utils)
```palladium
import stdlib::string_utils;

let trimmed = string_trim("  hello  ");
let upper = string_to_upper("hello");
let parts = string_split_first("hello,world", ",");
```

## Language Grammar

### Lexical Structure

#### Keywords
```
break continue else enum false fn for if import in let match
pub return struct true while
```

#### Identifiers
```
identifier = letter (letter | digit | '_')*
letter = 'a'..'z' | 'A'..'Z'
digit = '0'..'9'
```

#### Literals
```
integer_literal = digit+
string_literal = '"' (char | escape_sequence)* '"'
bool_literal = 'true' | 'false'
```

### Syntax Grammar (Simplified)

```
program = import* item*

import = 'import' module_path ';'
module_path = identifier ('::' identifier)*

item = function | struct_def | enum_def

function = visibility? 'fn' identifier generic_params? '(' params? ')' return_type? block
visibility = 'pub'
generic_params = '<' identifier (',' identifier)* '>'
params = param (',' param)*
param = identifier ':' type
return_type = '->' type

struct_def = visibility? 'struct' identifier '{' field_list? '}'
field_list = field (',' field)* ','?
field = identifier ':' type

enum_def = visibility? 'enum' identifier '{' variant_list '}'
variant_list = variant (',' variant)* ','?
variant = identifier variant_data?
variant_data = '(' type_list ')' | '{' field_list '}'

type = primitive_type | array_type | custom_type | generic_type
primitive_type = 'i32' | 'i64' | 'u32' | 'u64' | 'bool' | 'String' | '()'
array_type = '[' type ';' integer_literal ']'
custom_type = identifier
generic_type = identifier '<' type_list '>'

statement = let_stmt | expr_stmt | if_stmt | while_stmt | for_stmt | 
            match_stmt | return_stmt | break_stmt | continue_stmt | block

let_stmt = 'let' 'mut'? identifier ':' type? '=' expr ';'
expr_stmt = expr ';'
if_stmt = 'if' expr block ('else' if_stmt | 'else' block)?
while_stmt = 'while' expr block
for_stmt = 'for' identifier 'in' expr block
match_stmt = 'match' expr '{' match_arms '}'
return_stmt = 'return' expr? ';'
break_stmt = 'break' ';'
continue_stmt = 'continue' ';'
block = '{' statement* '}'

expr = primary | binary_expr | unary_expr | call_expr | index_expr | 
       field_expr | struct_literal | enum_constructor | range_expr

primary = literal | identifier | '(' expr ')' | array_literal
binary_expr = expr binary_op expr
unary_expr = unary_op expr
call_expr = expr '(' expr_list? ')'
index_expr = expr '[' expr ']'
field_expr = expr '.' identifier
struct_literal = identifier '{' field_init_list '}'
enum_constructor = identifier '::' identifier constructor_data?
range_expr = expr '..' expr

binary_op = '+' | '-' | '*' | '/' | '%' | '==' | '!=' | '<' | '>' | 
            '<=' | '>=' | '&&' | '||'
unary_op = '-' | '!'
```

## Self-Hosting

As of June 18, 2025, Palladium is self-hosting. A compiler written in Palladium can compile other Palladium programs, including itself!