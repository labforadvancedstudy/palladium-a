.PHONY: all build test clean bootstrap docs help

# Default target
all: build

# Build the Rust compiler
build:
	@echo "Building Palladium compiler..."
	@cargo build --release

# Run all tests
test:
	@echo "Running tests..."
	@cargo test
	@echo "Running example programs..."
	@./scripts/test_bootstrap.sh

# Clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	@cargo clean
	@rm -rf build_output/*
	@rm -rf archive/build_outputs/*
	@rm -rf bootstrap/*/build_output/*

# Build bootstrap compiler
bootstrap:
	@echo "Building bootstrap compiler..."
	@cd bootstrap/v3_incremental && ./build_minimal.sh

# Generate documentation
docs:
	@echo "Generating documentation..."
	@cargo doc --no-deps --open

# Show help
help:
	@echo "Palladium Build System"
	@echo "====================="
	@echo ""
	@echo "Available targets:"
	@echo "  make          - Build the compiler (default)"
	@echo "  make test     - Run all tests"
	@echo "  make clean    - Clean build artifacts"
	@echo "  make bootstrap - Build bootstrap compiler"
	@echo "  make docs     - Generate documentation"
	@echo "  make help     - Show this help message"

# Quick compile shortcut
compile-pd:
	@if [ -z "$(FILE)" ]; then \
		echo "Usage: make compile-pd FILE=<filename.pd>"; \
		exit 1; \
	fi
	@./target/release/pdc compile $(FILE)

# Development shortcuts
dev-build:
	@cargo build

dev-test:
	@cargo test -- --nocapture

# Check code without building
check:
	@cargo check

# Format code
fmt:
	@cargo fmt

# Run linter
lint:
	@cargo clippy -- -D warnings