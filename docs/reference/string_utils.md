# String Utilities Reference

This page documents the string manipulation utilities available in the Palladium standard library.

## Basic String Module (std::string)

Basic string operations for common tasks.

### Import
```palladium
import std::string;
```

### Trimming Functions

#### trim
```palladium
fn trim(s: String) -> String
```
Removes leading and trailing whitespace from a string.

**Example:**
```palladium
let cleaned = trim("  hello world  ");  // "hello world"
let no_change = trim("already trimmed"); // "already trimmed"
```

#### trim_start
```palladium
fn trim_start(s: String) -> String
```
Removes leading whitespace only.

**Example:**
```palladium
let result = trim_start("  hello world  ");  // "hello world  "
```

#### trim_end
```palladium
fn trim_end(s: String) -> String
```
Removes trailing whitespace only.

**Example:**
```palladium
let result = trim_end("  hello world  ");  // "  hello world"
```

### String Matching Functions

#### starts_with
```palladium
fn starts_with(s: String, prefix: String) -> bool
```
Checks if a string starts with the given prefix.

**Example:**
```palladium
if starts_with(filename, "test_") {
    print("This is a test file");
}

let has_prefix = starts_with("hello world", "hello");  // true
```

#### ends_with
```palladium
fn ends_with(s: String, suffix: String) -> bool
```
Checks if a string ends with the given suffix.

**Example:**
```palladium
if ends_with(filename, ".pd") {
    print("This is a Palladium source file");
}

let has_suffix = ends_with("hello.txt", ".txt");  // true
```

#### contains
```palladium
fn contains(s: String, needle: String) -> bool
```
Checks if a string contains a substring.

**Example:**
```palladium
if contains(error_msg, "permission denied") {
    print("Access error detected");
}

let found = contains("hello world", "lo wo");  // true
```

## Extended String Utilities (stdlib::string_utils)

Advanced string manipulation functions.

### Import
```palladium
import stdlib::string_utils;
```

### String Splitting

#### string_split_first
```palladium
fn string_split_first(s: String, delimiter: i64) -> String
```
Returns the substring before the first occurrence of the delimiter character.

**Parameters:**
- `s`: The string to split
- `delimiter`: ASCII value of the delimiter character

**Example:**
```palladium
let first = string_split_first("hello,world", 44);  // "hello" (44 = ',')
let key = string_split_first("name=John", 61);      // "name" (61 = '=')
```

#### string_split_rest
```palladium
fn string_split_rest(s: String, delimiter: i64) -> String
```
Returns the substring after the first occurrence of the delimiter character.

**Example:**
```palladium
let rest = string_split_rest("hello,world", 44);   // "world"
let value = string_split_rest("name=John", 61);    // "John"
```

### Case Conversion

#### string_to_upper
```palladium
fn string_to_upper(s: String) -> String
```
Converts a string to uppercase (ASCII only).

**Example:**
```palladium
let upper = string_to_upper("Hello World!");  // "HELLO WORLD!"
```

#### string_to_lower
```palladium
fn string_to_lower(s: String) -> String
```
Converts a string to lowercase (ASCII only).

**Example:**
```palladium
let lower = string_to_lower("Hello World!");  // "hello world!"
```

### String Transformation

#### string_replace_first
```palladium
fn string_replace_first(s: String, old: String, new: String) -> String
```
Replaces the first occurrence of a substring.

**Example:**
```palladium
let result = string_replace_first("hello hello", "hello", "hi");  // "hi hello"
```

#### string_reverse
```palladium
fn string_reverse(s: String) -> String
```
Reverses a string.

**Example:**
```palladium
let reversed = string_reverse("hello");  // "olleh"
let palindrome = string_reverse("radar"); // "radar"
```

### String Analysis

#### string_count_char
```palladium
fn string_count_char(s: String, target: i64) -> i64
```
Counts occurrences of a character in a string.

**Example:**
```palladium
let count = string_count_char("hello world", 108);  // 3 (count of 'l')
let spaces = string_count_char("a b c d", 32);      // 3 (count of spaces)
```

### String Padding

#### string_pad_left
```palladium
fn string_pad_left(s: String, width: i64, pad_char: i64) -> String
```
Pads a string on the left to reach the specified width.

**Parameters:**
- `s`: The string to pad
- `width`: The desired total width
- `pad_char`: ASCII value of the padding character

**Example:**
```palladium
let padded = string_pad_left("42", 5, 48);     // "00042" (48 = '0')
let aligned = string_pad_left("hello", 10, 32); // "     hello" (32 = ' ')
```

#### string_pad_right
```palladium
fn string_pad_right(s: String, width: i64, pad_char: i64) -> String
```
Pads a string on the right to reach the specified width.

**Example:**
```palladium
let padded = string_pad_right("hello", 10, 46);  // "hello....." (46 = '.')
```

### String Joining

#### string_join2
```palladium
fn string_join2(s1: String, s2: String, sep: String) -> String
```
Joins two strings with a separator.

**Example:**
```palladium
let path = string_join2("home", "user", "/");        // "home/user"
let csv = string_join2("John", "Doe", ", ");         // "John, Doe"
```

#### string_join3
```palladium
fn string_join3(s1: String, s2: String, s3: String, sep: String) -> String
```
Joins three strings with a separator.

**Example:**
```palladium
let full_path = string_join3("home", "user", "documents", "/");  // "home/user/documents"
```

### Additional Trimming Functions

The extended utilities also provide their own trimming functions:

