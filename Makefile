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
# THERE ARE FOUR INVENTORIES IN THIS REPO, AND THIS USED TO READ ONE.
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
# AND INVENTORY FOUR IS HERE NOW (GI-08). `docs/contributing/1.0-requirements.tsv`
# is the inventory that enumerates what is OWED rather than what has been
# observed to break; the other three are registers of DECLARED failures, so a
# requirement nobody has started on leaves all three clean. GI-08 is one
# sentence — "Every milestone exit reads BOTH debt inventories and this
# manifest" — and this target did not, which is why the row said `owed`.
#
# IT WAS LEFT OUT FOR A MEASURED REASON AND THE REASON DID NOT SURVIVE. The
# manifest has ZERO rows owned by M1 (`awk -F'\t' '$$2=="M1"'` prints nothing),
# so `REQ_MILESTONE=M1 python3 scripts/requirements.py` abstains — exit 2,
# NO_VERDICT — and appending it with `|| rc=1` would have reddened a legitimately
# green target for a reason that says nothing about M1. That is an argument
# against `|| rc=1`, not against reading the inventory: "do not consult it" and
# "consult it and know what its abstention means" are different answers, and only
# the second can tell that abstention apart from an unreadable manifest, which
# exits 2 as well. scripts/m1-exit.sh maps the two shapes apart by the sentence
# each one prints, tolerates the first with the sentence REPRINTED, and fails
# closed on everything else. Both shapes were probed before the mapping was
# written, and its self-test regenerates them live.
#
# THE AGGREGATION MOVED INTO A SCRIPT FOR THE REASON m2-exit's DID. This recipe
# was `|| rc=1` per inventory, which is two-valued, and Make folds every nonzero
# recipe status to 2 on the way out; the comment below m2-exit used to record
# that collapse as a live residual belonging to whoever owns M1's ledger. The
# mapping above needs a state that means "would not measure", and folding it onto
# OWED would report an abstention as a measurement. So m1-exit now carries the
# same three values, the same lattice and the same `<M>_EXIT_RESULT` last line as
# scripts/m2-exit.sh. Widening, not breaking: 0 still means the same thing and
# every previous 1 is still nonzero.
#
# All four run even when an earlier one is red: stopping at the first failure
# reports part of the debt and costs a round trip to discover the rest.
.PHONY: m1-exit
m1-exit: build ## M1's exit criterion: nothing in ANY inventory still owed to M1
	@bash scripts/m1-exit.sh

