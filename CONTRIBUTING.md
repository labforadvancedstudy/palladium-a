# Contributing to Alan von Palladium

First off, thank you for considering contributing to Palladium! It's people like you who will make Palladium the language where Turing's proofs meet von Neumann's performance.

## Code of Conduct

We believe in:
- **Respectful discourse** - Attack ideas, not people
- **Constructive feedback** - "This could be better if..." not "This sucks"
- **Inclusive community** - All backgrounds welcome, from theorem provers to bit twiddlers

## How Can I Contribute?

### 🐛 Reporting Bugs

Before creating bug reports, please check existing issues. When creating a bug report, include:

1. **Clear title and description**
2. **Steps to reproduce**
3. **Expected behavior**
4. **Actual behavior**
5. **Code samples** (minimal reproducible example)
6. **Environment details** (OS, Palladium version)

### 💡 Suggesting Enhancements

Enhancement suggestions are tracked as GitHub issues. Include:

1. **Use case** - Why is this needed?
2. **Proposed solution** - How should it work?
3. **Alternatives considered** - What else did you think about?
4. **Examples** - Show how it would be used

### 🔧 Pull Requests

1. **Fork the repo** and create your branch from `main`
2. **Write clear commit messages** following conventional commits
3. **Add tests** for new functionality
4. **Update documentation** as needed
5. **Ensure all tests pass** with `cargo test`
6. **Run the formatter** with `cargo fmt`
7. **Check lints** with `cargo clippy`

## Development Setup

```bash
# Clone your fork
git clone https://github.com/YOUR_USERNAME/palladium-a.git
cd palladium-a

# Add upstream remote
git remote add upstream https://github.com/labforadvancedstudy/palladium-a.git

# Create a feature branch
git checkout -b feature/amazing-feature

# Make your changes and test
cargo test
cargo run -- compile examples/basic/hello.pd

# Commit your changes
git commit -m "feat: add amazing feature"

# Push to your fork
git push origin feature/amazing-feature
```

## Project Structure

```
palladium-a/
├── src/           # Compiler source code
│   ├── lexer/     # Tokenization
│   ├── parser/    # AST generation
│   ├── typeck/    # Type checking
│   ├── codegen/   # Code generation
│   └── driver/    # Compilation driver
├── examples/      # Example programs
├── tests/         # Integration tests
└── docs/          # Documentation
```

## Testing

- **Unit tests**: In relevant source files
- **Integration tests**: In `tests/` directory
- **Example programs**: In `examples/` directory

Run all tests:
```bash
cargo test
```

Test specific module:
```bash
cargo test parser::
```

## Documentation

- **Code comments**: Explain "why", not "what"
- **Doc comments**: Use `///` for public items
- **Examples**: Include in doc comments when helpful
- **README updates**: Keep in sync with features

## Commit Messages

Follow conventional commits:
- `feat:` New feature
- `fix:` Bug fix
- `docs:` Documentation only
- `test:` Test additions/changes
- `refactor:` Code refactoring
- `style:` Formatting changes
- `chore:` Maintenance tasks

Examples:
```
feat: implement for loops with range syntax
fix: correct type inference for struct returns
docs: update examples README with new structure
test: add parser tests for struct returns
```

## Areas Needing Help

### 🌟 High Priority
- **Type system improvements** - Making inference smarter
- **Error messages** - Making them more helpful
- **Standard library** - Vec, HashMap, iterators
- **Self-hosting** - Compiler in Palladium

### 📚 Documentation
- **Tutorial series** - Teaching Palladium step by step
- **Language reference** - Formal specification
- **Example programs** - Showing off features

### 🔬 Research
- **Formal verification** - Proof system integration
- **Optimization** - Making it even faster
- **Memory models** - Alternative GC strategies

## Philosophy

When contributing, remember our core principles:
1. **Correctness first** - If it's not right, it's not done
2. **Performance matters** - Every cycle counts
3. **Developer experience** - Make it joy to use

## Questions?

- **Discord**: Join our community server
- **GitHub Discussions**: For longer form conversations
- **Issues**: For specific bugs or features

## Recognition

Contributors are recognized in:
- Release notes
- Contributors file
- Special thanks in README

## License

By contributing, you agree that your contributions will be licensed under the MIT License.

---

*"The best way to predict the future is to implement it"* - Alan von Palladium team