# Palladium Language Documentation

Welcome to the official documentation for the Palladium programming language!

> *"When Turing's Proofs Meet von Neumann's Performance"*

## 📊 Current Status: v0.8-alpha (85% Complete)

### Major Achievements ✅
- **100% Self-Hosting** - Compiler compiles itself
- **LLVM Backend** - Native code generation
- **Package Manager (pdm)** - Modern dependency management
- **Language Server (pls)** - Full IDE support
- **Formal Specification** - Complete language spec with EBNF grammar

### Progress Overview
```
Core Language    [█████████░] 95%  - Type system, ownership, async/effects
Compiler         [██████████] 100% - Self-hosting achieved!
Standard Library [███████░░░] 70%  - Core types, collections, I/O
Tooling          [████████░░] 85%  - pdm, pls, build system
Documentation    [███████░░░] 75%  - Spec, book, guides
Performance      [██████░░░░] 60%  - Optimization in progress
```

## 🚀 Quick Navigation

### For New Users
- 📚 **[The Alan von Palladium Book](the_palladium_book/)** - Learn Palladium the Feynman way
- 🎯 **[Getting Started Guide](guides/getting-started.md)** - Your first program
- 📋 **[Language Specification](language_specification.md)** - Formal specification

### For Contributors  
- 🎉 **[Bootstrap Documentation](bootstrap/)** - Self-hosting journey (100% complete!)
- 🏗️ **[Compiler Internals](internals/)** - How Palladium works
- 📈 **[Roadmap to v1.0](../MILESTONES.md)** - What's next

### Reference
- 📖 **[Language Reference](reference/LANGUAGE_REFERENCE.md)** - Syntax and semantics
- 🛠️ **[Standard Library](stdlib/)** - API documentation
- 🔧 **[Tools Documentation](tools/)** - pdm, pls, pdc

## 🎯 Learning Path

### Beginner
1. Start with [The Alan von Palladium Book](the_palladium_book/)
2. Try the [Getting Started Guide](guides/getting-started.md)
3. Explore [Example Programs](../examples/)

### Intermediate
1. Read the [Language Specification](language_specification.md)
2. Study [Design Documents](design/)
3. Build a small project with pdm

### Advanced
1. Explore [Bootstrap Code](bootstrap/)
2. Dive into [Compiler Internals](internals/)
3. Contribute to the compiler!

## 📁 Documentation Structure

```
docs/
├── the_palladium_book/    # Feynman-style guide (NEW!)
├── bootstrap/             # Self-hosting docs (UPDATED)
├── design/               # Architecture and philosophy
├── features/             # Feature specifications
├── guides/               # Tutorials and how-tos
├── internals/            # Compiler implementation
├── reference/            # Language reference
├── stdlib/               # Standard library docs
├── tools/                # Tooling documentation
├── language_specification.md  # Formal spec
├── grammar.ebnf          # EBNF grammar
└── semantics.md          # Operational semantics
```

## 🌟 Why Palladium?

Palladium solves the fundamental tension in systems programming:

| Feature | Rust | Go | C++ | Palladium |
|---------|------|-----|-----|-----------|
| Memory Safety | ✅ Complex | 🤷 GC | ❌ Manual | ✅ Automatic |
| Performance | ✅ Excellent | 🤷 Good | ✅ Excellent | ✅ Excellent |
| Ergonomics | 🤷 Learning curve | ✅ Simple | 🤷 Complex | ✅ Simple |
| Verification | ❌ External | ❌ None | ❌ None | ✅ Built-in |
| Async | 🤷 Colored | 🤷 Goroutines | ❌ Library | ✅ Effects |

## 🚀 What's New in v0.8-alpha

- ✅ Complete self-hosting capability
- ✅ LLVM backend for native performance
- ✅ Package manager with dependency resolution
- ✅ Language server for IDE support
- ✅ Formal language specification
- ✅ Pattern matching for enums
- ✅ Async/effects system
- ✅ Trait system with bounds

## 🗺️ Roadmap to v1.0

### v0.9-beta (February 2025)
- Complete standard library
- Multi-platform support
- Performance optimizations
- Production error messages

### v0.95-rc (March 2025)
- Package registry (crates.pd)
- Debugger integration
- Complete documentation
- Enterprise features

### v1.0 (May 2025)
- Stability guarantee
- LTS support
- Production ready
- Community launch

## 📖 Key Documents

### Current
- [Language Specification](language_specification.md) - Complete formal spec
- [EBNF Grammar](grammar.ebnf) - Parser grammar
- [The Palladium Book](the_palladium_book/) - User guide

### Design
- [Technical Manifesto](design/avp_technical_manifesto.md) - Vision
- [Trait System Design](design/trait_system_design.md) - Type system
- [Visual Documentation](visual/) - Feature tracking

### Historical
- [Bootstrap Journey](bootstrap/) - Self-hosting story
- [Early Releases](release/) - Version history

## 🤝 Contributing

We welcome contributions! Key areas:
- 🐛 Bug fixes and testing
- 📚 Documentation improvements
- 🧪 Standard library implementations
- 🌍 Platform ports
- 🎨 Tooling enhancements

See [CONTRIBUTING.md](../CONTRIBUTING.md) for details.

## 🔄 Stay Updated

- **GitHub**: [palladium-lang/palladium](https://github.com/palladium-lang/palladium)
- **Discord**: [Join our community](https://discord.gg/palladium)
- **Twitter**: [@palladium_lang](https://twitter.com/palladium_lang)
- **Blog**: [palladium-lang.org](https://palladium-lang.org)

---

*"In Palladium, we don't hope our code works—we know it does."*

**Ready to build the future? [Get Started →](guides/getting-started.md)**