.PHONY: build test check fmt fmt-check lint clean install uninstall run example help

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
	cargo clippy --all-targets -- -D warnings

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

# List available targets.
help:
	@grep -E '^[a-zA-Z_-]+:' Makefile | sed 's/:.*//' | sort
