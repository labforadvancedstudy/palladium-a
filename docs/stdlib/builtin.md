# Built-in Functions Reference

This page provides detailed documentation for all built-in functions in Palladium. These functions are available globally without any imports.

## I/O Functions

### print
```palladium
fn print(s: String)
```
Prints a string to standard output followed by a newline.

**Parameters:**
- `s`: The string to print

**Example:**
```palladium
print("Hello, World!");
print("The answer is: " + int_to_string(42));
```

### print_int
```palladium
fn print_int(n: i64)
```
Prints an integer to standard output followed by a newline.

**Parameters:**
- `n`: The integer to print

**Example:**
```palladium
print_int(42);
let sum = 10 + 20;
print_int(sum);
```

## String Operations

### string_len
```palladium
fn string_len(s: String) -> i64
```
Returns the length of a string in bytes.

**Parameters:**
- `s`: The string to measure

**Returns:**
- The number of bytes in the string

**Example:**
```palladium
let len = string_len("hello");  // 5
let empty_len = string_len("");  // 0
```

### string_concat
```palladium
fn string_concat(s1: String, s2: String) -> String
```
Concatenates two strings together.

**Parameters:**
- `s1`: The first string
- `s2`: The second string

**Returns:**
- A new string containing s1 followed by s2

**Note:** The `+` operator is syntactic sugar for this function.

**Example:**
```palladium
let greeting = string_concat("Hello, ", "World!");
// Equivalent to:
let greeting = "Hello, " + "World!";
```

### string_eq
```palladium
fn string_eq(s1: String, s2: String) -> bool
```
Compares two strings for equality.

**Parameters:**
- `s1`: The first string
- `s2`: The second string

**Returns:**
- `true` if the strings are identical, `false` otherwise

**Example:**
```palladium
if string_eq(username, "admin") {
    print("Welcome, administrator!");
}

let same = string_eq("hello", "hello");  // true
let different = string_eq("hello", "world");  // false
```

### string_char_at
```palladium
fn string_char_at(s: String, index: i64) -> i64
```
Returns the ASCII value of the character at the specified index.

**Parameters:**
- `s`: The string to index into
- `index`: The zero-based index of the character

**Returns:**
- The ASCII value of the character at the index, or -1 if index is out of bounds

**Example:**
```palladium
let ch = string_char_at("hello", 0);  // 104 ('h')
let ch2 = string_char_at("hello", 4); // 111 ('o')
let invalid = string_char_at("hello", 10); // -1 (out of bounds)
```

### string_substring
```palladium
fn string_substring(s: String, start: i64, end: i64) -> String
```
Extracts a substring from a string.

**Parameters:**
- `s`: The source string
- `start`: The starting index (inclusive)
- `end`: The ending index (exclusive)

**Returns:**
- A new string containing characters from start to end-1

**Example:**
```palladium
let sub = string_substring("hello world", 0, 5);   // "hello"
let sub2 = string_substring("hello world", 6, 11); // "world"
let empty = string_substring("hello", 2, 2);       // ""
```

### string_from_char
```palladium
fn string_from_char(c: i64) -> String
```
Creates a single-character string from an ASCII value.

**Parameters:**
- `c`: The ASCII value of the character

**Returns:**
- A string containing the single character

**Example:**
```palladium
let a = string_from_char(65);    // "A"
let newline = string_from_char(10); // "\n"
let space = string_from_char(32);  // " "
```

### string_to_int
```palladium
fn string_to_int(s: String) -> i64
```
Parses a string as a signed 64-bit integer.

**Parameters:**
- `s`: The string to parse

**Returns:**
- The parsed integer value, or 0 if parsing fails

**Note:** For safe parsing with error handling, use `parse_int_safe` from stdlib::option.

**Example:**
```palladium
let n = string_to_int("123");    // 123
let neg = string_to_int("-456"); // -456
let invalid = string_to_int("abc"); // 0
```

### int_to_string
```palladium
fn int_to_string(n: i64) -> String
```
Converts an integer to its string representation.

**Parameters:**
- `n`: The integer to convert

**Returns:**
- A string representation of the integer

**Example:**
```palladium
let s = int_to_string(42);      // "42"
let neg = int_to_string(-123);  // "-123"
let zero = int_to_string(0);    // "0"
```

## Character Classification

### char_is_digit
```palladium
fn char_is_digit(c: i64) -> bool
```
Checks if a character is a decimal digit (0-9).

**Parameters:**
- `c`: The ASCII value of the character to check

**Returns:**
- `true` if the character is a digit, `false` otherwise

**Example:**
```palladium
let is_digit = char_is_digit(48);  // true ('0')
let not_digit = char_is_digit(65); // false ('A')

// Check if string starts with digit
let first_char = string_char_at("123abc", 0);
if char_is_digit(first_char) {
    print("Starts with a digit");
}
```

### char_is_alpha
```palladium
fn char_is_alpha(c: i64) -> bool
```
Checks if a character is alphabetic (a-z, A-Z).

