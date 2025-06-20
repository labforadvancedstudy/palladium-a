# Palladium Programming Language

[![Crates.io](https://img.shields.io/crates/v/alan-von-palladium.svg)](https://crates.io/crates/alan-von-palladium)
[![Documentation](https://docs.rs/palladium/badge.svg)](https://docs.rs/palladium)
[![License](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](LICENSE)

Palladium is a systems programming language that combines Turing's correctness with von Neumann's performance.

## Features

- **Memory Safety**: Ownership system inspired by Rust
- **Type Safety**: Strong static typing with inference
- **Performance**: Compiles to optimized native code via C
- **Simplicity**: Clean, readable syntax
- **Self-Hosting**: 100% bootstrap capability achieved

## Quick Start

### Installation

```bash
cargo install alan-von-palladium
```

### Hello World

Create a file `hello.pd`:

```palladium
fn main() {
    print("Hello, World!");
}
```

Compile and run:

```bash
pdc hello.pd -o hello
./hello
```

## Language Features

### Variables and Types

```palladium
fn main() {
    let x: i32 = 42;
    let mut y = 10;  // Type inference
    y = y + 1;
    
    let message: string = "Hello";
    let flag: bool = true;
}
```

### Functions

```palladium
fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn greet(name: string) {
    print("Hello, ");
    print(name);
    print("!");
}
```

### Control Flow

```palladium
fn main() {
    let x = 10;
    
    if x > 5 {
        print("x is greater than 5");
    } else {
        print("x is 5 or less");
    }
    
    for i in 0..10 {
        print(i);
    }
    
    let mut count = 0;
    while count < 5 {
        count = count + 1;
    }
}
```

### Structs and Enums

```palladium
struct Point {
    x: i32,
    y: i32
}

enum Result<T, E> {
    Ok(T),
    Err(E)
}

fn main() {
    let p = Point { x: 10, y: 20 };
    
    let result: Result<i32, string> = Result::Ok(42);
    match result {
        Result::Ok(value) => print(value),
        Result::Err(error) => print(error)
    }
}
```

### Arrays

```palladium
fn main() {
    let numbers: [i32; 5] = [1, 2, 3, 4, 5];
    let first = numbers[0];
    
    for i in 0..5 {
        print(numbers[i]);
    }
}
```

## Tools

### Compiler (pdc)

```bash
# Compile to executable
pdc program.pd -o program

# Compile to C (for debugging)
pdc program.pd -c -o program.c

# With optimizations
pdc program.pd -O3 -o program
```

### Package Manager (pdm)

```bash
# Initialize new project
pdm init my_project

# Add dependency
pdm add some_package

# Build project
pdm build

# Run project
pdm run
```

### Language Server (pls)

The Palladium Language Server provides IDE support:

```bash
# Start language server
pls
```

Supported features:
- Syntax highlighting
- Error diagnostics
- Code completion
- Go to definition
- Find references

## Current Status

- ✅ Core language features
- ✅ Basic type system
- ✅ Pattern matching
- ✅ Memory safety via ownership
- ✅ Self-hosting compiler
- ⚠️  Standard library (in progress)
- ⚠️  Async/await (planned)
- ⚠️  Generics (planned)
- ⚠️  Traits (planned)

## Building from Source

```bash
git clone https://github.com/palladium-lang/palladium
cd palladium
cargo build --release
```

## Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

## License

Palladium is dual-licensed under MIT and Apache 2.0. See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE) for details.

## Acknowledgments

Special thanks to all contributors who helped achieve 100% bootstrap capability!