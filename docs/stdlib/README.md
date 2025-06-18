# Palladium Standard Library Documentation

Welcome to the Palladium Standard Library documentation. This guide covers all built-in functions and standard library modules available in Palladium.

## Table of Contents

1. [Built-in Functions](#built-in-functions)
2. [Standard Modules](#standard-modules)
3. [Extended Libraries](#extended-libraries)
4. [Usage Examples](#usage-examples)

## Built-in Functions

These functions are available globally without any imports:

### I/O Functions

#### `print(s: String)`
Prints a string to stdout with a newline.
```palladium
print("Hello, World!");
```

#### `print_int(n: i64)`
Prints an integer to stdout with a newline.
```palladium
print_int(42);
```

### String Operations

#### `string_len(s: String) -> i64`
Returns the length of a string in bytes.
```palladium
let len = string_len("hello");  // 5
```

#### `string_concat(s1: String, s2: String) -> String`
Concatenates two strings. Also available via the `+` operator.
```palladium
let greeting = string_concat("Hello, ", "World!");
// Or using the + operator:
let greeting = "Hello, " + "World!";
```

#### `string_eq(s1: String, s2: String) -> bool`
Compares two strings for equality.
```palladium
if string_eq(name, "Alice") {
    print("Hello Alice!");
}
```

#### `string_char_at(s: String, index: i64) -> i64`
Returns the character (as ASCII value) at the given index. Returns -1 if index is out of bounds.
```palladium
let ch = string_char_at("hello", 0);  // 104 ('h')
```

#### `string_substring(s: String, start: i64, end: i64) -> String`
Extracts a substring from start (inclusive) to end (exclusive).
```palladium
let sub = string_substring("hello world", 0, 5);  // "hello"
```

#### `string_from_char(c: i64) -> String`
Creates a single-character string from an ASCII value.
```palladium
let a = string_from_char(65);  // "A"
```

#### `string_to_int(s: String) -> i64`
Parses a string as an integer. Returns 0 for invalid input.
```palladium
let n = string_to_int("123");  // 123
```

#### `int_to_string(n: i64) -> String`
Converts an integer to its string representation.
```palladium
let s = int_to_string(42);  // "42"
```

### Character Classification

#### `char_is_digit(c: i64) -> bool`
Returns true if the character is a digit (0-9).
```palladium
if char_is_digit(string_char_at("123", 0)) {
    print("First character is a digit");
}
```

#### `char_is_alpha(c: i64) -> bool`
Returns true if the character is alphabetic (a-z, A-Z).
```palladium
if char_is_alpha(ch) {
    print("Character is a letter");
}
```

#### `char_is_whitespace(c: i64) -> bool`
Returns true if the character is whitespace (space, tab, newline, etc).
```palladium
while char_is_whitespace(string_char_at(input, i)) {
    i = i + 1;  // Skip whitespace
}
```

### File I/O

#### `file_open(path: String) -> i64`
Opens a file and returns a handle. Returns -1 on error.
```palladium
let handle = file_open("input.txt");
if handle < 0 {
    print("Failed to open file");
    return;
}
```

#### `file_read_all(handle: i64) -> String`
Reads the entire contents of a file.
```palladium
let content = file_read_all(handle);
```

#### `file_read_line(handle: i64) -> String`
Reads the next line from a file (excluding the newline character).
```palladium
let line = file_read_line(handle);
while string_len(line) > 0 {
    print(line);
    line = file_read_line(handle);
}
```

#### `file_write(handle: i64, content: String) -> bool`
Writes content to a file. Returns true on success.
```palladium
if !file_write(handle, "Hello, file!") {
    print("Write failed");
}
```

#### `file_close(handle: i64) -> bool`
Closes a file handle. Returns true on success.
```palladium
file_close(handle);
```

#### `file_exists(path: String) -> bool`
Checks if a file exists.
```palladium
if file_exists("config.txt") {
    print("Config file found");
}
```

## Standard Modules

### std::math

Mathematical functions for common operations.

```palladium
import std::math;
```

#### Functions

- **`pd_abs(x: i64) -> i64`** - Returns the absolute value of x
- **`max(a: i64, b: i64) -> i64`** - Returns the larger of two values
- **`min(a: i64, b: i64) -> i64`** - Returns the smaller of two values
- **`pd_pow(base: i64, exp: i64) -> i64`** - Returns base raised to the power of exp

Example:
```palladium
import std::math;

let distance = pd_abs(x2 - x1);
let largest = max(10, 20);  // 20
let result = pd_pow(2, 8);   // 256
```

### std::string

Extended string manipulation utilities.

```palladium
import std::string;
```

#### Functions

- **`trim(s: String) -> String`** - Removes leading and trailing whitespace
- **`trim_start(s: String) -> String`** - Removes leading whitespace
- **`trim_end(s: String) -> String`** - Removes trailing whitespace
- **`starts_with(s: String, prefix: String) -> bool`** - Checks if string starts with prefix
- **`ends_with(s: String, suffix: String) -> bool`** - Checks if string ends with suffix
- **`contains(s: String, needle: String) -> bool`** - Checks if string contains substring

Example:
```palladium
import std::string;

let cleaned = trim("  hello  ");  // "hello"
if starts_with(filename, "test_") {
    print("This is a test file");
}
```

## Extended Libraries

### stdlib::string_utils

Advanced string manipulation functions.

```palladium
import stdlib::string_utils;
```

#### Key Functions

- **`string_split_first(s: String, delimiter: i64) -> String`** - Returns substring before first delimiter
- **`string_split_rest(s: String, delimiter: i64) -> String`** - Returns substring after first delimiter
- **`string_replace_first(s: String, old: String, new: String) -> String`** - Replaces first occurrence
- **`string_to_upper(s: String) -> String`** - Converts to uppercase (ASCII only)
- **`string_to_lower(s: String) -> String`** - Converts to lowercase (ASCII only)
- **`string_reverse(s: String) -> String`** - Reverses the string
- **`string_pad_left(s: String, width: i64, pad_char: i64) -> String`** - Pads string on the left
- **`string_pad_right(s: String, width: i64, pad_char: i64) -> String`** - Pads string on the right

### stdlib::option

Option type for handling nullable values (type-specific implementations).

```palladium
import stdlib::option;
```

#### Types
- `OptionString` - Optional string value
- `OptionInt` - Optional integer value
- `OptionBool` - Optional boolean value

#### Common Functions (using OptionInt as example)
- **`option_int_is_some(opt: OptionInt) -> bool`** - Checks if option contains a value
- **`option_int_is_none(opt: OptionInt) -> bool`** - Checks if option is empty
- **`option_int_unwrap(opt: OptionInt) -> i64`** - Gets the value (panics if None)
- **`option_int_unwrap_or(opt: OptionInt, default: i64) -> i64`** - Gets value or default

#### Utility Functions
- **`parse_int_safe(s: String) -> OptionInt`** - Safe integer parsing
- **`string_find_char(s: String, target: i64) -> OptionInt`** - Find character position
- **`string_find(haystack: String, needle: String) -> OptionInt`** - Find substring position

Example:
```palladium
import stdlib::option;

let result = parse_int_safe("123");
match result {
    OptionInt::Some(n) => print_int(n),
    OptionInt::None => print("Invalid number")
}
```

### stdlib::result

Result type for error handling (type-specific implementations).

```palladium
import stdlib::result;
```

#### Types
- `StringResult` - Result containing string or error
- `IntResult` - Result containing integer or error
- `FileResult` - Result for file operations

#### Common Functions
- **`string_result_is_ok(r: StringResult) -> bool`** - Checks if result is successful
- **`string_result_unwrap(r: StringResult) -> String`** - Gets value (panics on error)
- **`string_result_unwrap_or(r: StringResult, default: String) -> String`** - Gets value or default

Example:
```palladium
import stdlib::result;

let file_result = safe_file_read("data.txt");
match file_result {
    StringResult::Ok(content) => print(content),
    StringResult::Err(error) => print("Error: " + error)
}
```

### stdlib::vec_simple

Dynamic array implementation for integers.

```palladium
import stdlib::vec_simple;
```

#### Core Functions
- **`vec_int_new() -> VecInt`** - Creates new empty vector
- **`vec_int_push(vec: mut VecInt, value: i64) -> VecInt`** - Adds element to end
- **`vec_int_get(vec: VecInt, index: i64) -> i64`** - Gets element at index
- **`vec_int_len(vec: VecInt) -> i64`** - Returns length
- **`vec_int_pop(vec: mut VecInt) -> (VecInt, i64)`** - Removes and returns last element

#### Utility Functions
- **`vec_int_sum(vec: VecInt) -> i64`** - Sum of all elements
- **`vec_int_min(vec: VecInt) -> i64`** - Minimum element
- **`vec_int_max(vec: VecInt) -> i64`** - Maximum element
- **`vec_int_sort(vec: mut VecInt) -> VecInt`** - Sorts vector in place
- **`vec_int_reverse(vec: mut VecInt) -> VecInt`** - Reverses vector in place

Example:
```palladium
import stdlib::vec_simple;

let mut vec = vec_int_new();
vec = vec_int_push(vec, 10);
vec = vec_int_push(vec, 20);
vec = vec_int_push(vec, 30);

let sum = vec_int_sum(vec);  // 60
vec = vec_int_sort(vec);
```

### stdlib::string_builder

Efficient string concatenation for building large strings.

```palladium
import stdlib::string_builder;
```

#### Functions
- **`sb_new() -> StringBuilder`** - Creates new string builder
- **`sb_append(mut sb: StringBuilder, s: String)`** - Appends string
- **`sb_append_char(mut sb: StringBuilder, ch: i64)`** - Appends character
- **`sb_append_int(mut sb: StringBuilder, n: i64)`** - Appends integer
- **`sb_to_string(sb: StringBuilder) -> String`** - Converts to final string

Example:
```palladium
import stdlib::string_builder;

let mut sb = sb_new();
sb_append(sb, "Total: ");
sb_append_int(sb, 42);
sb_append_char(sb, 10);  // newline
let result = sb_to_string(sb);
```

### stdlib::hashmap_simple

Simple hash map implementation for string keys and integer values.

```palladium
import stdlib::hashmap_simple;
```

#### Functions
- **`hashmap_new() -> HashMap`** - Creates new empty hashmap
- **`hashmap_insert(map: HashMap, key: String, value: i64) -> HashMap`** - Inserts or updates
- **`hashmap_get(map: HashMap, key: String) -> i64`** - Gets value (-1 if not found)
- **`hashmap_contains(map: HashMap, key: String) -> bool`** - Checks if key exists
- **`hashmap_remove(map: HashMap, key: String) -> HashMap`** - Removes key
- **`hashmap_size(map: HashMap) -> i64`** - Returns number of entries

Example:
```palladium
import stdlib::hashmap_simple;

let mut map = hashmap_new();
map = hashmap_insert(map, "age", 25);
map = hashmap_insert(map, "score", 100);

if hashmap_contains(map, "age") {
    let age = hashmap_get(map, "age");
    print_int(age);
}
```

## Usage Examples

### Reading and Processing a File

```palladium
import std::string;
import stdlib::string_utils;

fn process_config_file(filename: String) {
    if !file_exists(filename) {
        print("Config file not found");
        return;
    }
    
    let handle = file_open(filename);
    if handle < 0 {
        print("Failed to open config file");
        return;
    }
    
    let line = file_read_line(handle);
    while string_len(line) > 0 {
        let trimmed = trim(line);
        
        // Skip comments and empty lines
        if string_len(trimmed) > 0 && !starts_with(trimmed, "#") {
            // Parse key=value pairs
            let key = string_split_first(trimmed, 61);  // 61 = '='
            let value = string_split_rest(trimmed, 61);
            
            print("Config: " + key + " = " + value);
        }
        
        line = file_read_line(handle);
    }
    
    file_close(handle);
}
```

### Building a Report

```palladium
import stdlib::string_builder;
import stdlib::vec_simple;

fn generate_report(data: VecInt) -> String {
    let mut sb = sb_new();
    
    sb_append(sb, "Data Analysis Report\n");
    sb_append(sb, "===================\n\n");
    
    sb_append(sb, "Count: ");
    sb_append_int(sb, vec_int_len(data));
    sb_append_newline(sb);
    
    sb_append(sb, "Sum: ");
    sb_append_int(sb, vec_int_sum(data));
    sb_append_newline(sb);
    
    sb_append(sb, "Min: ");
    sb_append_int(sb, vec_int_min(data));
    sb_append_newline(sb);
    
    sb_append(sb, "Max: ");
    sb_append_int(sb, vec_int_max(data));
    sb_append_newline(sb);
    
    return sb_to_string(sb);
}
```

### Safe Number Parsing

```palladium
import stdlib::option;

fn read_number_from_user() -> i64 {
    print("Enter a number: ");
    let input = "42";  // Simulated input
    
    let result = parse_int_safe(input);
    match result {
        OptionInt::Some(n) => {
            print("You entered: " + int_to_string(n));
            return n;
        }
        OptionInt::None => {
            print("Invalid number, using default");
            return 0;
        }
    }
}
```

## Best Practices

1. **Error Handling**: Always check return values from file operations
2. **String Building**: Use StringBuilder for concatenating many strings
3. **Memory**: Be aware that string operations create new strings (immutable)
4. **Options**: Use Option types for functions that might not return a value
5. **Results**: Use Result types for operations that can fail with errors

## Future Enhancements

The standard library is continuously evolving. Planned additions include:
- Generic versions of collections (Vec<T>, HashMap<K,V>)
- More comprehensive file I/O (directories, permissions)
- Network operations
- Date/time handling
- Regular expressions
- JSON parsing
- Concurrent programming primitives