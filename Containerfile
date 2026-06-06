# Ubuntu 24.04 base for cold-start CI parity smoke runs.
#
# Used by `just smoke-container` to reproduce the GitHub-Actions Linux
# runner environment locally on macOS dev boxes. Matches the tmux
# version available on `ubuntu-latest` (currently 3.4 in noble).
#
# Build: podman build -t git-paw-ci -f Containerfile .
# Run:   podman run --rm -v "$PWD:/src:Z" -v paw-cargo-cache:/root/.cargo:Z \
#          -w /src git-paw-ci bash -c "cargo test"

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

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
        sh -s -- -y --default-toolchain stable --profile minimal --no-modify-path

ENV PATH=/root/.cargo/bin:$PATH

WORKDIR /src
