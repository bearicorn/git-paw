# git-paw task runner

# Run fmt + clippy + test
check: lint test

# Run all tests
test:
    cargo test

# Trustworthy full verification: runs the WHOLE test suite (no fail-fast) with
# the no-tmux-server guard neutralised, plus lint + supply-chain gates. Use
# this for verification instead of `check`/`test`: plain `cargo test` is
# fail-fast across binaries, so the env-guard test (tripped by any live `paw-*`
# tmux session, in an early-alphabetical binary) can abort the run and mask
# every later suite. The suite is socket-isolated, so the opt-out is safe.
verify: lint deny
    GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast
    cargo audit

# Generate HTML coverage report (same output location as CI)
coverage:
    cargo llvm-cov --html --output-dir docs/book/coverage
    @echo "Report: docs/book/coverage/index.html"

# Run fmt check + clippy
lint:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings

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

# Cold-start integration tests on the host (refuses if a paw-* tmux session is live)
smoke:
    #!/usr/bin/env bash
    set -euo pipefail
    if tmux ls 2>/dev/null | grep -q '^paw-'; then
        offending=$(tmux ls 2>/dev/null | grep '^paw-' | head -3 | sed 's/$/  (run: tmux kill-session -t <name>)/')
        echo "error: dogfood paw-* session detected on default tmux socket:" >&2
        printf '  %s\n' "$offending" >&2
        echo "" >&2
        echo "Kill it or pause it before running cold-start smoke." >&2
        exit 2
    fi
    env TMUX="" -u GIT_PAW_ALLOW_LIVE_SESSION cargo test --tests

# Cold-start integration tests inside an Ubuntu 24.04 container (podman → docker)
smoke-container:
    #!/usr/bin/env bash
    set -euo pipefail
    if command -v podman >/dev/null 2>&1; then
        ENGINE=podman
    elif command -v docker >/dev/null 2>&1; then
        ENGINE=docker
    else
        echo "error: neither podman nor docker found on PATH" >&2
        echo "Install one of them to run the containerised smoke:" >&2
        echo "  brew install podman   # or: brew install docker" >&2
        exit 2
    fi
    if ! $ENGINE image exists git-paw-ci >/dev/null 2>&1 \
        && ! $ENGINE image inspect git-paw-ci >/dev/null 2>&1; then
        echo "Building git-paw-ci image via $ENGINE (one-time, ~3-5 min)..." >&2
        $ENGINE build -t git-paw-ci -f Containerfile .
    fi
    echo "Running cold-start smoke in $ENGINE container..." >&2
    $ENGINE run --rm \
        -v "$PWD:/src:Z" \
        -v paw-cargo-cache:/root/.cargo:Z \
        -v paw-target-cache:/src/target:Z \
        -w /src \
        -e CARGO_TERM_COLOR=always \
        git-paw-ci \
        bash -c "cargo test --tests"

# Run all smoke layers: host on Linux, host + container on macOS
smoke-all: smoke
    #!/usr/bin/env bash
    set -euo pipefail
    case "$(uname -s)" in
        Darwin) just smoke-container ;;
        *)      echo "host matches CI on $(uname -s); skipping container layer" ;;
    esac
