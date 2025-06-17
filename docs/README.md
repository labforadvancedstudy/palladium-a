# Palladium Language Documentation

Welcome to the official documentation for the Palladium programming language - where Turing's proofs meet von Neumann's performance!

## 🚀 Quick Navigation

### Getting Started
- [📖 Getting Started Guide](./guides/getting-started.md) - Your first Palladium program
- [🎯 Language Tour](./guides/language-tour.md) - Learn Palladium in 30 minutes
- [🔄 Migrating from Rust](./guides/migration-from-rust.md) - For Rustaceans

### Implementation Status
- [📊 Feature Status Dashboard](./features/status.yaml) - Real-time progress tracking
- [🎉 Bootstrap Documentation](./bootstrap/) - Self-hosting journey
- [📈 Progress Reports](../reports/) - Detailed milestone updates

### Language Features
- [🧬 Core Language](./features/core-language/) - Type system, memory, control flow
- [⚡ Async System](./features/async-system/) - Revolutionary async design
- [🔬 Advanced Features](./features/advanced/) - Verification, proofs, safety

### Reference Documentation
- [📚 Language Reference](./reference/) - Syntax and semantics
- [🛠️ Standard Library](./reference/stdlib/) - Built-in types and functions
- [🔧 Tools](./tools/) - Compiler, formatter, LSP

### Developer Resources
- [🏗️ Compiler Internals](./internals/) - How Palladium works under the hood
- [🤝 Contributing Guide](../CONTRIBUTING.md) - Join the revolution
- [📝 Design Documents](./design/) - Language philosophy and decisions

## 📊 Current Implementation Status

### Overall Progress: 45% Complete

```
Core Language    [██████░░░░] 59%  - Type system, borrowing, lifetimes
Bootstrap        [██████░░░░] 60%  - Self-hosting compiler progress  
Advanced         [███░░░░░░░] 32%  - Verification, macros, optimization
Tooling          [███░░░░░░░] 32%  - Compiler, formatter, IDE support
Ecosystem        [████░░░░░░] 42%  - Standard library, FFI, packages
```

### 🎯 Next Milestones

1. **Complete Self-hosting** (7-10 days)
   - Fix string type inference in tiny compiler
   - Add missing expression parsing
   - Test full compiler self-compilation

2. **Enhanced Type System** (2-3 weeks)
   - Complete trait implementation
   - Add const generics
   - Improve type inference

3. **Developer Experience** (1 month)
   - LSP server for IDE support
   - Better error messages
   - Package manager alpha

## 🌟 Why Palladium?

Palladium solves the fundamental tension in systems programming:

| Feature | Rust | Go | C++ | Palladium |
|---------|------|-----|-----|-----------|
| Memory Safety | ✅ Manual | 🤷 GC | ❌ Manual | ✅ Automatic |
| Performance | ✅ Excellent | 🤷 Good | ✅ Excellent | ✅ Excellent |
| Ergonomics | 🤷 Complex | ✅ Simple | 🤷 Complex | ✅ Simple |
| Verification | ❌ External | ❌ None | ❌ None | ✅ Built-in |
| Async | 🤷 Colored | 🤷 Goroutines | ❌ Library | ✅ Transparent |

## 📚 Documentation Structure

```
docs/
├── features/          # Feature specifications and status
│   ├── status.yaml   # Progress tracking (single source of truth)
│   ├── core-language/
│   ├── async-system/
│   └── advanced/
├── guides/           # Tutorials and how-tos
├── reference/        # Language and library reference
├── bootstrap/        # Self-hosting documentation
├── internals/        # Compiler design and implementation
├── tools/           # Tooling documentation
├── design/          # Architecture and vision documents
├── release/         # Release notes and changelogs
└── planning/        # Development planning
```

## 🔄 Staying Updated

- **Status Updates**: Check [status.yaml](./features/status.yaml) for real-time progress
- **Release Notes**: See [release/](./release/) for version history
- **Blog**: Visit [palladium-lang.org/blog](https://palladium-lang.org/blog)
- **Discord**: Join our [community](https://discord.gg/palladium)

## 🤝 Contributing

We welcome contributions! See our [Contributing Guide](../CONTRIBUTING.md) for:
- Setting up the development environment
- Understanding the codebase
- Submitting pull requests
- Coding standards

## 📖 Learning Path

1. **Beginner**: Start with [Getting Started](./guides/getting-started.md)
2. **Intermediate**: Explore [Language Features](./features/)
3. **Advanced**: Dive into [Compiler Internals](./internals/)
4. **Expert**: Contribute to [Bootstrap Compiler](./bootstrap/)

## 🗂️ Directory Overview

### features/
Feature specifications with implementation status:
- `status.yaml` - Central progress tracking
- Individual feature documentation with code examples

### guides/
User-friendly tutorials and guides:
- Getting started
- Migration guides
- Best practices

### bootstrap/
Self-hosting compiler documentation:
- Bootstrap strategy
- Implementation progress
- Tutorial for contributing

### internals/
Compiler design and implementation:
- Architecture overview
- Type system design
- Code generation strategy

### design/
High-level design documents:
- Technical manifesto
- Vision and roadmap
- Comparison with other languages

### tools/
Documentation for Palladium tooling:
- pdc compiler usage
- formatter configuration
- IDE integration

---

*"In Palladium, your proofs compile to bare metal performance."*