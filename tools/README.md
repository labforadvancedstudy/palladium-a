# Palladium Development Tools

Development tools for the Palladium programming language.

## Tools

### pdfmt - Code Formatter
Formats Palladium code according to the official style guide.

```bash
pdfmt input.pd              # Format single file
pdfmt src/                  # Format directory
pdfmt --check input.pd      # Check formatting
```

### pdlint - Linter
Static analysis tool for finding common issues.

```bash
pdlint input.pd             # Lint single file
pdlint --fix input.pd       # Auto-fix issues
pdlint --explain E001       # Explain error code
```

### pddoc - Documentation Generator
Generates documentation from Palladium source code.

```bash
pddoc src/                  # Generate docs for directory
pddoc --output docs/ src/   # Specify output directory
pddoc --markdown src/       # Generate markdown docs
```

### pd-lsp - Language Server
Language Server Protocol implementation for IDE support.

```bash
pd-lsp                      # Start language server
```

## Installation

```bash
# Install all tools
make install-tools

# Install specific tool
make install-pdfmt
```

## Development

Each tool is written in Rust for performance and reliability.

### Building from source

```bash
cd tools/pdfmt
cargo build --release
```

## Roadmap

- [x] Basic project structure
- [ ] pdfmt implementation
- [ ] pdlint implementation  
- [ ] pddoc implementation
- [ ] pd-lsp implementation
- [ ] Package manager (pdpm)