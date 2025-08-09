# 'Alan von Palladium' - Palladium Programming Language

```
     _    __     ______    ____                      _ _ _                 
    / \   \ \   / /  _ \  / ___|___  _ __ ___  _ __ (_) | ___ _ __       
   / _ \   \ \ / /| |_) || |   / _ \| '_ ` _ \| '_ \| | |/ _ \ '__|      
  / ___ \   \ V / |  __/ | |__| (_) | | | | | | |_) | | |  __/ |        
 /_/   \_\   \_/  |_|     \____\___/|_| |_| |_| .__/|_|_|\___|_|         
                                               |_|                        
```

> *"When Turing's Proofs Meet von Neumann's Performance"*

[![Crates.io](https://img.shields.io/crates/v/alan-von-palladium.svg)](https://crates.io/crates/alan-von-palladium)
[![Documentation](https://docs.rs/alan-von-palladium/badge.svg)](https://docs.rs/alan-von-palladium)
[![License](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](LICENSE)

> ⚠️ **Alpha Software**: Palladium is in active development (v0.1.1). APIs and language features are subject to change.

Palladium is a systems programming language that combines Turing's correctness with von Neumann's performance.

## 🚀 Features

- **Memory Safety**: Ownership system inspired by Rust
- **Type Safety**: Strong static typing with inference
- **Performance**: Compiles to optimized native code via C
- **Simplicity**: Clean, readable syntax
- **Self-Hosting**: 100% bootstrap capability achieved

## 📦 Installation

### From crates.io (Recommended)

```bash
cargo install alan-von-palladium
```

### From Source

```bash
git clone https://github.com/labforadvancedstudy/palladium-a.git
cd palladium-a
cargo build --release

# Add to PATH
export PATH="$PATH:$(pwd)/target/release"
```

## 🎯 Quick Start

### Hello World

Create `hello.pd`:

```palladium
fn main() {
    print("Hello, World!");
}
```

Compile and run:

```bash
pdc compile hello.pd -o hello
./build_output/hello
```

Output:
```
Hello, World!
```

## 📚 Language Tour

### Variables and Types

```palladium
fn main() {
    // Immutable by default
    let x = 42;
    let y: i64 = 100;
    
    // Mutable variables
    let mut count = 0;
    count = count + 1;
    
    // Strings
    let message = "Hello, Palladium!";
    print(message);
}
```

### Functions

```palladium
fn add(a: i64, b: i64) -> i64 {
    return a + b;  // Explicit return required
}

fn greet(name: String) {
    print("Hello, ");
    print(name);
    print("!");
}

fn main() {
    let sum = add(10, 20);
    print_int(sum);  // Output: 30
    
    greet("Palladium");
}
```

### Control Flow

```palladium
fn main() {
    // if-else
    let x = 10;
    if x > 5 {
        print("x is greater than 5");
    } else {
        print("x is 5 or less");
    }
    
    // for loops
    for i in 0..5 {
        print_int(i);
    }
    
    // while loops
    let mut count = 5;
    while count > 0 {
        print_int(count);
        count = count - 1;
    }
}
```

### Structs and Enums

```palladium
struct Point {
    x: i64,
    y: i64,
}

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
    let p = Point { x: 10, y: 20 };
    print_int(p.x);
    
    let result = divide(10, 2);
    match result {
        Result::Ok(value) => {
            print_int(value);
        }
        Result::Err(msg) => {
            print(msg);
        }
    }
}
```

### Arrays

```palladium
fn main() {
    // Fixed-size arrays
    let numbers = [1, 2, 3, 4, 5];
    let zeros = [0; 10];  // Array of 10 zeros
    
    // Array access
    let first = numbers[0];
    print_int(first);
    
    // Iteration
    for i in 0..5 {
        print_int(numbers[i]);
    }
}
```

### Memory Safety

```palladium
fn main() {
    let x = 42;
    let y = &x;        // Immutable borrow
    print_int(*y);
    
    let mut z = 10;
    let w = &mut z;    // Mutable borrow
    *w = 20;
    print_int(z);      // Output: 20
}
```

## 🛠️ Compiler Usage

### Basic Commands

```bash
# Compile a file
pdc compile program.pd -o program

# Compile with optimization
pdc compile program.pd -o program -O

# Use LLVM backend (experimental)
pdc compile program.pd -o program --llvm

# Show help
pdc --help
```

### Compilation Process

When you compile, you'll see detailed progress:

```
🔨 Compiling program.pd...
📖 Lexing...
🌳 Parsing...
🔍 Type checking...
🔒 Borrow checking...
🌊 Analyzing effects...
⚠️  Checking unsafe operations...
🔧 Optimizing...
⚡ Generating C code...
✅ Compilation successful!
🔗 Linking...
```

## 📊 Current Status

### ✅ Implemented

- Core language features (variables, functions, control flow)
- Basic type system (integers, booleans, strings, arrays)
- Structs and enums
- Pattern matching (basic)
- Ownership and borrowing
- Effects system
- C code generation backend

### 🚧 In Progress

- Standard library (Vec, HashMap, etc.)
- LLVM backend optimization
- Package manager (pdm)
- Language server (pls)

### 📋 Planned

- Generics
- Traits
- Async/await
- Closures
- Module system
- Macro system

### ⚠️ Known Limitations

- No implicit returns (use explicit `return`)
- No `else if` (use nested `if`)
- No method syntax (`obj.method()`)
- Limited pattern matching
- `print` and `print_int` output on separate lines
- UTF-8 handling in error messages needs work

## 🏗️ Building from Source

```bash
# Clone repository
git clone https://github.com/labforadvancedstudy/palladium-a.git
cd palladium-a

# Build in release mode
cargo build --release

# Run tests
cargo test

# Install locally
cargo install --path .
```

## 📖 Documentation

- [Getting Started Guide](docs/user-guide/getting-started.md)
- [Language Reference](docs/specification/language_specification.md)
- [User Guide](docs/user-guide/)
- [Examples](examples/)

## 🧪 Examples

Check out the `examples/` directory:

- `examples/tutorial/` - Step-by-step tutorials
- `examples/practical/` - Real-world examples

```bash
# Run an example
pdc compile examples/tutorial/01_hello_world.pd -o hello
./build_output/hello
```

## 🤝 Contributing

We welcome contributions! Areas where help is needed:

- Standard library implementation
- Documentation improvements
- Bug fixes
- Test coverage
- LLVM backend improvements

Please see our [Contributing Guide](CONTRIBUTING.md) for details.

## 📊 Benchmarks

Performance comparisons coming soon. Goal: within 10% of C performance.

## 🔍 Philosophy

Palladium aims to be:

1. **Safe**: Memory and type safety by default
2. **Fast**: Zero-cost abstractions, optimal performance
3. **Simple**: Clear syntax, minimal complexity
4. **Practical**: Designed for real systems programming

## 📜 License

Palladium is dual-licensed:

- MIT License ([LICENSE-MIT](LICENSE-MIT))
- Apache License 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

Choose whichever license works best for you.

## 🙏 Acknowledgments

Special thanks to:

- All contributors who helped achieve 100% bootstrap capability
- The Rust community for inspiration
- Alan Turing and John von Neumann for their legendary contributions to computing

---

**Project Status**: Alpha (v0.1.1) | **First Release**: June 2025 | **Bootstrap**: 100% Complete

*"Combining Turing's correctness with von Neumann's performance"*
