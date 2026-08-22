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
# --- Language conformance and self-hosting gates ---------------------------

.PHONY: conformance
conformance: build ## Compile+link+run every .pd under tests/ and examples/, against tests/conformance-manifest.txt
	@bash scripts/conformance.sh tests examples

.PHONY: test-conformance-runner
test-conformance-runner: build ## Prove the conformance gate still goes RED when it should
	@bash scripts/test-conformance-runner.sh

# M1's exit criterion, as a command.
#
# THERE ARE THREE INVENTORIES IN THIS REPO, AND THIS USED TO READ ONE.
#   tests/conformance-manifest.txt          owns .pd fixtures (`owner` column)
#   tests/rust-debt-manifest.txt            owns Rust tests, cross-checked
#                                           against #[ignore = "… (owned by M<n>)"]
#   the ordinary Rust suite                 owns everything not ignored
# The first two use the same `(owned by M<n>)` grammar and both are enforceable,
# but this target only ran the first. Measured at 2ef170f, the v0.3.0 release
# commit: it exited 0 while THREE M1-owned #[ignore] rows were still failing,
# one of them the tail-`if` miscompile M1 was named for — `fib(10)` printed
# 8261746944 and exited 0. The release that exists to remove silent miscompiles
# shipped one, and its own exit criterion could not see it.
#
# WHY THE ORDINARY SUITE IS HERE TOO (inventory 3). The Rust debt manifest is a
# closed inventory of tests that are ALLOWED to fail. `paid` says a row's test
# is no longer #[ignore]d — it does not say the test passes, and asking
# scripts/test-xfail.py to prove that would mean running the suite anyway. So
# the suite runs, once, here: a row transitioned to `paid` over a test that
# still fails is red in this step. Without it, `paid` would be a way to retire a
# failing test by editing one word.
#
# All three run even when an earlier one is red: stopping at the first failure
# reports part of the debt and costs a round trip to discover the rest.
.PHONY: m1-exit
m1-exit: build ## M1's exit criterion: nothing in ANY inventory still owed to M1
	@rc=0; \
	echo "$(YELLOW)== inventory 1 of 3: .pd fixtures (tests/conformance-manifest.txt) ==$(NC)"; \
	CONFORMANCE_FORBID_OWNER=M1 bash scripts/conformance.sh tests examples || rc=1; \
	echo; \
	echo "$(YELLOW)== inventory 2 of 3: Rust debt (tests/rust-debt-manifest.txt + #[ignore] reasons) ==$(NC)"; \
	TEST_XFAIL_FORBID_OWNER=M1 python3 scripts/test-xfail.py || rc=1; \
	echo; \
	echo "$(YELLOW)== inventory 3 of 3: the ordinary Rust suite (nothing here is allowed to fail) ==$(NC)"; \
	$(CARGO) test --release --no-fail-fast || rc=1; \
	echo; \
	if [ $$rc -eq 0 ]; then \
	  echo "$(GREEN)✓ M1 exit criterion met — nothing in any inventory is owed to M1$(NC)"; \
	else \
	  echo "$(RED)✗ M1 is NOT finished — see the OWED_TO_M1 / failure line(s) above$(NC)"; \
	fi; \
	exit $$rc

.PHONY: selfhost
selfhost: build ## Run the self-hosting fixed-point gate (bootstrap/pdc.pd)
	@bash scripts/selfhost.sh

.PHONY: test-honest
test-honest: ## Run EVERY test binary, including integration tests (no fail-fast)
	@echo "$(YELLOW)Running all test binaries (integration included)...$(NC)"
	@echo "$(YELLOW)Note: 'make test-rust' uses --lib --bins and never runs tests/*.rs,$(NC)"
	@echo "$(YELLOW)which is why the integration failures below went unnoticed.$(NC)"
	$(CARGO) test --release --no-fail-fast

.PHONY: check-docs
check-docs: build check-doc-evidence ## Compile every ```palladium block in the documentation
	@bash scripts/check-docs.sh docs README.md

.PHONY: check-doc-evidence
check-doc-evidence: ## Pin doc citations, the no-compile allowlist and the evidence tags
	@bash scripts/check-doc-evidence.sh

# stdlib/ = library modules with no `fn main`; the only pinnable thing is a
# compile verdict plus its blocker. tests/stdlib/*.pd are ordinary conformance
# fixtures and are run + transcript-diffed by `make conformance`, NOT here.
.PHONY: stdlib-gate
stdlib-gate: build ## Pin the stdlib/ measurement, account builtins, check generated C
	@bash scripts/stdlib-gate.sh

.PHONY: test-gate-probe
test-gate-probe: build ## Fault-inject every producer the stdlib gate reads as evidence
	@bash scripts/test-gate-probe.sh

.PHONY: gates
gates: conformance test-conformance-runner check-docs selfhost stdlib-gate test-gate-probe ## Run every language-level gate
	@echo "$(GREEN)✓ all gates green$(NC)"

# --- Expected failures in the Rust test suite ------------------------------
# Appended at the end deliberately: the gates section above is being edited in
# parallel.

.PHONY: test-xfail
test-xfail: ## Run the #[ignore]d tests and fail if a declared failure now passes
	@python3 scripts/test-xfail.py
