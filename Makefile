# Palladium Build System
# Makefile for building, testing, and managing the Palladium compiler

# Default target
.DEFAULT_GOAL := help

# Variables
CARGO := cargo
PDC := ./compiler/rust/target/release/pdc
BOOTSTRAP_DIR := compiler/bootstrap/v3_incremental
TINY_COMPILER := $(BOOTSTRAP_DIR)/tiny_compiler
PALLADIUM_COMPILER := compiler/palladium/pdc

# Colors for output
GREEN := \033[0;32m
YELLOW := \033[0;33m
RED := \033[0;31m
NC := \033[0m # No Color

.PHONY: help
help: ## Show this help message
	@echo "Palladium Build System"
	@echo "====================="
	@echo ""
	@echo "Available targets:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  $(GREEN)%-20s$(NC) %s\n", $$1, $$2}'

.PHONY: build
build: ## Build the Rust compiler in release mode
	@echo "$(YELLOW)Building Palladium compiler...$(NC)"
	cd compiler/rust && $(CARGO) build --release
	@echo "$(GREEN)✓ Build complete$(NC)"

.PHONY: build-debug
build-debug: ## Build the Rust compiler in debug mode
	@echo "$(YELLOW)Building Palladium compiler (debug)...$(NC)"
	$(CARGO) build
	@echo "$(GREEN)✓ Debug build complete$(NC)"

.PHONY: test
test: ## Run all tests
	@echo "$(YELLOW)Running tests...$(NC)"
	$(CARGO) test --all
	@echo "$(GREEN)✓ All tests passed$(NC)"

.PHONY: test-palladium
test-palladium: build ## Run Palladium test suite
	@echo "$(YELLOW)Running Palladium test suite...$(NC)"
	./scripts/run_tests.sh

.PHONY: test-all
test-all: test test-palladium ## Run all tests (Rust + Palladium)

.PHONY: test-examples
test-examples: build ## Test all example programs
	@echo "$(YELLOW)Testing example programs...$(NC)"
	./scripts/run_tests.sh -f "examples"

.PHONY: test-bootstrap
test-bootstrap: build ## Test bootstrap compilers
	@echo "$(YELLOW)Testing bootstrap compilers...$(NC)"
	./scripts/run_tests.sh -f "bootstrap"

.PHONY: test-verbose
test-verbose: build ## Run tests with verbose output
	./scripts/run_tests.sh -v

.PHONY: bench
bench: build ## Run all benchmarks
	@echo "$(YELLOW)Running benchmarks...$(NC)"
	cd benchmarks && ./run_benchmarks.sh

.PHONY: bench-quick
bench-quick: build ## Run quick benchmarks only
	@echo "$(YELLOW)Running quick benchmarks...$(NC)"
	cd benchmarks && ./run_benchmarks.sh fibonacci bubble_sort

.PHONY: bench-analyze
bench-analyze: ## Analyze benchmark results
	@echo "$(YELLOW)Analyzing benchmark results...$(NC)"
	cd benchmarks && python3 analyze_results.py

.PHONY: lint
lint: ## Run clippy linter
	@echo "$(YELLOW)Running clippy...$(NC)"
	$(CARGO) clippy --all-targets --all-features -- -D warnings
	@echo "$(GREEN)✓ No lint errors$(NC)"

.PHONY: fmt
fmt: ## Format code with rustfmt
	@echo "$(YELLOW)Formatting code...$(NC)"
	$(CARGO) fmt
	@echo "$(GREEN)✓ Code formatted$(NC)"

.PHONY: check
check: ## Run cargo check
	@echo "$(YELLOW)Checking code...$(NC)"
	$(CARGO) check --all
	@echo "$(GREEN)✓ Check complete$(NC)"

.PHONY: clean
clean: ## Clean build artifacts
	@echo "$(YELLOW)Cleaning...$(NC)"
	$(CARGO) clean
	rm -rf build_output/*
	rm -rf archive/build_outputs/*
	rm -rf bootstrap/*/build_output/*
	@echo "$(GREEN)✓ Clean complete$(NC)"

.PHONY: all
all: clean build test lint ## Clean, build, test, and lint

# Bootstrap targets
.PHONY: bootstrap
bootstrap: build ## Build the tiny bootstrap compiler
	@echo "$(YELLOW)Building bootstrap compiler...$(NC)"
	cd $(BOOTSTRAP_DIR) && ./build_minimal.sh
	@echo "$(GREEN)✓ Bootstrap compiler built$(NC)"

.PHONY: bootstrap-test
bootstrap-test: bootstrap ## Test the bootstrap compiler
	@echo "$(YELLOW)Testing bootstrap compiler...$(NC)"
	cd $(BOOTSTRAP_DIR) && ./test_tiny.sh
	@echo "$(GREEN)✓ Bootstrap test complete$(NC)"

.PHONY: self-host
self-host: build ## Demonstrate self-hosting capability
	@echo "$(YELLOW)Testing self-hosting...$(NC)"
	$(PDC) compile bootstrap/v3_incremental/minimal_self_compiler.pd -o build_output/self_compiler
	./build_output/self_compiler
	@echo "$(GREEN)✓ Self-hosting verified$(NC)"

# Example compilation targets
.PHONY: example-hello
example-hello: build ## Compile and run hello world example
	@echo "$(YELLOW)Compiling hello world...$(NC)"
	$(PDC) compile examples/basic/hello.pd -o build_output/hello
	@echo "$(YELLOW)Running hello world...$(NC)"
	./build_output/hello
	@echo "$(GREEN)✓ Hello world complete$(NC)"

.PHONY: compile-pd
compile-pd: build ## Compile a Palladium file (use with FILE=...)
	@if [ -z "$(FILE)" ]; then \
		echo "Usage: make compile-pd FILE=<filename.pd>"; \
		exit 1; \
	fi
	@echo "$(YELLOW)Compiling $(FILE)...$(NC)"
	$(PDC) compile $(FILE) -o build_output/$$(basename $(FILE) .pd)
	@echo "$(GREEN)✓ Compilation complete$(NC)"

# Documentation targets
.PHONY: docs
docs: ## Generate documentation
	@echo "$(YELLOW)Generating documentation...$(NC)"
	$(CARGO) doc --no-deps
	@echo "$(GREEN)✓ Documentation generated$(NC)"

.PHONY: docs-open
docs-open: docs ## Generate and open documentation
	$(CARGO) doc --no-deps --open

# Development helpers
.PHONY: dev-build
dev-build: ## Build in debug mode
	@echo "$(YELLOW)Building (debug)...$(NC)"
	$(CARGO) build
	@echo "$(GREEN)✓ Debug build complete$(NC)"

.PHONY: dev-test
dev-test: ## Run tests with output
	@echo "$(YELLOW)Running tests (verbose)...$(NC)"
	$(CARGO) test -- --nocapture

# Quick commands (shortcuts)
.PHONY: b
b: build ## Alias for build

.PHONY: t
t: test-all ## Alias for test-all

.PHONY: c
c: check ## Alias for check

.PHONY: l
l: lint ## Alias for lint

# CI/CD commands
.PHONY: ci
ci: check fmt lint test test-palladium ## Run all CI checks

# Project info
.PHONY: stats
stats: ## Show project statistics
	@echo "$(YELLOW)Project Statistics:$(NC)"
	@echo "Lines of Rust code:"
	@find src -name "*.rs" | xargs wc -l | tail -1
	@echo ""
	@echo "Lines of Palladium code:"
	@find . -name "*.pd" | xargs wc -l | tail -1
	@echo ""
	@echo "Number of tests:"
	@grep -r "fn test_" src tests | wc -l

.PHONY: todo
todo: ## Show all TODO items in the codebase
	@echo "$(YELLOW)TODO items:$(NC)"
	@grep -r "TODO" src --color=always | head -20 || echo "No TODOs found!"
	@echo ""
	@echo "$(YELLOW)Total TODO count:$(NC)"
	@grep -r "TODO" src | wc -l