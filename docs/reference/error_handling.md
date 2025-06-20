# Error Handling Library Reference

This page documents the Option and Result types for safe error handling in Palladium.

## Option Types

Option types represent values that may or may not be present. They are useful for functions that might not return a value.

### Import
```palladium
import stdlib::option;
```

### Option Types Available

Due to current language limitations (no generics), we have type-specific Option implementations:

- `OptionString` - Optional string value
- `OptionInt` - Optional integer value  
- `OptionBool` - Optional boolean value

### OptionInt

#### Type Definition
```palladium
enum OptionInt {
    Some(i64),
    None
}
```

#### Core Functions

##### option_int_is_some
```palladium
fn option_int_is_some(opt: OptionInt) -> bool
```
Checks if the option contains a value.

**Example:**
```palladium
let opt = OptionInt::Some(42);
if option_int_is_some(opt) {
    print("Has a value");
}
```

##### option_int_is_none
```palladium
fn option_int_is_none(opt: OptionInt) -> bool
```
Checks if the option is empty.

**Example:**
```palladium
let opt = OptionInt::None;
if option_int_is_none(opt) {
    print("No value present");
}
```

##### option_int_unwrap
```palladium
fn option_int_unwrap(opt: OptionInt) -> i64
```
Extracts the value from Some. **Panics if None!**

**Example:**
```palladium
let opt = OptionInt::Some(42);
let value = option_int_unwrap(opt);  // 42
```

##### option_int_unwrap_or
```palladium
fn option_int_unwrap_or(opt: OptionInt, default: i64) -> i64
```
Extracts the value from Some, or returns a default if None.

**Example:**
```palladium
let opt1 = OptionInt::Some(42);
let opt2 = OptionInt::None;
let val1 = option_int_unwrap_or(opt1, 0);  // 42
let val2 = option_int_unwrap_or(opt2, 0);  // 0
```

### OptionString

#### Type Definition
```palladium
enum OptionString {
    Some(String),
    None
}
```

#### Core Functions

##### option_string_is_some / option_string_is_none
Same behavior as OptionInt versions but for strings.

##### option_string_unwrap
```palladium
fn option_string_unwrap(opt: OptionString) -> String
```
Extracts the string value. **Panics if None!**

##### option_string_unwrap_or
```palladium
fn option_string_unwrap_or(opt: OptionString, default: String) -> String
```
Extracts the string or returns a default.

##### option_string_map_or
```palladium
fn option_string_map_or(opt: OptionString, default: String, f: fn(String) -> String) -> String
```
Applies a function to the value if Some, otherwise returns default.

**Example:**
```palladium
fn to_upper(s: String) -> String {
    return string_to_upper(s);
}

let opt = OptionString::Some("hello");
let result = option_string_map_or(opt, "DEFAULT", to_upper);  // "HELLO"
```

### OptionBool

Similar to the other Option types but for boolean values.

### Utility Functions Using Options

#### parse_int_safe
```palladium
fn parse_int_safe(s: String) -> OptionInt
```
Safely parses a string to an integer.

**Example:**
```palladium
let input = "123";
match parse_int_safe(input) {
    OptionInt::Some(n) => print("Parsed: " + int_to_string(n)),
    OptionInt::None => print("Invalid number")
}
```

#### string_find_char
```palladium
fn string_find_char(s: String, target: i64) -> OptionInt
```
Finds the first occurrence of a character.

**Example:**
```palladium
let pos = string_find_char("hello", 108);  // 'l'
match pos {
    OptionInt::Some(i) => print("Found at index: " + int_to_string(i)),
    OptionInt::None => print("Character not found")
}
```

#### string_find
```palladium
fn string_find(haystack: String, needle: String) -> OptionInt
```
Finds the first occurrence of a substring.

**Example:**
```palladium
let pos = string_find("hello world", "world");
if option_int_is_some(pos) {
    let index = option_int_unwrap(pos);
    print("Found 'world' at position: " + int_to_string(index));
}
```

#### string_first_line
```palladium
fn string_first_line(s: String) -> OptionString
```
Extracts the first line from a string.

**Example:**
```palladium
let text = "Line 1\nLine 2\nLine 3";
match string_first_line(text) {
    OptionString::Some(line) => print("First line: " + line),
    OptionString::None => print("Empty string")
}
```

## Result Types

Result types represent operations that can succeed with a value or fail with an error.

### Import
```palladium
import stdlib::result;
```

### Result Types Available

- `StringResult` - Result containing string or error
- `IntResult` - Result containing integer or error
- `FileResult` - Result for file operations
- `CompileResult` - Example for compiler operations

### IntResult

#### Type Definition
```palladium
enum IntResult {
    Ok(i64),
    Err(String)
}
```

#### Core Functions

##### int_result_is_ok
```palladium
fn int_result_is_ok(r: IntResult) -> bool
```
Checks if the result is successful.

**Example:**
```palladium
let result = parse_int("123");
if int_result_is_ok(result) {
    print("Parsing succeeded");
}
```

##### int_result_unwrap
```palladium
fn int_result_unwrap(r: IntResult) -> i64
```
Extracts the value from Ok. **Panics if Err!**

##### int_result_unwrap_or
```palladium
fn int_result_unwrap_or(r: IntResult, default: i64) -> i64
```
Extracts the value from Ok, or returns a default if Err.

### StringResult

#### Type Definition
```palladium
enum StringResult {
    Ok(String),
    Err(String)
}
```

#### Core Functions

##### string_result_is_ok
```palladium
fn string_result_is_ok(r: StringResult) -> bool
```
Checks if the result is successful.

