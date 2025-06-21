# Getting Started with Palladium

> ⚠️ **Note**: Palladium is currently in active development (v0.1.1). Some features mentioned in this guide may be incomplete or subject to change.

Welcome to the Palladium programming language! This guide will help you get started with installing and using Palladium.

## Table of Contents

1. [Installation](#installation)
2. [Your First Program](#your-first-program)
3. [Basic Examples](#basic-examples)
4. [Language Features](#language-features)
5. [Current Limitations](#current-limitations)
6. [Troubleshooting](#troubleshooting)

## Installation

### From crates.io (Recommended)

```bash
cargo install alan-von-palladium
```

### From Source

```bash
# Clone the repository
git clone https://github.com/labforadvancedstudy/palladium-a.git
cd palladium-a

# Build in release mode
cargo build --release

# The compiler binary will be at ./target/release/pdc
```

### Verify Installation

```bash
pdc --version
```

You should see:
```
pdc 0.1.1
```

## Your First Program

Create a file named `hello.pd`:

```palladium
// Your first Palladium program
fn main() {
    print("Hello, Palladium!");
}
```

Compile and run:

```bash
# Compile
pdc compile hello.pd -o hello

# Run
./build_output/hello
```

Output:
```
Hello, Palladium!
```

### Understanding the Compilation Process

When you compile a Palladium program, you'll see detailed output:

```
🔨 Compiling hello.pd...
📖 Lexing...
   Found 11 tokens (0.40ms)
🌳 Parsing...
   Parsed 1 top-level items (0.24ms)
🔮 Expanding macros...
   Macros expanded successfully! (0.19ms)
🔍 Type checking...
   All types check out! (0.38ms)
🔒 Borrow checking...
   Memory safety verified! (0.16ms)
🌊 Analyzing effects...
   Function 'main' has effects: {IO}
   Effect analysis complete!
⚠️  Checking unsafe operations...
   Unsafe operations verified!
🔧 Optimizing...
   Running Constant Folding
   Running Dead Code Elimination
   Running Expression Simplification
   Optimization complete (0.07ms)
⚡ Generating C code...
   Generated C code: build_output/hello.c
   Code generation complete (0.44ms)
✅ Compilation successful!
🔗 Linking with gcc...
   Created executable: build_output/hello
```

## Basic Examples

### Variables and Types

```palladium
fn main() {
    // Integer variables
    let x = 42;
    let y: i64 = 100;
    
    print("x = ");
    print_int(x);
    print("y = ");
    print_int(y);
    
    // Mutable variables
    let mut count = 0;
    count = count + 1;
    count = count + 1;
    
    print("count = ");
    print_int(count);
    
    // Strings
    let message = "Hello from Palladium!";
    print(message);
}
```

Output:
```
x = 
42
y = 
100
count = 
2
Hello from Palladium!
```

### Functions

```palladium
fn add(a: i64, b: i64) -> i64 {
    return a + b;
}

fn multiply(x: i64, y: i64) -> i64 {
    return x * y;
}

fn greet(name: String) {
    print("Hello, ");
    print(name);
    print("!");
}

fn main() {
    let sum = add(10, 20);
    print("10 + 20 = ");
    print_int(sum);
    
    let product = multiply(6, 7);
    print("6 * 7 = ");
    print_int(product);
    
    greet("Palladium");
}
```

Output:
```
10 + 20 = 
30
6 * 7 = 
42
Hello, 
Palladium
!
```

### Control Flow

```palladium
fn main() {
    // if-else statements
    let x = 10;
    if x > 5 {
        print("x is greater than 5");
    } else {
        print("x is less than or equal to 5");
    }
    
    // while loops
    print("Counting down:");
    let mut i = 5;
    while i > 0 {
        print_int(i);
        i = i - 1;
    }
    
    // for loops
    print("For loop:");
    for j in 0..5 {
        print_int(j);
    }
}
```

Output:
```
x is greater than 5
Counting down:
5
4
3
2
1
For loop:
0
1
2
3
4
```

### Arrays and Structs

```palladium
struct Point {
    x: i64,
    y: i64,
}

struct Person {
    age: i64,
    id: i64,
}

fn main() {
    // Arrays
    let numbers = [1, 2, 3, 4, 5];
    print("Array elements:");
    for i in 0..5 {
        print_int(numbers[i]);
    }
    
    // Array initialization
    let mut scores = [0; 5];  // 5 elements initialized to 0
    scores[0] = 100;
    scores[1] = 95;
    scores[2] = 87;
    
    // Structs
    let p = Point { x: 10, y: 20 };
    print("Point x:");
    print_int(p.x);
    print("Point y:");
    print_int(p.y);
    
    // Mutable structs
    let mut person = Person { age: 25, id: 1001 };
    person.age = person.age + 1;
    print("After birthday:");
    print_int(person.age);
}
```

Output:
```
Array elements:
1
2
3
4
5
Point x:
10
Point y:
20
After birthday:
26
```

## Language Features

### Currently Supported

✅ **Basic Types**
- `i32`, `i64` - Signed integers
- `u32`, `u64` - Unsigned integers  
- `bool` - Boolean values
- `String` - String type
- Arrays - Fixed-size arrays `[T; N]`

✅ **Control Flow**
- `if`/`else` statements
- `while` loops
- `for` loops with ranges
- `break` and `continue`

✅ **Functions**
- Function definitions with parameters
- Return types
- Multiple parameters

✅ **Structs and Enums**
- Struct definitions
- Field access
- Enum definitions
- Basic pattern matching

✅ **Memory Safety**
- Ownership system
- Borrowing checker
- Mutable and immutable references

✅ **Other Features**
- Single-line comments (`//`)
- Effects system (IO tracking)

### Pattern Matching Example

```palladium
enum Result {
    Ok(i64),
    Err(String),
}

fn divide(a: i64, b: i64) -> Result {
    if b == 0 {
        return Result::Err("Division by zero");
    }
    return Result::Ok(a / b);
}

fn main() {
    let result = divide(10, 2);
    match result {
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

Output:
```
Result: 
5
```

## Current Limitations

> ⚠️ **Development Status**: The following features are planned but not yet implemented:

### Not Yet Supported

❌ **Generics**
```palladium
// This will NOT compile
struct Vec<T> {
    data: [T; 100],
    len: i64,
}
```

❌ **Traits**
```palladium
// This will NOT compile
trait Display {
    fn display(&self);
}
```

❌ **Closures**
```palladium
// This will NOT compile
let add = |x, y| x + y;
```

❌ **Async/Await**
```palladium
// This will NOT compile
async fn fetch_data() -> String {
    await some_io_operation()
}
```

❌ **Modules and Imports**
```palladium
// This will NOT compile
use std::collections::Vec;
mod my_module;
```

❌ **Advanced Pattern Matching**
- Guards in match expressions
- Destructuring in patterns
- `if let` and `while let`

❌ **Other Missing Features**
- Multi-line comments (`/* */`)
- Nested block comments
- Hex/binary literals (`0xFF`, `0b1010`)
- String concatenation with `+`
- `else if` chains (use nested `if` instead)
- Implicit returns
- Method syntax (`obj.method()`)
- Operator overloading

### Known Issues

1. **Print Functions**: `print` and `print_int` output on separate lines
2. **Error Messages**: UTF-8 handling in error messages may have issues
3. **LLVM Backend**: SSA numbering issues prevent LLVM IR compilation
4. **Borrow Checker**: May be overly restrictive in some cases

## Troubleshooting

### Common Errors

**"Unexpected token"**
- Check for missing semicolons
- Ensure all brackets and parentheses are matched
- Verify function signatures are correct

**"Type mismatch"**
- Palladium has strict typing
- Ensure explicit types match
- Use explicit casts when needed

**"Borrow checker error"**
- Cannot have mutable and immutable borrows simultaneously
- Store intermediate values in variables to avoid complex borrowing

### Compiler Options

```bash
# Compile with optimization
pdc compile program.pd -o program -O

# Use LLVM backend (experimental)
pdc compile program.pd -o program --llvm

# Show help
pdc --help
```

## Next Steps

1. **Explore Examples**: Check the `examples/` directory for more code samples
2. **Read the User Guide**: Continue with the language chapters in this guide
3. **Report Issues**: Help improve Palladium by reporting bugs on GitHub
4. **Join the Community**: Contribute to the language development

## Getting Help

- **GitHub Issues**: https://github.com/labforadvancedstudy/palladium-a/issues
- **Documentation**: Continue reading this user guide
- **Examples**: `examples/tutorial/` and `examples/practical/`

Remember that Palladium is in active development. Your feedback and contributions are welcome!

---

*Last updated: January 2025 | Palladium v0.1.1*