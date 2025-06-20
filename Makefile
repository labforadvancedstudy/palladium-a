# Palladium Build System
# Makefile for building, testing, and managing the Palladium compiler

# Default target
.DEFAULT_GOAL := help

# Variables
CARGO := cargo
PDC := ./target/release/pdc
BOOTSTRAP_DIR := bootstrap/v3_incremental
TINY_COMPILER := $(BOOTSTRAP_DIR)/tiny_v16
PALLADIUM_COMPILER := bootstrap/v2_full/pdc
TEST_DIR := tests
EXAMPLES_DIR := examples

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
	$(CARGO) build --release
	@echo "$(GREEN)✓ Build complete$(NC)"

.PHONY: build-debug
build-debug: ## Build the Rust compiler in debug mode
	@echo "$(YELLOW)Building Palladium compiler (debug)...$(NC)"
	$(CARGO) build
	@echo "$(GREEN)✓ Debug build complete$(NC)"

.PHONY: test
test: test-rust test-pd ## Run all tests (Rust + Palladium)

.PHONY: test-rust
test-rust: ## Run Rust unit tests
	@echo "$(YELLOW)Running Rust unit tests...$(NC)"
	$(CARGO) test --lib --bins
	@echo "$(GREEN)✓ Rust tests passed$(NC)"

.PHONY: test-pd
test-pd: build ## Run Palladium language tests
	@echo "$(YELLOW)Running Palladium language tests...$(NC)"
	@cd $(TEST_DIR) && bash run_all_tests.sh
	@echo "$(GREEN)✓ Palladium tests passed$(NC)"

.PHONY: test-integration
test-integration: build ## Run integration tests
	@echo "$(YELLOW)Running integration tests...$(NC)"
	$(CARGO) test --test '*' -- --test-threads=1
	@echo "$(GREEN)✓ Integration tests passed$(NC)"

.PHONY: test-all
test-all: test-rust test-pd test-examples ## Run all tests (Rust + Palladium + Examples)

.PHONY: test-examples
test-examples: build ## Test all example programs
	@echo "$(YELLOW)Testing example programs...$(NC)"
	@for dir in $(EXAMPLES_DIR)/tutorial $(EXAMPLES_DIR)/practical; do \
		echo "Testing $$dir..."; \
		for file in $$dir/*.pd; do \
			if [ -f "$$file" ]; then \
				echo -n "  Testing $$(basename $$file)... "; \
				$(PDC) compile "$$file" -o /tmp/test_output 2>/dev/null && echo "$(GREEN)✓$(NC)" || echo "$(RED)✗$(NC)"; \
			fi; \
		done; \
	done

.PHONY: test-bootstrap
test-bootstrap: build ## Test bootstrap compilers
	@echo "$(YELLOW)Testing bootstrap compilers...$(NC)"
	@cd bootstrap/minimal_self_host && bash test_self_host.sh

.PHONY: test-verbose
test-verbose: ## Run tests with verbose output
	@echo "$(YELLOW)Running verbose tests...$(NC)"
	RUST_BACKTRACE=1 $(CARGO) test -- --nocapture

.PHONY: bench
bench: ## Run Rust benchmarks
	@echo "$(YELLOW)Running benchmarks...$(NC)"
	$(CARGO) bench

.PHONY: coverage
coverage: ## Generate test coverage report
	@echo "$(YELLOW)Generating coverage report...$(NC)"
	@bash scripts/pd_coverage.sh
	@echo "$(GREEN)✓ Coverage report generated$(NC)"

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
bootstrap: ## Build and test bootstrap compiler
	@echo "$(YELLOW)Building bootstrap compiler...$(NC)"
	@cd bootstrap/minimal_self_host && bash build_self_host.sh
	@echo "$(GREEN)✓ Bootstrap compiler built$(NC)"

.PHONY: bootstrap-test
bootstrap-test: ## Test the bootstrap compiler
	@echo "$(YELLOW)Testing bootstrap compiler...$(NC)"
	@cd bootstrap/minimal_self_host && bash test_self_host.sh
	@echo "$(GREEN)✓ Bootstrap test complete$(NC)"

# Example compilation targets
.PHONY: example-hello
example-hello: build ## Compile and run hello world example
	@echo "$(YELLOW)Compiling hello world...$(NC)"
	@mkdir -p build_output
	$(PDC) compile examples/tutorial/01_hello_world.pd -o build_output/hello
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
	@mkdir -p build_output
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
.PHONY: dev
dev: ## Watch for changes and rebuild
	@echo "$(YELLOW)Starting development mode...$(NC)"
	$(CARGO) watch -x build -x test

.PHONY: dev-test
dev-test: ## Run a specific test (use with TEST=test_name)
	@echo "$(YELLOW)Running tests...$(NC)"
	$(CARGO) test $(TEST) -- --nocapture

# Quick commands (shortcuts)
.PHONY: b
b: build ## Alias for build

.PHONY: t
t: test ## Alias for test

.PHONY: c
c: check ## Alias for check

.PHONY: l
l: lint ## Alias for lint

.PHONY: r
r: build ## Alias for run (build and run hello world)
	@make example-hello

# CI/CD commands
.PHONY: ci
ci: check fmt lint test ## Run all CI checks

.PHONY: ci-full
ci-full: check fmt lint test-all coverage ## Run full CI checks with coverage

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
	@echo "Number of Rust tests:"
	@grep -r "#\[test\]" src | wc -l
	@echo ""
	@echo "Number of Palladium tests:"
	@ls -1 tests/*.pd | wc -l

.PHONY: todo
todo: ## Show all TODO items in the codebase
	@echo "$(YELLOW)TODO items:$(NC)"
	@rg -i "todo|fixme|hack|xxx" src --color=always | head -20 || echo "No TODOs found!"
	@echo ""
	@echo "$(YELLOW)Total TODO count:$(NC)"
	@rg -i "todo|fixme" src | wc -l

# Package management
.PHONY: publish
publish: ci ## Publish to crates.io
	@echo "$(YELLOW)Publishing to crates.io...$(NC)"
	$(CARGO) publish
	@echo "$(GREEN)✓ Published successfully$(NC)"

.PHONY: package
package: ## Create distributable package
	@echo "$(YELLOW)Creating package...$(NC)"
	$(CARGO) package
	@echo "$(GREEN)✓ Package created$(NC)"

# Installation
.PHONY: install
install: build ## Install pdc locally
	@echo "$(YELLOW)Installing pdc...$(NC)"
	$(CARGO) install --path .
	@echo "$(GREEN)✓ pdc installed$(NC)"

.PHONY: uninstall
uninstall: ## Uninstall pdc
	@echo "$(YELLOW)Uninstalling pdc...$(NC)"
	$(CARGO) uninstall alan-von-palladium
	@echo "$(GREEN)✓ pdc uninstalled$(NC)"