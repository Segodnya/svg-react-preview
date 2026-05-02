.PHONY: build release test check fmt fmt-check lint lint-pedantic lint-cross clean install uninstall run example help \
        coverage coverage-html coverage-gate verify deny machete audit udeps similarity geiger outdated msrv mutants

# cargo-llvm-cov needs llvm-cov / llvm-profdata that match rustc's profile format.
# Resolution order:
#   1. caller-supplied LLVM_COV / LLVM_PROFDATA (highest priority)
#   2. rustup's `llvm-tools-preview` component (only if found via the active toolchain)
#   3. Homebrew LLVM at /opt/homebrew/opt/llvm/bin (last-ditch fallback)
# A bare `cargo llvm-cov` is unreliable when both Homebrew Rust and rustup are
# installed: PATH may surface Homebrew's rustc whose sysroot lacks llvm-tools.
RUSTUP_LLVM_BIN := $(shell find $(HOME)/.rustup/toolchains -maxdepth 5 -name llvm-cov -path '*lib/rustlib*' 2>/dev/null | head -1 | xargs -I{} dirname {})
ifneq ($(RUSTUP_LLVM_BIN),)
    LLVM_COV      ?= $(RUSTUP_LLVM_BIN)/llvm-cov
    LLVM_PROFDATA ?= $(RUSTUP_LLVM_BIN)/llvm-profdata
else
    LLVM_COV      ?= /opt/homebrew/opt/llvm/bin/llvm-cov
    LLVM_PROFDATA ?= /opt/homebrew/opt/llvm/bin/llvm-profdata
endif
COVERAGE_ENV = LLVM_COV=$(LLVM_COV) LLVM_PROFDATA=$(LLVM_PROFDATA)

# Minimum line coverage enforced by `make coverage-gate`. Bump as the suite grows.
COVERAGE_MIN  ?= 95

# Build the binary in debug mode.
build:
	cargo build

# Build the optimized release binary at target/release/svg-react-preview.
release:
	cargo build --release

# Run all integration tests (golden fixtures in tests/fixtures).
test:
	cargo test

# Type-check without producing artifacts (faster than build).
check:
	cargo check --all-targets

# Format the workspace.
fmt:
	cargo fmt --all

# Verify formatting without modifying files (use in CI).
fmt-check:
	cargo fmt --all -- --check

# Run clippy with warnings promoted to errors.
lint:
	cargo clippy --all-targets --all-features -- -D warnings

# Run clippy with pedantic + nursery groups (matches CI).
lint-pedantic:
	cargo clippy --all-targets --all-features -- \
		-D warnings -W clippy::pedantic -W clippy::nursery

# Cross-platform clippy: lint the workspace as if it were built on Linux.
# Catches non-macOS `dead_code` / `unused_variables` warnings that the host
# toolchain can't see (e.g. items behind `#[cfg(target_os = "macos")]`).
# Requires:
#   * rustup (Homebrew Rust ships only the host target)
#   * `rustup target add x86_64-unknown-linux-gnu`
#   * `x86_64-linux-gnu-gcc` (e.g. brew tap messense/macos-cross-toolchains
#     && brew install x86_64-unknown-linux-gnu) — needed by psm's build.rs.
# Routes through rustup-managed cargo and uses an isolated CARGO_TARGET_DIR
# so proc-macros built by Homebrew rustc don't poison the cache.
lint-cross:
	@command -v rustup >/dev/null 2>&1 || { echo "rustup not on PATH; install from https://rustup.rs"; exit 1; }
	@rustup target list --installed | grep -q '^x86_64-unknown-linux-gnu$$' || { echo "missing target; run: rustup target add x86_64-unknown-linux-gnu"; exit 1; }
	@command -v x86_64-linux-gnu-gcc >/dev/null 2>&1 || { echo "missing x86_64-linux-gnu-gcc; run: brew tap messense/macos-cross-toolchains && brew install x86_64-unknown-linux-gnu"; exit 1; }
	PATH="$$(dirname $$(rustup which cargo)):$$PATH" \
	    CARGO_TARGET_DIR=target/cross-linux \
	    CC_x86_64_unknown_linux_gnu=x86_64-linux-gnu-gcc \
	    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc \
	    cargo clippy --all-targets --all-features --target x86_64-unknown-linux-gnu -- \
	        -D warnings -W clippy::pedantic -W clippy::nursery

# Remove build artifacts.
clean:
	cargo clean

# Install svg-react-preview into ~/.cargo/bin so the Zed task can find it on PATH.
install:
	cargo install --path . --force

# Remove the installed binary.
uninstall:
	cargo uninstall svg-react-preview

# Run the binary against the full_svg fixture (smoke check that the pipeline works end-to-end).
example:
	SVG_REACT_PREVIEW_INPUT="$$(cat tests/fixtures/full_svg.tsx)" cargo run --quiet

# Pipe a JSX/TSX selection from stdin (usage: `echo '<path d="M0 0"/>' | make run`).
run:
	cargo run --quiet

# Print a per-file line/region coverage summary plus the list of uncovered lines.
# Requires `cargo install cargo-llvm-cov`.
coverage:
	$(COVERAGE_ENV) cargo llvm-cov --show-missing-lines

# Generate an HTML report at target/llvm-cov/html/index.html.
coverage-html:
	$(COVERAGE_ENV) cargo llvm-cov --html
	@echo "open target/llvm-cov/html/index.html"

# CI gate: fail if line coverage drops below COVERAGE_MIN (default: 95%).
coverage-gate:
	$(COVERAGE_ENV) cargo llvm-cov --summary-only --fail-under-lines $(COVERAGE_MIN)

# --- Quality / security tooling -------------------------------------------------
# All of these mirror the corresponding CI job; install hints are in the README.

# cargo-deny: advisories + bans + licenses + sources.
deny:
	cargo deny check

# cargo-audit: RustSec advisories (subset of deny, kept for parity with CI).
audit:
	cargo audit

# cargo-machete: unused dependencies (stable).
machete:
	cargo machete

# cargo-udeps: unused dependencies (nightly, more accurate).
udeps:
	cargo +nightly udeps --all-targets --all-features --locked

# similarity-rs: copy-paste detector for Rust source.
similarity:
	similarity-rs --threshold 0.85 --min-lines 5 src

# cargo-geiger: count unsafe usage (forbid-only mode).
geiger:
	cargo geiger --all-features --locked --forbid-only

# cargo-outdated: dependencies with newer versions available.
outdated:
	cargo outdated --workspace --exit-code 1

# cargo-msrv: verify that the pinned MSRV still compiles.
msrv:
	cargo msrv verify

# cargo-mutants: mutation testing.
mutants:
	cargo mutants --in-place --no-shuffle --timeout 120

# Pre-commit gate replicated locally — same set as `.cargo-husky/hooks/pre-commit`.
verify: fmt-check lint-pedantic lint-cross test deny machete

# List available targets.
help:
	@grep -E '^[a-zA-Z_-]+:' Makefile | sed 's/:.*//' | sort
