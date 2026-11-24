# Build the workspace in debug mode
build:
	cargo build

# Build the release binary
release:
	cargo build --release

# Run all workspace tests
test:
	cargo test --workspace

# Format the code
fmt:
	cargo fmt --all

# Check formatting without writing changes
fmt-check:
	cargo fmt --all -- --check

# Lint the workspace, failing on warnings
clippy:
	cargo clippy --workspace --all-targets -- -D warnings

# Run the same checks as CI
ci: fmt-check clippy test

# Run fenrir, forwarding extra arguments
run *args:
	cargo run -- {{args}}

# Start the TUI
tui:
	cargo run -- tui

# Run as an HTTP server
serve:
	cargo run -- serve

# Install the release binary
install:
	cargo install --path . --locked

# Default recipe: build
default: build
