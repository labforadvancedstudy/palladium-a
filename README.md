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

- **Memory Safety**: Ownership and borrow checking at compile time
- **Type Safety**: Strong static typing
- **Performance**: Compiles to C, then to native code
- **Simplicity**: Clean, readable syntax

- **Self-Hosting**: achieved as a fixed point (see below)

### Self-hosting

`bootstrap/pdc.pd` is a Palladium compiler written in Palladium. It is verified as a **fixed
point**, not a demo — the C emitted by the stage-1 compiler and by the stage-2 compiler are
byte-identical:

```
$ make selfhost
== stage0: Rust pdc compiles bootstrap/pdc.pd ==
== stage1: pdc1 compiles bootstrap/pdc.pd ==   -> c1.c (972 lines) -> pdc2
== stage2: pdc2 compiles bootstrap/pdc.pd ==   -> c2.c (972 lines)
✅ SELF-HOSTING ACHIEVED — fixed point reached.
   9b0cf24e640eb689a1744ffdf589a44428ef5649  c1.c
   9b0cf24e640eb689a1744ffdf589a44428ef5649  c2.c
```

Earlier versions of this README claimed "100% bootstrap" while no Palladium-written compiler
had ever compiled itself; that claim was false and the compilers it pointed at could not have
worked. The language subset the bootstrap compiler is written in — and implements — is
specified in [`docs/specification/bootstrap-subset.md`](docs/specification/bootstrap-subset.md).

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
    let x: i64 = 42;
    let y: &i64 = &x;      // immutable borrow — annotate it
    print_int(*y);

    let mut z: i64 = 10;
    let w: &mut i64 = &mut z;
    *w = 20;
    print_int(z);          // 20
}
```

## 🛠️ Compiler Usage

### Basic Commands

```bash
# Compile a file
pdc compile program.pd -o program

# Compile with optimization
pdc compile program.pd -o program -O

# Show help
pdc --help
```

There is one working backend: the default, which compiles to C. The `--llvm`
flag exists and refuses — the LLVM text backend is a skeleton kept for
development, not something you can build with. See
[the specification](docs/specification/language-spec.md) §1.

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

### ✅ Works end-to-end

- Functions, `let`/assignment, `if`/`else`, `while`, `for`-over-range
- `i32`/`i64`/`u32`/`u64`, `bool`, `String`, fixed-size arrays
- Structs; enums with unit/tuple/struct variants; `match` on enums
- Top-level `const` and `static` items, with `static mut` for writable storage
- Ownership and borrow checking
- C code generation and linking

### ⚠️ Parses but is broken downstream

- `?` operator — emits C referencing a `struct Result` layout codegen never defines
- `async` / `.await` — emits a call to a `poll` member that is never generated
- Generic types in a struct field, and tuples in a struct field or an enum payload — refused by name
- Generics — generic arguments that are all-uppercase are misparsed as const generics
- `for` over an array *parameter* — uses `sizeof` on a decayed pointer

### ❌ Not implemented

> **This list is older than the compiler in places.** The pattern and tuple entries were corrected
> when issue #41 landed; the entries marked *(stale)* were falsified by earlier branches and are
> left standing rather than quietly deleted, because correcting them is that branch's receipt to
> write, not this one's. `docs/specification/language-spec.md`'s A-sections are the measured status.

- Traits (parse, then emit nothing — no dispatch mechanism exists)
- Closures
- *(stale)* method call syntax `obj.method()`, `else if`, `loop` — all implemented by
  `feat/m2-expressions`
- *(stale)* floats, bitwise operators, compound assignment (`+=`), `as` casts — same branch
- Chars as a distinct TYPE (`'a'` lexes and carries its scalar; its type is `i64`)
- Slice patterns, `ref`/`mut` bindings, field shorthand in a struct-variant pattern,
  destructuring `let`
- String interpolation
- Macro hygiene — expansion is textual and a macro body reads the CALL SITE's names. The macro
  system itself works for a token template with `$name` substitution (`macro double!(x) { $x * 2 }`).
  What a macro BODY or ARGUMENT may contain is a closed set and everything outside it is refused by
  name — non-integer literals, two-character operators, a bare parameter name, an unknown `$name`,
  a nested invocation. The three unusable builtins are NOT in that set: `println!`, `assert!` and
  `dbg!` fail with ordinary parse errors from their own expansions, which `A4.6` of the
  specification lists shape by shape

### ⚠️ Known Limitations

- A `match` on any type other than an enum or a `bool` needs a `_` or binding arm: no set of
  literal or range arms is complete, and coverage by ranges is not checked
- A chained tuple index needs parentheses — `(p.0).1`, because `.0.1` lexes as one float literal
- A one-element tuple `(e,)` is not a form this language has; `(e)` is grouping
- `print` and `print_int` output on separate lines
- `pdc` must be run from the repository root: it links `runtime/palladium_runtime.c` by
  relative path

Feature-by-feature status with evidence: [`docs/specification/language-spec.md`](docs/specification/language-spec.md).
Run `scripts/conformance.sh` to reproduce the current numbers.

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
- [Language Specification](docs/specification/language-spec.md) — what the compiler actually implements
- [Bootstrap Subset (PBS-1)](docs/specification/bootstrap-subset.md) — the self-hosting target
- [User Guide](docs/user-guide/)
- [Examples](examples/)

## 🧪 Examples

Check out the `examples/` directory:

- `examples/tutorial/` - Step-by-step tutorials
- `examples/practical/` - Real-world examples

```bash
# Run an example
pdc compile examples/tutorial/01_variables.pd -o vars
./build_output/vars
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

Palladium is released under the MIT License — see [LICENSE](LICENSE).

(Earlier revisions of this section advertised a dual MIT/Apache-2.0 licence and linked to
`LICENSE-MIT` and `LICENSE-APACHE`. Neither file has ever existed in this repository, and
`Cargo.toml` declares `MIT`.)

## 🙏 Acknowledgments

Special thanks to:

- All contributors to the compiler and standard library
- The Rust community for inspiration
- Alan Turing and John von Neumann for their legendary contributions to computing

---

**Project Status**: Alpha (v0.1.1) | **Self-hosting**: not achieved — see `docs/specification/bootstrap-subset.md`

*"Combining Turing's correctness with von Neumann's performance"*