# M2's exit criterion, as a command — GI-08. THIS TARGET DID NOT EXIST, and
# docs/contributing/MILESTONES.md named it as M2's Exit line, so the milestone
# had no exit criterion at all. That is the same hole M1 shipped through: v0.3.0
# was released under M1's name while `make m1-exit` was RED, and the reason
# nobody saw it is that a milestone whose exit criterion is a sentence rather
# than a command is measured by whoever is reading the sentence.
#
# It ships FIRST, before the rest of M2, for that reason and only that reason.
#
# IT IS RED TODAY AND THAT IS THE CORRECT STATE. A green `m2-exit` on a branch
# that has not done M2 would be the defect, not the achievement.
#
# FOUR INVENTORIES, THE SAME FOUR `m1-exit` READS. The first three are
# m1-exit's, character for character, with the owner changed:
#   tests/conformance-manifest.txt          owns .pd fixtures (`owner` column)
#   tests/rust-debt-manifest.txt            owns Rust tests, cross-checked
#                                           against #[ignore = "… (owned by M<n>)"]
#   the ordinary Rust suite                 owns everything not ignored
# and every one of them is a register of DECLARED FAILURES. That is the hole
# GI-08 names: a declared failure is a proxy that exists only where somebody
# already wrote a red test, so a requirement nobody has started on leaves all
# three clean. `docs/contributing/1.0-requirements.tsv` is the inventory that
# enumerates what is OWED rather than what has been observed to break, and
# GI-08 is one sentence — "Every milestone exit reads BOTH debt inventories and
# this manifest". Inventory four is that clause.
#
# `m1-exit` GETS INVENTORY FOUR TOO, as of GI-08, and the reason it did not for
# a while is worth keeping: the manifest has ZERO rows owned by M1 (measured:
# `awk -F'\t' '$$2=="M1"'` prints nothing), so the requirement gate abstains —
# NO_VERDICT, nonzero — and a `|| rc=1` aggregation would have turned a target
# that is legitimately green RED for a reason that says nothing about M1. The
# absence of M1 rows is still worth knowing and is still not fixed by retagging
# rows into a shipped milestone; what changed is that `scripts/m1-exit.sh` now
# tells that abstention apart from the other exit-2 shapes and prints the
# sentence it tolerates. See the block above `m1-exit`.
#
# All four run even when an earlier one is red, for m1-exit's reason: stopping
# at the first failure reports part of the debt and costs a round trip to
# discover the rest.
#
# THE AGGREGATION IS IN A SCRIPT AND NOT HERE, AND THAT WAS A DEFECT FIX. This
# recipe used to be four `|| rc=1` lines, which folded a NO_VERDICT into OWED;
# then Make folded every nonzero recipe status to 2. Measured on that version:
# `REQ_MILESTONE=M2 python3 scripts/requirements.py` exited 1 (OWED) while `make
# m2-exit` exited 2, which in this repo's vocabulary says NO_VERDICT. Not lossy —
# WRONG. scripts/m2-exit.sh keeps the three states and prints the verdict on its
# last line as `M2_EXIT_RESULT <code> <name>`, which survives Make.
#
# `m1-exit` HAD THE SAME AMBIGUITY AND NO LONGER DOES. It used to collapse to 2
# when red as well, so "M1 owes rows" and "an inventory would not measure" were
# one number there too; that was recorded here as dormant-not-misreporting and
# left to whoever owns M1's ledger, because giving a shipped exit criterion a new
# contract should not be a side-effect of building M2's. GI-08 is that decision,
# taken on purpose: scripts/m1-exit.sh carries the same three values, the same
# lattice and the same `<M>_EXIT_RESULT` last line as this file. The two targets
# now differ only in their owner and in one mapping M1 needs and M2 does not —
# the tolerated zero-row abstention, which M2 cannot reach because it owns rows.
.PHONY: m2-exit
m2-exit: build ## M2's exit criterion: nothing in ANY inventory still owed to M2
	@bash scripts/m2-exit.sh

# GI-09, and it is the reason `m2-exit` is allowed to be believed. An owner
# filter nobody has watched fail is not a filter: `CONFORMANCE_FORBID_OWNER` has
# its negative controls in scripts/test-conformance-runner.sh (item7) and
# `TEST_XFAIL_FORBID_OWNER` has its own in scripts/test-xfail.py, so inventory
# four arrived owing the same proof.
#
# It plants a row for the milestone under test and requires the runner to go RED
# for it — and, the half that is easy to leave out, it requires `make m2-exit`
# to still READ all four inventories. Weakening the exit target is otherwise
# invisible: deleting one line of the recipe leaves every other gate green.
.PHONY: test-requirements-runner
test-requirements-runner: ## Plant a row for the milestone under test and prove the gate goes RED
	@bash scripts/test-requirements-runner.sh

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
check-doc-evidence: ## Pin doc citations, the no-compile allowlist, and RUN every cmd: item
	@bash scripts/check-doc-evidence.sh

# The evidence gate passed every day for as long as `cmd:` was a shape check, over nine
# false items, because passing cost nothing. This hands it lies and requires it to say so.
.PHONY: test-doc-evidence
test-doc-evidence: ## Prove the evidence gate goes RED on a deliberately false cmd: item
	@bash scripts/test-doc-evidence.sh

