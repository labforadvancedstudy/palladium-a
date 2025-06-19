# Palladium Standard Library

The Palladium standard library provides essential functionality for Palladium programs. It includes core types, collections, I/O operations, and utility functions.

## Structure

```
stdlib/
├── prelude.pd          # Automatically imported items
└── std/
    ├── mod.pd          # Main module file
    ├── option.pd       # Option type for nullable values
    ├── result.pd       # Result type for error handling
    ├── collections/    # Data structures
    │   ├── mod.pd
    │   ├── vec.pd      # Dynamic arrays
    │   └── hashmap.pd  # Hash tables
    ├── string.pd       # String utilities
    ├── io.pd          # Input/output operations
    ├── math.pd        # Mathematical functions
    ├── mem.pd         # Memory management
    └── traits.pd      # Core traits/interfaces
```

## Core Types

### Option<T>
Represents an optional value - either `Some(T)` or `None`.

```palladium
use std::option::{Option, Some, None};

let x: Option<i64> = Some(42);
let y: Option<i64> = None;

// Pattern matching
match x {
    Some(value) => print_int(value),
    None => print("No value"),
}
```

### Result<T, E>
Represents either success (`Ok(T)`) or failure (`Err(E)`).

```palladium
use std::result::{Result, Ok, Err};

fn divide(a: i64, b: i64) -> Result<i64, String> {
    if b == 0 {
        Err("Division by zero")
    } else {
        Ok(a / b)
    }
}
```

## Collections

### Vec<T>
A growable array type with heap allocation.

```palladium
use std::collections::vec::Vec;

let mut v = Vec::new();
v.push(1);
v.push(2);
v.push(3);

for i in 0..v.len() {
    print_int(v[i]);
}
```

### HashMap<K, V>
A hash table implementation for key-value storage.

```palladium
use std::collections::hashmap::HashMap;

let mut map = HashMap::new();
map.insert("hello", 42);
map.insert("world", 100);

match map.get(&"hello") {
    Some(value) => print_int(*value),
    None => print("Not found"),
}
```

## String Utilities

Enhanced string manipulation functions:

```palladium
use std::string::String;

let s = "  hello world  ";
let trimmed = s.trim();
let upper = s.to_uppercase();
let parts = s.split(" ");
```

## I/O Operations

File and console I/O:

```palladium
use std::io::{File, Path, println};

// Console output
println(&"Hello, world!");

// File operations (when runtime support is available)
let path = Path::new(&"/tmp/test.txt");
let mut file = File::create(&path.to_string())?;
file.write_string(&"Hello, file!")?;
```

## Math Functions

Common mathematical operations:

```palladium
use std::math::{abs, pow, gcd, factorial, is_prime};

let x = abs(-42);        // 42
let y = pow(2, 10);      // 1024
let z = gcd(48, 18);     // 6
let f = factorial(5);    // 120
let p = is_prime(17);    // true
```

## Memory Management

Low-level memory operations:

```palladium
use std::mem::{size_of, Box, Rc};

// Smart pointers
let boxed = Box::new(42);
let shared = Rc::new("shared string");

// Size information
let size = size_of::<i64>(); // 8
```

## Traits

Core traits define common behavior:

```palladium
use std::traits::{Clone, Display, Iterator};

// Implement Display for custom type
impl Display for MyType {
    fn fmt(self: &MyType) -> String {
        "MyType instance"
    }
}
```

## The Prelude

The prelude module is automatically imported into every Palladium program. It includes:

- Core types: `Option`, `Result`
- Common traits: `Clone`, `Copy`, `Debug`, `Display`
- Collections: `Vec`, `HashMap`
- I/O functions: `print`, `println`
- Math functions: `abs`, `min`, `max`
- Assertions: `assert`, `assert_eq`

## Usage

In your Palladium programs:

```palladium
// Prelude items are automatically available
let v = Vec::new();
let opt = Some(42);

// Import specific items
use std::io::File;
use std::math::PI;

// Import entire modules
use std::collections::hashmap;
```

## Implementation Status

Current implementation provides:
- ✅ Core types (Option, Result)
- ✅ Collections (Vec, HashMap)
- ✅ String utilities
- ✅ Math functions
- ✅ Basic I/O structure
- ✅ Memory utilities
- ✅ Trait definitions
- ✅ Prelude

Note: Some I/O and memory operations require runtime support that may not be fully implemented in the current compiler version.