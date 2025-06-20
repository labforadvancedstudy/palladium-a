# Collections Library Reference

This page documents the collection types available in the Palladium standard library.

## VecInt - Dynamic Integer Array

A growable array implementation for storing integers.

### Import
```palladium
import stdlib::vec_simple;
```

### Type Definition
```palladium
struct VecInt {
    data: [i64; 1000];  // Fixed backing array
    len: i64;           // Current number of elements
    capacity: i64;      // Maximum capacity (1000)
}
```

### Constructor

#### vec_int_new
```palladium
fn vec_int_new() -> VecInt
```
Creates a new empty vector.

**Example:**
```palladium
let mut vec = vec_int_new();
```

### Core Operations

#### vec_int_push
```palladium
fn vec_int_push(vec: mut VecInt, value: i64) -> VecInt
```
Adds an element to the end of the vector.

**Parameters:**
- `vec`: The vector to modify
- `value`: The value to add

**Returns:**
- The modified vector

**Example:**
```palladium
vec = vec_int_push(vec, 42);
vec = vec_int_push(vec, 100);
```

#### vec_int_get
```palladium
fn vec_int_get(vec: VecInt, index: i64) -> i64
```
Gets the element at the specified index.

**Parameters:**
- `vec`: The vector to read from
- `index`: The zero-based index

**Returns:**
- The value at the index, or 0 if out of bounds

**Example:**
```palladium
let first = vec_int_get(vec, 0);
let second = vec_int_get(vec, 1);
```

#### vec_int_set
```palladium
fn vec_int_set(vec: mut VecInt, index: i64, value: i64) -> VecInt
```
Sets the element at the specified index.

**Parameters:**
- `vec`: The vector to modify
- `index`: The zero-based index
- `value`: The new value

**Returns:**
- The modified vector

**Example:**
```palladium
vec = vec_int_set(vec, 0, 999);
```

#### vec_int_pop
```palladium
fn vec_int_pop(vec: mut VecInt) -> (VecInt, i64)
```
Removes and returns the last element.

**Returns:**
- A tuple containing the modified vector and the popped value (0 if empty)

**Example:**
```palladium
let (vec, last_value) = vec_int_pop(vec);
print_int(last_value);
```

### Query Operations

#### vec_int_len
```palladium
fn vec_int_len(vec: VecInt) -> i64
```
Returns the number of elements in the vector.

**Example:**
```palladium
let count = vec_int_len(vec);
print("Vector has " + int_to_string(count) + " elements");
```

#### vec_int_is_empty
```palladium
fn vec_int_is_empty(vec: VecInt) -> bool
```
Checks if the vector is empty.

**Example:**
```palladium
if vec_int_is_empty(vec) {
    print("Vector is empty");
}
```

#### vec_int_contains
```palladium
fn vec_int_contains(vec: VecInt, value: i64) -> bool
```
Checks if the vector contains a specific value.

**Example:**
```palladium
if vec_int_contains(vec, 42) {
    print("Found 42 in the vector");
}
```

#### vec_int_find
```palladium
fn vec_int_find(vec: VecInt, value: i64) -> i64
```
Finds the index of the first occurrence of a value.

**Returns:**
- The index of the value, or -1 if not found

**Example:**
```palladium
let index = vec_int_find(vec, 42);
if index >= 0 {
    print("Found at index: " + int_to_string(index));
}
```

### Modification Operations

#### vec_int_clear
```palladium
fn vec_int_clear(vec: mut VecInt) -> VecInt
```
Removes all elements from the vector.

**Example:**
```palladium
vec = vec_int_clear(vec);
```

#### vec_int_remove
```palladium
fn vec_int_remove(vec: mut VecInt, index: i64) -> VecInt
```
Removes the element at the specified index, shifting subsequent elements left.

**Example:**
```palladium
vec = vec_int_remove(vec, 2);  // Remove element at index 2
```

#### vec_int_insert
```palladium
fn vec_int_insert(vec: mut VecInt, index: i64, value: i64) -> VecInt
```
Inserts an element at the specified index, shifting subsequent elements right.

**Example:**
```palladium
vec = vec_int_insert(vec, 1, 99);  // Insert 99 at index 1
```

#### vec_int_reverse
```palladium
fn vec_int_reverse(vec: mut VecInt) -> VecInt
```
Reverses the order of elements in place.

**Example:**
```palladium
vec = vec_int_reverse(vec);
```

#### vec_int_sort
```palladium
fn vec_int_sort(vec: mut VecInt) -> VecInt
```
Sorts the vector in ascending order (uses bubble sort).

**Example:**
```palladium
vec = vec_int_sort(vec);
```

### Aggregate Operations

#### vec_int_sum
```palladium
fn vec_int_sum(vec: VecInt) -> i64
```
Returns the sum of all elements.

**Example:**
```palladium
let total = vec_int_sum(vec);
print("Sum: " + int_to_string(total));
```

