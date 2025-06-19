# Palladium Compiler Implementations

This directory contains all compiler implementations for the Palladium programming language.

## Directory Structure

```
compiler/
├── rust/          # Production compiler written in Rust
├── palladium/     # Self-hosting compiler written in Palladium
└── bootstrap/     # Bootstrap compilers for self-hosting
```

## Implementations

### 1. Rust Compiler (`rust/`)
- **Status**: Production-ready
- **Language**: Rust
- **Features**: Full language support
- **Size**: ~19,384 lines of code
- **Purpose**: Main compiler for Palladium development

### 2. Self-Hosting Compiler (`palladium/`)
- **Status**: Working prototype
- **Language**: Palladium
- **Features**: Core language features
- **Size**: ~5,947 lines of code
- **Purpose**: Demonstrates self-hosting capability

### 3. Bootstrap Compilers (`bootstrap/`)
- **Status**: Historical/Educational
- **Language**: Palladium
- **Versions**: 
  - v1: Early attempts (archived)
  - v2: Full compiler approach (1,220 lines)
  - v3: Incremental approach (760 lines final)
- **Purpose**: Shows evolution of self-hosting

## Building

Each compiler has its own build instructions:

- **Rust Compiler**: `cargo build --release`
- **Palladium Compiler**: Use bootstrap compiler to build
- **Bootstrap**: Self-contained, compile with minimal compiler

## Usage

```bash
# Use Rust compiler (recommended)
compiler/rust/target/release/palladium input.pd -o output

# Use self-hosting compiler
compiler/palladium/pdc input.pd -o output.c

# Use bootstrap compiler
compiler/bootstrap/v3/tiny_v16 < input.pd > output.c
```