#### string_trim_start / string_trim_end / string_trim
Similar to the basic module but with the stdlib:: prefix.

## Complete Examples

### CSV Parser
```palladium
import stdlib::string_utils;

fn parse_csv_line(line: String) -> VecString {
    let mut values = vec_string_new();
    let mut current = line;
    
    while string_len(current) > 0 {
        let comma_pos = string_find_char(current, 44);  // ','
        
        match comma_pos {
            OptionInt::Some(pos) => {
                let value = string_substring(current, 0, pos);
                values = vec_string_push(values, string_trim(value));
                current = string_substring(current, pos + 1, string_len(current));
            }
            OptionInt::None => {
                // Last value
                values = vec_string_push(values, string_trim(current));
                current = "";
            }
        }
    }
    
    return values;
}
```

### Configuration File Parser
```palladium
import std::string;
import stdlib::string_utils;

fn parse_config_line(line: String) -> (String, String) {
    let trimmed = trim(line);
    
    // Skip comments and empty lines
    if string_len(trimmed) == 0 || starts_with(trimmed, "#") {
        return ("", "");
    }
    
    // Parse key=value
    let key = string_split_first(trimmed, 61);    // '='
    let value = string_split_rest(trimmed, 61);
    
    return (string_trim(key), string_trim(value));
}

fn is_boolean_true(value: String) -> bool {
    let lower = string_to_lower(value);
    return string_eq(lower, "true") || 
           string_eq(lower, "yes") || 
           string_eq(lower, "1");
}
```

### Text Formatter
```palladium
import stdlib::string_utils;

fn format_table_row(col1: String, col2: String, col3: String) -> String {
    let c1 = string_pad_right(col1, 20, 32);  // Space-padded
    let c2 = string_pad_right(col2, 15, 32);
    let c3 = string_pad_left(col3, 10, 32);
    
    return c1 + " | " + c2 + " | " + c3;
}

fn format_number_with_commas(n: i64) -> String {
    let s = int_to_string(n);
    if string_len(s) <= 3 {
        return s;
    }
    
    // Simple version for 4-6 digits
    if string_len(s) <= 6 {
        let thousands = string_substring(s, 0, string_len(s) - 3);
        let hundreds = string_substring(s, string_len(s) - 3, string_len(s));
        return thousands + "," + hundreds;
    }
    
    return s;  // TODO: Handle larger numbers
}
```

### String Validation
```palladium
import std::string;
import stdlib::string_utils;

fn is_valid_identifier(s: String) -> bool {
    if string_len(s) == 0 {
        return false;
    }
    
    // Must start with letter or underscore
    let first = string_char_at(s, 0);
    if !char_is_alpha(first) && first != 95 {  // '_'
        return false;
    }
    
    // Rest must be alphanumeric or underscore
    let i = 1;
    while i < string_len(s) {
        let ch = string_char_at(s, i);
        if !char_is_alpha(ch) && !char_is_digit(ch) && ch != 95 {
            return false;
        }
        i = i + 1;
    }
    
    return true;
}

fn is_valid_email(email: String) -> bool {
    // Simple validation
    if !contains(email, "@") {
        return false;
    }
    
    let at_count = string_count_char(email, 64);  // '@'
    if at_count != 1 {
        return false;
    }
    
    let parts = string_split_first(email, 64);
    let domain = string_split_rest(email, 64);
    
    return string_len(parts) > 0 && 
           string_len(domain) > 0 && 
           contains(domain, ".");
}
```

### Text Processing Pipeline
```palladium
import std::string;
import stdlib::string_utils;

fn normalize_text(text: String) -> String {
    // Trim whitespace
    let result = trim(text);
    
    // Convert to lowercase
    result = string_to_lower(result);
    
    // Replace multiple spaces with single space
    while contains(result, "  ") {
        result = string_replace_first(result, "  ", " ");
    }
    
    return result;
}

fn extract_words(text: String) -> VecString {
    let normalized = normalize_text(text);
    let mut words = vec_string_new();
    let mut current_word = "";
    let i = 0;
    
    while i < string_len(normalized) {
        let ch = string_char_at(normalized, i);
        
        if char_is_alpha(ch) {
            current_word = current_word + string_from_char(ch);
        } else {
            if string_len(current_word) > 0 {
                words = vec_string_push(words, current_word);
                current_word = "";
            }
        }
        i = i + 1;
    }
    
    // Don't forget the last word
    if string_len(current_word) > 0 {
        words = vec_string_push(words, current_word);
    }
    
    return words;
}
```

## Performance Tips

1. **Use StringBuilder for concatenation**: When building strings with many operations
2. **Avoid repeated string operations**: Cache results when possible
3. **Use character operations**: Working with ASCII values is faster than string operations
4. **Preallocate when possible**: Know your string sizes in advance

## Common Patterns

### Safe String Indexing
```palladium
fn safe_char_at(s: String, index: i64) -> i64 {
    if index < 0 || index >= string_len(s) {
        return -1;
    }
    return string_char_at(s, index);
}
```

### String Iteration
```palladium
fn for_each_char(s: String, f: fn(i64)) {
    let i = 0;
    let len = string_len(s);
    while i < len {
        f(string_char_at(s, i));
        i = i + 1;
    }
}
```

### Multiple Delimiters
```palladium
fn is_delimiter(ch: i64) -> bool {
    return ch == 32 ||   // space
           ch == 44 ||   // comma
           ch == 59 ||   // semicolon
           ch == 9;      // tab
}
```