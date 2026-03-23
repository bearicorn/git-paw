# git-paw task runner

# Run fmt + clippy + test
check: lint test

# Run all tests
test:
    cargo test

# Run all tests including tmux-dependent ignored tests
test-all:
    cargo test -- --include-ignored

# Generate HTML coverage report
coverage:
    cargo llvm-cov --html
    @echo "Report: target/llvm-cov/html/index.html"

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
changelog:
    git cliff -o CHANGELOG.md

# Build release binary
build:
    cargo build --release

# Install from local source
install:
    cargo install --path .

# Clean build artifacts
clean:
    cargo clean
