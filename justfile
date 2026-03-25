# git-paw task runner

# Run fmt + clippy + test
check: lint test

# Run all tests
test:
    cargo test

# Generate HTML coverage report (same output location as CI)
coverage:
    cargo llvm-cov --html --output-dir docs/book/coverage
    @echo "Report: docs/book/coverage/index.html"

# Run fmt check + clippy
lint:
    cargo fmt --check
    cargo clippy -- -D warnings

# Run cargo deny checks
deny:
    cargo deny check

# Run cargo audit
audit:
    cargo audit

# Build and open mdBook docs
docs:
    mdbook build docs/
    open docs/book/index.html || xdg-open docs/book/index.html

# Build and open Rustdoc API docs
api-docs:
    cargo doc --no-deps --open

# Regenerate CHANGELOG.md
changelog tag="":
    git cliff {{ if tag != "" { "--tag " + tag } else { "" } }} -o CHANGELOG.md

# Build release binary
build:
    cargo build --release

# Install from local source
install:
    cargo install --path .

# Clean build artifacts
clean:
    cargo clean
