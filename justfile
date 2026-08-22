# Mab — My Alignment Browser
# Common development tasks

_default:
    @just --list

# Build the workspace
build:
    cargo build

# Build in release mode
release:
    cargo build --release

# Run the application
run:
    cargo run --bin mab

# Run tests
test:
    cargo test

# Run clippy on all targets and features
clippy:
    cargo clippy --all-targets --all-features

# Check formatting
fmt-check:
    cargo fmt --check

# Format code
fmt:
    cargo fmt

# Format Markdown files with dprint
fmt-md:
    dprint fmt "**/*.md"

# Check Markdown formatting without modifying files
md-check:
    dprint check "**/*.md"

# Run all lints (clippy + formatting check)
lint: clippy fmt-check

# Check dependencies with cargo-deny
deny:
    cargo deny check

# Check security advisories
audit:
    cargo audit

# Find unused dependencies
machete:
    cargo machete

# Run the full local check suite
check: lint test deny audit machete

# Clean build artifacts
clean:
    cargo clean
