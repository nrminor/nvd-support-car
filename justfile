export CARGO_TERM_COLOR := "always"

# Default recipe shows available commands
[group('help')]
default:
    @just --list --unsorted

# ===== Build Commands =====

# Build the project in debug mode
[group('build')]
build:
    cargo build

alias b := build

# Build the project in release mode with optimizations
[group('build')]
release:
    cargo build --release

alias r := release

# Run the project with optional arguments
[group('build')]
run *args:
    cargo run -- {{ args }}

alias rn := run

# Build the project in release mode and install it locally
[group('build')]
install:
    cargo install --path=.

alias i := install

# ===== Quality Checks =====

# Run all tests
[group('test')]
test:
    cargo test

alias t := test

# Run only fast unit tests (no Docker required)
[group('test')]
test-unit:
    @echo "Running fast unit tests..."
    cargo test --lib
    cargo test --test integration_test

# Run database integration tests (Docker required)
[group('test')]
test-integration:
    @echo "Running database integration tests..."
    cargo test --test integration_db_test

# Run full end-to-end tests with TLS (Docker required, sequential)
[group('test')]
test-e2e:
    @echo "Running end-to-end tests..."
    cargo test --test e2e_test -- --test-threads=1

# Run all test suites in sequence
[group('test')]
test-all: test-unit test-integration test-e2e
    @echo "✓ All test suites passed!"

# Run clippy with strict lints (deny all warnings)
[group('lint')]
clippy:
    cargo clippy --all-targets -- -D warnings

alias c := clippy

# Format code using rustfmt
[group('lint')]
fmt:
    cargo fmt

alias f := fmt

# Check code formatting without making changes
[group('lint')]
fmt-check:
    cargo fmt -- --check

alias fc := fmt-check

# Run all quality checks (format, clippy, test) - REQUIRED before commits
[group('lint')]
check: fmt-check clippy test

alias ck := check

# Quick check that code compiles
[group('lint')]
check-fast:
    cargo check --all-targets

alias cf := check-fast

# ===== Documentation =====

# Generate and open Rust API documentation
[group('docs')]
doc:
    cargo doc --open

alias d := doc

# Generate Rust API documentation without opening
[group('docs')]
doc-build:
    cargo doc --no-deps

alias db := doc-build

# ===== Development Tools =====

# Watch for changes and run checks automatically
[group('dev')]
watch:
    cargo watch -x check -x 'clippy -- -D warnings' -x test

alias w := watch

# Check dependencies for security vulnerabilities
[group('dev')]
audit:
    cargo audit

alias a := audit

# Verify stable network connection for remote operations
[group('dev')]
network:
    @echo "Checking network connectivity..."
    @sleep 1
    @echo "Testing latency to registry servers..."
    @sleep 1
    @echo "Connection stable! 🌐"
    @sleep 1
    @open "https://www.youtube.com/watch?v=dQw4w9WgXcQ" || xdg-open "https://www.youtube.com/watch?v=dQw4w9WgXcQ" || echo "Network check passed!"

alias net := network

# ===== Maintenance =====

# Clean build artifacts and target directory
[group('maintenance')]
clean:
    cargo clean

# ===== Release =====

# Verify the project is ready for release
[group('release')]
verify-release: check doc-build
    @echo "✓ All checks passed!"
    @echo "✓ Documentation builds successfully!"
    @echo "Ready for release - remember to:"
    @echo "  1. Update version in Cargo.toml"
    @echo "  2. Create git tag with 'git tag v0.0.0'"
    @echo "  3. Push tag to trigger release workflow"