# NOT in `gates`: it rewrites the gate's own sources and puts back what it found (a
# startup snapshot, not `git checkout`), and it refuses to run against a dirty tree — the
# normal state while someone is working on one — because mutating an unreviewed file
# measures an unreviewed file.
# It runs in CI, where the tree is always clean, and on demand. A green control suite only
# says the controls agree with the code; this says they would NOTICE if the code stopped
# working. (No counts here on purpose: they drift, and a stale number in a comment is the
# same defect this whole branch is about.)
.PHONY: test-doc-evidence-coverage
test-doc-evidence-coverage: ## Revert each evidence-gate fix and count the controls that catch it
	@bash scripts/test-doc-evidence-coverage.sh

# `gate:` evidence cannot be validated by the doc lint: check-docs is a step of `gates`,
# and the gates the index cites are conformance and selfhost, so the lint would recurse
# into its own caller. Static linting and receipt collection are therefore separate
# targets. This one runs each DISTINCT cited gate once — three rows cite `make selfhost`,
# it runs once — and requires every declared outcome to appear in what that gate printed
# in THIS run. It re-runs gates that `make gates` also runs directly (~31s measured); that
# is the price of keeping their output streamed rather than buried in a receipt file.
.PHONY: gate-receipts
gate-receipts: build ## Run every gate the feature index cites, and validate its claims
	@bash scripts/gate-receipts.sh

# stdlib/ = library modules with no `fn main`; the only pinnable thing is a
# compile verdict plus its blocker. tests/stdlib/*.pd are ordinary conformance
# fixtures and are run + transcript-diffed by `make conformance`, NOT here.
.PHONY: stdlib-gate
stdlib-gate: build ## Pin the stdlib/ measurement, account builtins, check generated C
	@bash scripts/stdlib-gate.sh

.PHONY: test-gate-probe
test-gate-probe: build ## Fault-inject every producer the stdlib gate reads as evidence
	@bash scripts/test-gate-probe.sh

# `#[command(version = "0.1.0-alpha")]` sat in src/cli.rs while Cargo.toml went 0.1.0 ->
# 0.2.0 -> 0.3.0, so both shipped releases install a compiler that answers `--version` with
# 0.1.0-alpha. Two releases, no gate, because no gate ever ran the binary. This one does:
# it executes every declared [[bin]] and compares what it printed against the manifest.
# Not a grep for `env!` — that reads the source, and the source is not what the user runs.
.PHONY: version-gate
version-gate: build ## Run every built binary and require its --version to match Cargo.toml
	@bash scripts/version-gate.sh

.PHONY: test-version-gate
test-version-gate: ## Prove the version gate goes RED on binaries that misreport themselves
	@bash scripts/test-version-gate.sh

# THE SOURCE SIDE OF THE VERSION CLAIM, AND IT NEEDED ITS OWN TARGET BECAUSE IT
# HAD NO PATH TO `gates` AT ALL. `version-gate` above RUNS the binaries; it can
# only read output shaped `<name> <version>`, so the banner in src/main.rs and a
# `pub const` no binary prints are structurally invisible to it — which is where
# two of the three version defects actually lived. That surface is covered by
# tests/version_matches_cargo_toml.rs, an ORDINARY integration test, and
# `make test-rust` is `--lib --bins`: nothing on the certifying path executed it.
# A check reachable only by someone who already knows to name it is a document.
#
# COST, MEASURED AT THE `gates` LEVEL, the way the test-xfail entry below insists
# on — AND MEASURED TWICE, because once was misleading. Two back-to-back pairs of
# `make gates` without / with this entry and `test-honest`:
#
#     1m45s -> 2m07s   (+21s)
#     2m00s -> 2m46s   (+46s)
#
# Same tree, same machine, hours apart. The spread is machine load, not the
# gates, and quoting either number alone would have been a claim the next run
# falsifies — so both are here and the honest statement is "tens of seconds on a
# two-minute run". On its own this target is 0.2s warm once the test binary is
# linked; quoting THAT would understate what adding it to the list costs. It is
# placed beside the two version targets it completes rather than at the end,
# because its failure is the same class.
.PHONY: version-source-gate
version-source-gate: ## Require that no source file states this compiler's version by hand
	@$(CARGO) test --release --test version_matches_cargo_toml

