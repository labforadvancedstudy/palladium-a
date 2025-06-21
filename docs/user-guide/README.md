# The Alan von Palladium Book

*A Feynman-style guide to understanding Palladium*

> "If you can't explain it simply, you don't understand it well enough."  
> — Often attributed to Einstein, but lived by Feynman

> ⚠️ **Note**: This book is being updated for Palladium v0.1.1 (Alpha). Some advanced features described in later chapters are not yet implemented.

## 🚀 Quick Start

**New to Palladium?** Start with the [Getting Started Guide](./getting-started.md) to install Palladium and write your first program!

## 📚 Table of Contents

### Part I: Fundamentals ✅
1. [Getting Started](getting-started.md) - Installation and first program
2. [Chapter 1: What's the Problem?](chapter_1_the_problem.md) - Why we need a new language
3. [Chapter 2: Memory is Just Boxes](chapter_2_memory.md) - Understanding memory management
4. [Chapter 3: Types are Shapes](chapter_3_types.md) - Type system fundamentals
5. [Chapter 4: Functions are Machines](chapter_4_functions.md) - Functions and control flow

### Part II: Memory Safety ✅
6. [Chapter 5: Ownership is Responsibility](chapter_5_ownership.md) - The ownership system

### Part III: Advanced Features ⚠️
7. [Chapter 6: Traits are Promises](chapter_6_traits.md) - Trait-based polymorphism *(Coming Soon)*
8. [Chapter 7: Async is Just Waiting](chapter_7_async.md) - Asynchronous programming *(Coming Soon)*
9. [Chapter 8: Effects are Side Stories](chapter_8_effects.md) - Effect tracking system *(Partially Implemented)*

### Part IV: Future Vision 📋
10. [Chapter 9: Proofs are Certainty](chapter_9_proofs.md) - Formal verification *(Planned)*
11. [Chapter 10: Building Real Things](chapter_10_applications.md) - Real-world applications

## 🎯 Current Implementation Status

### ✅ Available Now (v0.1.1)
- Basic syntax and types
- Functions and control flow
- Structs and enums
- Basic pattern matching
- Ownership and borrowing
- Effects tracking (basic)
- C code generation

### 🚧 In Development
- Standard library (Vec, HashMap)
- LLVM backend
- Package manager
- Language server

### 📋 Planned Features
- Generics
- Traits
- Async/await
- Modules
- Macros
- Formal verification

## How to Read This Book

This isn't your typical programming language book. We won't start with syntax or grammar rules. Instead, we'll start with problems—real problems that programmers face every day—and discover how Palladium solves them.

Like Feynman teaching physics, we'll use:
- **Simple analogies** that a child could understand
- **Concrete examples** before abstract concepts  
- **Working code** that you can run and modify
- **"What if?" questions** to explore ideas
- **No jargon** until we've earned it

## Running the Examples

All examples in Part I (Fundamentals) work with the current compiler:

```bash
# Install Palladium
cargo install alan-von-palladium

# Run an example
pdc compile example.pd -o example
./build_output/example
```

Examples in later chapters may include features not yet implemented. These are marked with ⚠️.

## Who This Book Is For

- **Curious programmers** who want to understand, not just memorize
- **Students** learning their first systems language
- **Experienced developers** seeking a fresh perspective
- **Anyone** who's ever wondered "Why is programming so hard?"

## The Feynman Method

Throughout this book, we'll use Feynman's teaching method:

1. **Explain it to a child** - If you can't, you don't understand it
2. **Show working examples** - Code that actually runs
3. **Identify gaps** - Where does the explanation break down?
4. **Simplify** - Remove unnecessary complexity

## Contributing

Found an error? Have a suggestion? Please open an issue on GitHub!

This book is open source and contributions are welcome:
- Fix typos or errors
- Add more examples
- Improve explanations
- Update for new features

## Let's Begin

Ready? Let's start with installation and your first program...

[Getting Started →](getting-started.md)

---

*Last updated: January 2025 | For Palladium v0.1.1 (Alpha)*