##### string_result_unwrap
```palladium
fn string_result_unwrap(r: StringResult) -> String
```
Extracts the value from Ok. **Panics if Err!**

##### string_result_expect
```palladium
fn string_result_expect(r: StringResult, msg: String) -> String
```
Extracts the value from Ok with a custom panic message if Err.

**Example:**
```palladium
let result = safe_file_read("config.txt");
let content = string_result_expect(result, "Config file must exist");
```

### Utility Functions Using Results

#### parse_int
```palladium
fn parse_int(s: String) -> IntResult
```
Parses a string to integer with detailed error messages.

**Example:**
```palladium
match parse_int("abc") {
    IntResult::Ok(n) => print("Number: " + int_to_string(n)),
    IntResult::Err(msg) => print("Parse error: " + msg)
}
```

#### safe_file_open
```palladium
fn safe_file_open(path: String) -> FileResult
```
Opens a file with error handling.

**Example:**
```palladium
match safe_file_open("data.txt") {
    FileResult::Ok(handle) => {
        // Use file handle
        file_close(handle);
    }
    FileResult::Err(msg) => print("Cannot open file: " + msg)
}
```

#### safe_file_read
```palladium
fn safe_file_read(path: String) -> StringResult
```
Reads entire file contents with error handling.

**Example:**
```palladium
match safe_file_read("input.txt") {
    StringResult::Ok(content) => process_content(content),
    StringResult::Err(error) => print("Read error: " + error)
}
```

#### read_and_parse_number
```palladium
fn read_and_parse_number(path: String) -> IntResult
```
Reads a file and parses its content as a number, propagating errors.

**Example:**
```palladium
match read_and_parse_number("count.txt") {
    IntResult::Ok(count) => print("Count: " + int_to_string(count)),
    IntResult::Err(error) => print("Error: " + error)
}
```

## Complete Examples

### Safe Configuration Parser

```palladium
import stdlib::option;
import stdlib::result;

struct Config {
    width: i64;
    height: i64;
    fullscreen: bool;
}

fn parse_config_line(line: String) -> OptionString {
    // Skip empty lines and comments
    let trimmed = string_trim(line);
    if string_len(trimmed) == 0 || string_char_at(trimmed, 0) == 35 { // '#'
        return OptionString::None;
    }
    
    return OptionString::Some(trimmed);
}

fn parse_config_value(line: String) -> StringResult {
    let eq_pos = string_find_char(line, 61); // '='
    match eq_pos {
        OptionInt::Some(pos) => {
            let key = string_substring(line, 0, pos);
            let value = string_substring(line, pos + 1, string_len(line));
            return StringResult::Ok(string_trim(value));
        }
        OptionInt::None => {
            return StringResult::Err("Missing '=' in config line");
        }
    }
}

fn load_config(path: String) -> Result<Config> {
    match safe_file_read(path) {
        StringResult::Ok(content) => {
            // Parse config content
            let config = Config { 
                width: 800, 
                height: 600, 
                fullscreen: false 
            };
            // ... parsing logic ...
            return Result::Ok(config);
        }
        StringResult::Err(error) => {
            return Result::Err("Failed to load config: " + error);
        }
    }
}
```

### Robust Number Input

```palladium
import stdlib::option;

fn get_number_from_user(prompt: String, min: i64, max: i64) -> i64 {
    let attempts = 0;
    
    while attempts < 3 {
        print(prompt);
        let input = read_line();  // Hypothetical function
        
        match parse_int_safe(input) {
            OptionInt::Some(n) => {
                if n >= min && n <= max {
                    return n;
                } else {
                    print("Number must be between " + 
                          int_to_string(min) + " and " + 
                          int_to_string(max));
                }
            }
            OptionInt::None => {
                print("Invalid number, please try again");
            }
        }
        
        attempts = attempts + 1;
    }
    
    print("Too many invalid attempts, using default");
    return min;
}
```

### Chain of Fallible Operations

```palladium
import stdlib::result;

fn process_data_file(input_path: String, output_path: String) -> StringResult {
    // Read input file
    match safe_file_read(input_path) {
        StringResult::Ok(content) => {
            // Process content
            let processed = transform_data(content);
            
            // Write output file
            match safe_file_write(output_path, processed) {
                StringResult::Ok(_) => {
                    return StringResult::Ok("Processing complete");
                }
                StringResult::Err(write_err) => {
                    return StringResult::Err("Write failed: " + write_err);
                }
            }
        }
        StringResult::Err(read_err) => {
            return StringResult::Err("Read failed: " + read_err);
        }
    }
}

fn main() {
    match process_data_file("input.dat", "output.dat") {
        StringResult::Ok(msg) => print("Success: " + msg),
        StringResult::Err(error) => print("Failed: " + error)
    }
}
```

## Best Practices

1. **Use Option for nullable values**: When a function might not return a value (e.g., search operations)

2. **Use Result for fallible operations**: When a function can fail with an error message (e.g., I/O, parsing)

3. **Always handle both cases**: Use pattern matching to handle both Some/None or Ok/Err

4. **Avoid unwrap in production**: Use `unwrap_or` or pattern matching instead

5. **Propagate errors**: When chaining operations, propagate errors up the call stack

6. **Provide context**: When creating Err values, include helpful error messages

## Limitations

Current limitations due to missing language features:
- No generic Option<T> or Result<T, E>
- Type-specific implementations only
- No ? operator for error propagation
- No custom error types

Future improvements planned:
- Generic Option and Result types
- Error propagation operator (?)
- Custom error types with rich information
- Try blocks for error handling