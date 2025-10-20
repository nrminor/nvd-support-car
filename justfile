export CARGO_TERM_COLOR := "always"

[group('help')]
default:
    @just --list --unsorted

[group('build')]
build:
    cargo build

alias b := build

[group('build')]
release:
    cargo build --release

alias r := release

[group('build')]
run *args:
    cargo run -- {{ args }}

alias rn := run

[group('build')]
install:
    cargo install --path=.

alias i := install

[group('test')]
test:
    cargo test

alias t := test

[group('lint')]
clippy:
    cargo clippy --all-targets -- -D warnings

alias c := clippy

[group('lint')]
fmt:
    cargo fmt

alias f := fmt

[group('lint')]
fmt-check:
    cargo fmt -- --check

alias fc := fmt-check

[group('lint')]
check: fmt-check clippy test

alias ck := check

[group('lint')]
check-fast:
    cargo check --all-targets

alias cf := check-fast

[group('docs')]
doc:
    cargo doc --open

alias d := doc

[group('docs')]
doc-build:
    cargo doc --no-deps

alias db := doc-build

[group('dev')]
watch:
    cargo watch -x check -x 'clippy -- -D warnings' -x test

alias w := watch

[group('dev')]
audit:
    cargo audit

alias a := audit

[group('maintenance')]
clean:
    cargo clean

[group('release')]
verify-release: check doc-build
    @echo "✓ All checks passed!"
    @echo "✓ Documentation builds successfully!"
    @echo "Ready for release - remember to:"
    @echo "  1. Update version in Cargo.toml"
    @echo "  2. Create git tag with 'git tag v0.0.0'"
    @echo "  3. Push tag to trigger release workflow"
