#!/bin/bash
# Repository reorganization script for Palladium
# This script reorganizes the project structure for better clarity

echo "=== Palladium Repository Reorganization ==="
echo "This will reorganize the repository structure."
echo "A backup will be created first."
echo

# Create timestamp for backup
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_DIR="../palladium-backup-$TIMESTAMP"

# Function to create directory if it doesn't exist
create_dir() {
    if [ ! -d "$1" ]; then
        mkdir -p "$1"
        echo "Created: $1"
    fi
}

# Create new directory structure
echo "Step 1: Creating new directory structure..."

# Main directories
create_dir "compiler/rust"
create_dir "compiler/palladium"
create_dir "compiler/bootstrap"
create_dir "benchmarks"
create_dir "tools"

# Compiler subdirectories
create_dir "compiler/rust/src"
create_dir "compiler/rust/tests"
create_dir "compiler/palladium/src"
create_dir "compiler/palladium/tests"
create_dir "compiler/bootstrap/v1_archive"
create_dir "compiler/bootstrap/v2_full"
create_dir "compiler/bootstrap/v3_incremental"

# Move files to new structure
echo
echo "Step 2: Moving files to new locations..."

# Move Rust compiler
if [ -d "src" ]; then
    echo "Moving Rust compiler source..."
    cp -r src/* compiler/rust/src/ 2>/dev/null
fi

# Move Palladium self-hosting compiler
if [ -d "src_pd" ]; then
    echo "Moving Palladium self-hosting compiler..."
    cp -r src_pd/* compiler/palladium/src/ 2>/dev/null
    if [ -d "src_pd/tests" ]; then
        cp -r src_pd/tests/* compiler/palladium/tests/ 2>/dev/null
    fi
fi

# Move bootstrap compilers
if [ -d "bootstrap" ]; then
    echo "Moving bootstrap compilers..."
    if [ -d "bootstrap/v1_archive" ]; then
        cp -r bootstrap/v1_archive/* compiler/bootstrap/v1_archive/ 2>/dev/null
    fi
    if [ -d "bootstrap/v2_full_compiler" ]; then
        cp -r bootstrap/v2_full_compiler/* compiler/bootstrap/v2_full/ 2>/dev/null
    fi
    if [ -d "bootstrap/v3_incremental" ]; then
        cp -r bootstrap/v3_incremental/* compiler/bootstrap/v3_incremental/ 2>/dev/null
    fi
fi

# Move benchmarks
echo "Creating benchmark structure..."
create_dir "benchmarks/palladium"
create_dir "benchmarks/c"
create_dir "benchmarks/rust"

# Move tools
if [ -d "scripts/tools" ]; then
    echo "Moving tools..."
    cp -r scripts/tools/* tools/ 2>/dev/null
fi

# Clean up build outputs
echo
echo "Step 3: Consolidating build outputs..."
create_dir "build"
create_dir "build/cache"
create_dir "build/output"

# Move all build outputs to consolidated location
if [ -d "build_output" ]; then
    cp -r build_output/* build/output/ 2>/dev/null
fi
if [ -d "test_output" ]; then
    cp -r test_output/* build/output/ 2>/dev/null
fi

# Update documentation
echo
echo "Step 4: Updating documentation references..."

# Create main project documentation
cat > ARCHITECTURE.md << 'EOF'
# Palladium Architecture

## Directory Structure

```
palladium/
├── compiler/           # All compiler implementations
│   ├── rust/          # Rust implementation (main compiler)
│   ├── palladium/     # Self-hosting Palladium compiler
│   └── bootstrap/     # Bootstrap compilers
├── stdlib/            # Standard library
├── examples/          # Example programs
├── tests/            # Test suite
├── benchmarks/       # Performance benchmarks
├── tools/           # Development tools
├── docs/            # Documentation
└── build/           # Build outputs (git-ignored)
```

## Compiler Implementations

1. **Rust Compiler** (`compiler/rust/`)
   - Production compiler written in Rust
   - Full feature support
   - ~19K lines of code

2. **Self-Hosting Compiler** (`compiler/palladium/`)
   - Compiler written in Palladium
   - ~6K lines of Palladium code
   - Proves self-hosting capability

3. **Bootstrap Compilers** (`compiler/bootstrap/`)
   - Historical bootstrap implementations
   - Demonstrates incremental development
   - Minimal compilers for bootstrapping
EOF

echo
echo "=== Reorganization Plan Complete ==="
echo
echo "This script shows the reorganization plan. To execute:"
echo "1. Review the changes"
echo "2. Run with --execute flag to perform the reorganization"
echo
echo "Note: The script currently runs in dry-run mode."
echo "Add --execute to actually move files."