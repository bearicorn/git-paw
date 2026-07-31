# Ubuntu 24.04 base for cold-start CI parity smoke runs.
#
# Used by `just smoke-container` to reproduce the GitHub-Actions Linux
# runner environment locally on macOS dev boxes. Matches the tmux
# version available on `ubuntu-latest` (currently 3.4 in noble).
#
# Build: podman build -t git-paw-ci -f Containerfile .
# Run:   podman run --rm --init --userns=keep-id \
#          -v "$PWD:/src:Z" -v paw-cargo-cache:/home/ci/.cargo:Z \
#          -w /src git-paw-ci bash -c "cargo test"
#
# Runs as the non-root `ci` user to MIRROR the GitHub-Actions Linux runner
# (which is non-root). Running as root bypasses file-permission bits and would
# break permission-sensitive tests that CI passes; `--init` gives a real PID-1
# reaper so orphan-detection (getppid) tests see a normal process tree.

FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential \
        pkg-config \
        libssl-dev \
        curl \
        ca-certificates \
        git \
        tmux \
        jq \
    && rm -rf /var/lib/apt/lists/*

# Non-root user matching the CI runner. Rust is installed as this user so the
# toolchain lives under /home/ci/.cargo (copied into the cache volume on first
# mount, exactly as the old /root/.cargo setup relied on).
RUN useradd --create-home --shell /bin/bash ci
USER ci
WORKDIR /home/ci

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
        sh -s -- -y --default-toolchain stable --profile minimal --no-modify-path

ENV PATH=/home/ci/.cargo/bin:$PATH

WORKDIR /src