**Parameters:**
- `c`: The ASCII value of the character to check

**Returns:**
- `true` if the character is a letter, `false` otherwise

**Example:**
```palladium
let is_letter = char_is_alpha(65);   // true ('A')
let is_lower = char_is_alpha(97);    // true ('a')
let not_letter = char_is_alpha(48);  // false ('0')
```

### char_is_whitespace
```palladium
fn char_is_whitespace(c: i64) -> bool
```
Checks if a character is whitespace (space, tab, newline, etc).

**Parameters:**
- `c`: The ASCII value of the character to check

**Returns:**
- `true` if the character is whitespace, `false` otherwise

**Example:**
```palladium
let is_space = char_is_whitespace(32);    // true (' ')
let is_tab = char_is_whitespace(9);       // true ('\t')
let is_newline = char_is_whitespace(10);  // true ('\n')
let not_ws = char_is_whitespace(65);      // false ('A')
```

## File I/O

### file_open
```palladium
fn file_open(path: String) -> i64
```
Opens a file for reading and writing.

**Parameters:**
- `path`: The path to the file

**Returns:**
- A file handle (positive integer) on success, or -1 on error

**Note:** Always check the return value and close files when done.

**Example:**
```palladium
let handle = file_open("data.txt");
if handle < 0 {
    print("Failed to open file");
    return;
}
// Use the file...
file_close(handle);
```

### file_read_all
```palladium
fn file_read_all(handle: i64) -> String
```
Reads the entire contents of a file into a string.

**Parameters:**
- `handle`: The file handle from file_open

**Returns:**
- The complete file contents as a string, or empty string on error

**Example:**
```palladium
let handle = file_open("config.txt");
if handle >= 0 {
    let content = file_read_all(handle);
    print("File contents: " + content);
    file_close(handle);
}
```

### file_read_line
```palladium
fn file_read_line(handle: i64) -> String
```
Reads the next line from a file, excluding the newline character.

**Parameters:**
- `handle`: The file handle from file_open

**Returns:**
- The next line as a string, or empty string at EOF or on error

**Example:**
```palladium
let handle = file_open("lines.txt");
if handle >= 0 {
    let line = file_read_line(handle);
    while string_len(line) > 0 {
        print("Line: " + line);
        line = file_read_line(handle);
    }
    file_close(handle);
}
```

### file_write
```palladium
fn file_write(handle: i64, content: String) -> bool
```
Writes a string to a file.

**Parameters:**
- `handle`: The file handle from file_open
- `content`: The string to write

**Returns:**
- `true` on success, `false` on error

**Example:**
```palladium
let handle = file_open("output.txt");
if handle >= 0 {
    if file_write(handle, "Hello, file!\n") {
        print("Write successful");
    }
    file_close(handle);
}
```

### file_close
```palladium
fn file_close(handle: i64) -> bool
```
Closes an open file handle.

**Parameters:**
- `handle`: The file handle to close

**Returns:**
- `true` on success, `false` on error

**Note:** Always close files to free resources.

**Example:**
```palladium
let handle = file_open("data.txt");
if handle >= 0 {
    // Do file operations...
    if !file_close(handle) {
        print("Warning: Failed to close file");
    }
}
```

### file_exists
```palladium
fn file_exists(path: String) -> bool
```
Checks if a file exists at the given path.

**Parameters:**
- `path`: The path to check

**Returns:**
- `true` if the file exists, `false` otherwise

**Example:**
```palladium
if file_exists("config.txt") {
    print("Config file found");
} else {
    print("Config file missing, using defaults");
}
```

## Common Patterns

### Safe File Reading
```palladium
fn read_file_safely(path: String) -> String {
    if !file_exists(path) {
        return "";
    }
    
    let handle = file_open(path);
    if handle < 0 {
        return "";
    }
    
    let content = file_read_all(handle);
    file_close(handle);
    return content;
}
```

### String Tokenization
```palladium
fn count_words(text: String) -> i64 {
    let count = 0;
    let in_word = false;
    let i = 0;
    let len = string_len(text);
    
    while i < len {
        let ch = string_char_at(text, i);
        if char_is_whitespace(ch) {
            in_word = false;
        } else {
            if !in_word {
                count = count + 1;
                in_word = true;
            }
        }
        i = i + 1;
    }
    
    return count;
}
```

### Number Validation
```palladium
fn is_valid_number(s: String) -> bool {
    let len = string_len(s);
    if len == 0 {
        return false;
    }
    
    let i = 0;
    
    // Check for optional minus sign
    if string_char_at(s, 0) == 45 { // '-'
        i = 1;
        if len == 1 {
            return false; // Just a minus sign
        }
    }
    
    // Check all remaining characters are digits
    while i < len {
        if !char_is_digit(string_char_at(s, i)) {
            return false;
        }
        i = i + 1;
    }
    
    return true;
}
```