# `test-xfail` BELONGS IN THIS LIST, and its absence was this round's own defect.
# It is the check that every #[ignore]d row still fails FOR THE REASON IT
# DECLARES — the headline mechanism of the last two rounds — and until now it was
# reachable only by naming its own target or through `m1-exit` — which was RED
# when this was written, and so was no evidence that anything passed. A check
# that the certifying path does not run is a document.
#
# COST, MEASURED BY RUNNING IT BOTH WAYS BACK TO BACK rather than by subtracting
# a standalone timing: `make gates` 1m43s without this entry, 1m47s with it — 4s,
# because everything ahead of it in this list has already built the test binaries
# it needs. (`make test-xfail` on its own is 41s from a build warm for `cargo
# build --release` but not for `cargo test --release`; that number is the
# rebuild, not this gate, and quoting it here would have overstated the cost by
# ten times.) It sits at the expensive end of the list — it runs every #[ignore]d
# row — so every cheaper failure is reported before it starts. It was "last, and
# the only entry that runs the whole test suite" until `test-honest` joined the
# prerequisites after it, which is both of those things more literally.
#
# `test-honest` IS HERE NOW, AND THE COMMENT THAT USED TO SIT IN ITS PLACE WAS
# WRONG TWICE OVER. It said the suite was "a real hole… a scope decision with an
# owner", which named no owner and tracked no obligation — a decoration. Then,
# writing this round's replacement, I asserted the suite could not be in this
# list because its debt is open by design. MEASURED, before shipping that
# sentence: `make test-honest` is 668 passed / 0 failed / 46 ignored, GREEN, 8.4s
# warm. The debt that is open is `m1-exit`'s first two inventories (2 rows
# OWED_TO_M1); inventory 3, which is this same command character for character,
# is green. So the only argument against it was cost, and the cost is 8.4s
# standalone-warm — the bulk of what this entry and `version-source-gate` add to
# `make gates` together, which two back-to-back pairs put at +21s and +46s on a
# two-minute run. See the `version-source-gate` comment for why both are quoted.
#
# And "m1-exit already runs it" is not an answer — that is exactly the argument
# the last round rejected for `test-xfail`: a target that is RED by design is
# never evidence that anything inside it passed.
#
# `version-source-gate` IS ALSO newly here, and its absence was this round's
# defect, the same shape as `test-xfail`'s last round: the source-side version
# check lived in an ordinary integration test that no target on this path ran.
# It OVERLAPS `test-honest` — the same four tests run twice, 0.2s — and stays
# separate on purpose: it fails with a headline naming the version claim instead
# of one line inside a 714-test `--no-fail-fast` wall, and a named target is
# something scripts/test-version-gate.sh can pin membership of, which "some test
# somewhere inside the suite" is not.
.PHONY: gates
# `thesis-exit` is DELIBERATELY ABSENT and must stay absent: it exits 2 by design, and a
# green umbrella that swallowed a NO_VERDICT would be the one reading this branch exists to
# prevent. Its SELF-TEST belongs here, though — every defence this branch built was
# reachable only from a target outside the umbrella, which is an enforcement gap, not a
# design choice.
#
# RESOLVED AS A UNION, DELIBERATELY, AND FOR THE SECOND TIME. `main` and each incoming
# branch have carried gates the other did not, so taking either side wholesale silently
# DROPS gates. From the thesis branch: check-retracted-claims, test-thesis-runner,
# test-xfail. From fix/d3b-tail-if: version-source-gate, test-honest. Fifteen targets, and
# a conflict on this line should be resolved this way every time — the union is the only
# resolution that cannot lose a gate, and losing one is silent by construction.
#
# `test-honest` ARRIVING HERE CLOSES GI-06, which docs/contributing/1.0-requirements.tsv
# carried as `owed` and MILESTONES.md described as "a one-word change nobody has made".
# Both are updated in this same commit: a requirement whose status changes in a merge and
# is not written down in that merge is a claim measured against a state that no longer
# exists.
#
# `test-requirements-runner` IS HERE (GI-09) and `m2-exit` IS NOT (GI-08), and the split is
# the same one `thesis-exit` / `test-thesis-runner` already draws: the exit target is RED by
# design, so it can never be in this list; its self-test is green by design, and leaving it
# out would put the only proof that inventory four can go RED behind a target nobody on the
# certifying path runs. That is the argument this repo has now made three times, for
# `test-xfail`, for `version-source-gate` and for `test-honest`.
#
# COST, MEASURED THE WAY THOSE TWO ENTRIES INSIST ON — `make gates` back to back without and
# with this entry: 1m54s -> 1m56s. It builds nothing (no `build` prerequisite; it reads two
# text files and shells out to `make -n`), so the 2s is its own runtime and not a rebuild.
gates: conformance test-conformance-runner check-docs gate-receipts test-doc-evidence selfhost stdlib-gate test-gate-probe version-gate test-version-gate version-source-gate check-retracted-claims test-thesis-runner test-xfail test-requirements-runner test-honest ## Run every language-level gate
	@echo "$(GREEN)✓ all gates green$(NC)"