#### vec_int_min
```palladium
fn vec_int_min(vec: VecInt) -> i64
```
Returns the minimum element, or 0 if empty.

**Example:**
```palladium
let minimum = vec_int_min(vec);
```

#### vec_int_max
```palladium
fn vec_int_max(vec: VecInt) -> i64
```
Returns the maximum element, or 0 if empty.

**Example:**
```palladium
let maximum = vec_int_max(vec);
```

### Utility Operations

#### vec_int_print
```palladium
fn vec_int_print(vec: VecInt)
```
Prints the vector contents in a formatted way.

**Example:**
```palladium
vec_int_print(vec);  // Output: [1, 2, 3, 4, 5]
```

### Complete Example

```palladium
import stdlib::vec_simple;

fn demonstrate_vectors() {
    // Create and populate a vector
    let mut vec = vec_int_new();
    vec = vec_int_push(vec, 30);
    vec = vec_int_push(vec, 10);
    vec = vec_int_push(vec, 50);
    vec = vec_int_push(vec, 20);
    vec = vec_int_push(vec, 40);
    
    print("Original vector:");
    vec_int_print(vec);
    
    // Sort the vector
    vec = vec_int_sort(vec);
    print("Sorted vector:");
    vec_int_print(vec);
    
    // Calculate statistics
    print("Sum: " + int_to_string(vec_int_sum(vec)));
    print("Min: " + int_to_string(vec_int_min(vec)));
    print("Max: " + int_to_string(vec_int_max(vec)));
    print("Length: " + int_to_string(vec_int_len(vec)));
    
    // Find an element
    let index = vec_int_find(vec, 30);
    if index >= 0 {
        print("Found 30 at index: " + int_to_string(index));
    }
    
    // Remove an element
    vec = vec_int_remove(vec, 2);
    print("After removing index 2:");
    vec_int_print(vec);
    
    // Pop the last element
    let (vec, popped) = vec_int_pop(vec);
    print("Popped: " + int_to_string(popped));
    
    // Clear the vector
    vec = vec_int_clear(vec);
    print("Is empty: " + bool_to_string(vec_int_is_empty(vec)));
}
```

## HashMap - String to Integer Mapping

A simple hash map implementation for mapping strings to integers.

### Import
```palladium
import stdlib::hashmap_simple;
```

### Type Definition
```palladium
struct HashMap {
    keys: [String; 100];    // Fixed array of keys
    values: [i64; 100];     // Corresponding values
    size: i64;              // Number of entries
}
```

### Constructor

#### hashmap_new
```palladium
fn hashmap_new() -> HashMap
```
Creates a new empty hash map.

**Example:**
```palladium
let mut map = hashmap_new();
```

### Core Operations

#### hashmap_insert
```palladium
fn hashmap_insert(map: HashMap, key: String, value: i64) -> HashMap
```
Inserts or updates a key-value pair.

**Parameters:**
- `map`: The hash map to modify
- `key`: The string key
- `value`: The integer value

**Returns:**
- The modified hash map

**Example:**
```palladium
map = hashmap_insert(map, "age", 25);
map = hashmap_insert(map, "score", 100);
```

#### hashmap_get
```palladium
fn hashmap_get(map: HashMap, key: String) -> i64
```
Gets the value associated with a key.

**Parameters:**
- `map`: The hash map to search
- `key`: The key to look up

**Returns:**
- The value associated with the key, or -1 if not found

**Example:**
```palladium
let age = hashmap_get(map, "age");
if age >= 0 {
    print("Age: " + int_to_string(age));
}
```

#### hashmap_contains
```palladium
fn hashmap_contains(map: HashMap, key: String) -> bool
```
Checks if a key exists in the map.

**Example:**
```palladium
if hashmap_contains(map, "score") {
    print("Score is recorded");
}
```

#### hashmap_remove
```palladium
fn hashmap_remove(map: HashMap, key: String) -> HashMap
```
Removes a key-value pair from the map.

**Example:**
```palladium
map = hashmap_remove(map, "temp");
```

#### hashmap_size
```palladium
fn hashmap_size(map: HashMap) -> i64
```
Returns the number of entries in the map.

**Example:**
```palladium
let count = hashmap_size(map);
print("Map has " + int_to_string(count) + " entries");
```

### Complete Example

```palladium
import stdlib::hashmap_simple;

fn demonstrate_hashmap() {
    let mut config = hashmap_new();
    
    // Add configuration values
    config = hashmap_insert(config, "width", 800);
    config = hashmap_insert(config, "height", 600);
    config = hashmap_insert(config, "fps", 60);
    config = hashmap_insert(config, "vsync", 1);
    
    // Read values
    let width = hashmap_get(config, "width");
    let height = hashmap_get(config, "height");
    print("Resolution: " + int_to_string(width) + "x" + int_to_string(height));
    
    // Check for optional settings
    if hashmap_contains(config, "vsync") {
        let vsync = hashmap_get(config, "vsync");
        if vsync == 1 {
            print("VSync enabled");
        }
    }
    
    // Update a value
    config = hashmap_insert(config, "fps", 144);
    
    // Remove a setting
    config = hashmap_remove(config, "vsync");
    
    print("Config has " + int_to_string(hashmap_size(config)) + " settings");
}
```

