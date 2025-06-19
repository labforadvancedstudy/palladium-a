# Palladium Language Documentation

## Quick Start

Palladium is a systems programming language that combines Rust's safety with improved ergonomics and powerful verification capabilities.

### View Implementation Status
- [Interactive Dashboard](https://palladium-lang.org/status) (coming soon)
- [Status YAML](./features/status.yaml) - Current implementation progress
- [Feature Docs](./features/) - Detailed documentation for each feature

## Key Innovations

### 1. **Implicit Lifetimes** (80% complete)
No more lifetime annotations for 90% of cases
```palladium
fn longest(x: ref str, y: ref str) -> ref str {
    if x.len() > y.len() { x } else { y }
}
```

### 2. **Async as Effect** (40% complete)  
No function coloring, no `.await` spam
```palladium
fn fetch_all(ids: Vec<u64>) -> Vec<User> {
    ids.map(fetch_user).collect()  // Parallel by default!
}
```

### 3. **Totality Checking** (30% complete)
Prove your functions terminate
```palladium
#[total]
fn factorial(n: u64) -> u64 {
    if n == 0 { 1 } else { n * factorial(n - 1) }
}
```

## Documentation Structure

```
docs/
├── features/           # Individual feature documentation
│   ├── status.yaml    # Implementation progress tracker
│   ├── *.md          # Feature specifications
├── guides/            # User guides and tutorials
├── reference/         # Language reference
└── tools/            # Tooling documentation
```

## Contributing

When implementing a new feature:

1. Update `features/status.yaml` with progress
2. Create/update `features/[feature].md` with:
   - Current status
   - Code comparisons (Rust/Go/Palladium)
   - Rationale and design decisions
   - Implementation notes

3. Run `palladium-docs verify` to ensure consistency

## Development Roadmap

### Phase 1: Core Language (Current)
- ✅ Basic type system (90%)
- ✅ Borrow checker (95%)
- ⏳ Implicit lifetimes (80%)
- ⏳ Bootstrap compiler (60%)

### Phase 2: Advanced Features
- ⏳ Async as effect (40%)
- ⏳ Totality checking (30%)
- 🔲 Refinement types (5%)
- 🔲 Proof generation (0%)

### Phase 3: Ecosystem
- ⏳ Standard library (40%)
- 🔲 Package manager (0%)
- 🔲 IDE support (10%)

## Quick Links

- [Why Palladium?](./guides/why-palladium.md)
- [Migration from Rust](./guides/migration-from-rust.md)
- [Language Reference](./reference/index.md)
- [Compiler Internals](./internals/compiler.md)

## Status Legend

- ✅ Complete (80-100%)
- ⏳ In Progress (20-79%)
- 🔲 Planned (0-19%)

For real-time progress updates, check [status.yaml](./features/status.yaml).