# `make gates` GREEN IS A STATEMENT ABOUT THIS WORKTREE, AND THAT IS NOT THE TREE
# THAT LANDS. Measured: this branch was green under `make gates` six consecutive
# times and its merge into main was RED with 18 cited line ranges MOVED — main's
# MILESTONES.md cites LINE NUMBERS into files this branch inserted lines above,
# and git merged both sides without a single conflict marker because they touch
# different regions of different files. A semantic conflict is invisible to a
# three-way merge and was invisible to both branches' gates.
#
# DELIBERATELY NOT IN `gates`: it RUNS `gates`, so putting it in the list would
# recurse. It is the hand-back step, not a gate — run it before declaring a
# branch ready, and read the sha it prints, because the verdict expires the
# moment the target ref moves.
.PHONY: merge-preflight
merge-preflight: ## Run `make gates` against the merge of this branch into REF (default main)
	@bash scripts/merge-preflight.sh $(REF)

# --- Expected failures in the Rust test suite ------------------------------
# Appended at the end deliberately: the gates section above is being edited in
# parallel.

.PHONY: test-xfail
test-xfail: ## Run the #[ignore]d tests and fail if a declared failure now passes
	@python3 scripts/test-xfail.py

# --- The definition of 1.0, as a command -----------------------------------
# Committed RED on purpose. See scripts/thesis-exit.sh and
# docs/contributing/MILESTONES.md: 1.0 is the thesis proven on the self-hosting
# compiler, not an inventory with no unmet rows.

.PHONY: thesis-exit
thesis-exit: build ## The definition of Palladium 1.0. RED until M9.
	@bash scripts/thesis-exit.sh

# Fault injection, not a call to the helpers: for every probe that reads source or
# a verdict, a state that VIOLATES the property must go RED and a state that
# satisfies it must go green. Probe groups with no negative control are named in
# the output rather than left silent.
.PHONY: test-thesis-runner
test-thesis-runner: build ## Fault-inject every thesis probe and prove it can still go RED
	@bash scripts/thesis-exit.sh --self-test

# The banned-list check belongs on the release path, not only under --self-test:
# three retracted claims survived a deletion because nothing on this path looked.
.PHONY: check-retracted-claims
check-retracted-claims: ## Fail if wording a review round retracted has come back
	@python3 scripts/thesis_exit.py --check-retracted-claims