## StringBuilder - Efficient String Building

A string builder for efficient concatenation of many strings.

### Import
```palladium
import stdlib::string_builder;
```

### Type Definition
```palladium
struct StringBuilder {
    buffer: String;     // Internal buffer
    length: i64;        // Current length
}
```

### Constructor

#### sb_new
```palladium
fn sb_new() -> StringBuilder
```
Creates a new empty string builder.

**Example:**
```palladium
let mut sb = sb_new();
```

### Building Operations

#### sb_append
```palladium
fn sb_append(mut sb: StringBuilder, s: String)
```
Appends a string to the builder.

**Example:**
```palladium
sb_append(sb, "Hello, ");
sb_append(sb, "World!");
```

#### sb_append_char
```palladium
fn sb_append_char(mut sb: StringBuilder, ch: i64)
```
Appends a single character.

**Example:**
```palladium
sb_append_char(sb, 65);  // 'A'
sb_append_char(sb, 10);  // newline
```

#### sb_append_int
```palladium
fn sb_append_int(mut sb: StringBuilder, n: i64)
```
Appends an integer's string representation.

**Example:**
```palladium
sb_append(sb, "The answer is ");
sb_append_int(sb, 42);
```

#### sb_append_newline
```palladium
fn sb_append_newline(mut sb: StringBuilder)
```
Appends a newline character.

**Example:**
```palladium
sb_append(sb, "First line");
sb_append_newline(sb);
sb_append(sb, "Second line");
```

### Query Operations

#### sb_to_string
```palladium
fn sb_to_string(sb: StringBuilder) -> String
```
Converts the builder to a final string.

**Example:**
```palladium
let result = sb_to_string(sb);
print(result);
```

#### sb_len
```palladium
fn sb_len(sb: StringBuilder) -> i64
```
Returns the current length of the builder.

**Example:**
```palladium
let length = sb_len(sb);
```

#### sb_is_empty
```palladium
fn sb_is_empty(sb: StringBuilder) -> bool
```
Checks if the builder is empty.

**Example:**
```palladium
if !sb_is_empty(sb) {
    print(sb_to_string(sb));
}
```

### Modification Operations

#### sb_clear
```palladium
fn sb_clear(mut sb: StringBuilder)
```
Clears the builder's contents.

**Example:**
```palladium
sb_clear(sb);
```

### Complete Example

```palladium
import stdlib::string_builder;

fn generate_html_table(data: VecInt) -> String {
    let mut sb = sb_new();
    
    // Start HTML table
    sb_append(sb, "<table>\n");
    sb_append(sb, "  <tr><th>Index</th><th>Value</th></tr>\n");
    
    // Add rows
    let i = 0;
    let len = vec_int_len(data);
    while i < len {
        sb_append(sb, "  <tr><td>");
        sb_append_int(sb, i);
        sb_append(sb, "</td><td>");
        sb_append_int(sb, vec_int_get(data, i));
        sb_append(sb, "</td></tr>\n");
        i = i + 1;
    }
    
    // Close table
    sb_append(sb, "</table>\n");
    
    return sb_to_string(sb);
}

fn build_csv_line(values: VecInt) -> String {
    let mut sb = sb_new();
    let i = 0;
    let len = vec_int_len(values);
    
    while i < len {
        if i > 0 {
            sb_append_char(sb, 44);  // comma
        }
        sb_append_int(sb, vec_int_get(values, i));
        i = i + 1;
    }
    
    sb_append_newline(sb);
    return sb_to_string(sb);
}
```

## Performance Considerations

1. **VecInt**: 
   - Fixed capacity of 1000 elements
   - O(1) push/pop operations
   - O(n) insert/remove operations
   - O(n²) sort (bubble sort)

2. **HashMap**:
   - Fixed capacity of 100 entries
   - O(n) lookup (linear search)
   - Simple implementation, not optimized for large datasets

3. **StringBuilder**:
   - More efficient than repeated string concatenation
   - Reduces memory allocations
   - Ideal for building large strings incrementally

## Limitations and Future Work

Current limitations due to language features:
- Type-specific implementations (no generics yet)
- Fixed capacities (no dynamic memory allocation)
- Simple algorithms (e.g., bubble sort instead of quicksort)

Future improvements planned:
- Generic collections (Vec<T>, HashMap<K,V>)
- Dynamic resizing
- More efficient algorithms
- Additional collection types (Set, Queue, Stack)
- Iterator support