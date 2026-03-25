# Contributing

Contributions to git-paw are welcome! This guide covers the development workflow.

## Prerequisites

- Rust (see `rust-toolchain.toml` for the exact version)
- tmux
- [just](https://github.com/casey/just) (task runner)

## Getting Started

```bash
git clone https://github.com/bearicorn/git-paw.git
cd git-paw
just check
```

## Development Commands

git-paw uses `just` as a task runner. Key recipes:

| Command | Description |
|---------|-------------|
| `just check` | Run fmt + clippy + tests |
| `just test` | Run all tests |
| `just test-all` | Run all tests including tmux-dependent ones |
| `just lint` | Run `cargo fmt --check` and `cargo clippy` |
| `just coverage` | Generate HTML coverage report |
| `just docs` | Build and open mdBook documentation |
| `just api-docs` | Build and open Rustdoc API docs |
| `just build` | Build release binary |
| `just install` | Install from local source |
| `just clean` | Clean build artifacts |

## Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Install locally
cargo install --path .
```

## Testing

```bash
# Unit tests
cargo test

# Include tmux-dependent tests (requires tmux installed)
cargo test -- --include-ignored

# Coverage report
cargo llvm-cov --html
# Open: target/llvm-cov/html/index.html
```

Tests are organized as:
- **Unit tests** — `#[cfg(test)]` modules within each source file
- **Integration tests** — `tests/` directory (CLI binary tests, worktree lifecycle, session round-trips)
- **Tmux-dependent tests** — marked `#[ignore]`, run with `--include-ignored`

## Code Style

- **Formatting:** `cargo fmt` (config in `rustfmt.toml`)
- **Linting:** `cargo clippy -- -D warnings` with pedantic lints enabled
- **No panics:** No `unwrap()` or `expect()` in non-test `src/` code
- **Documentation:** `//!` module-level doc comments, `///` on all public items

## Commit Format

This project uses [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]

[optional footer(s)]
```

**Types:** `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `ci`, `chore`

**Scopes:** `cli`, `detect`, `git`, `tmux`, `session`, `config`, `interactive`, `error`

**Examples:**
```
feat(tmux): add mouse mode support
fix(session): handle missing state file gracefully
docs: update installation instructions
test(git): add worktree creation edge cases
```

## Branch Naming

```
feat/<description>     # New features
fix/<description>      # Bug fixes
docs/<description>     # Documentation
test/<description>     # Test additions
ci/<description>       # CI/CD changes
```

## Pull Request Process

1. Fork the repository
2. Create a feature branch from `main`
3. Make your changes
4. Ensure `just check` passes (fmt, clippy, tests)
5. Write or update tests as needed
6. Open a PR against `main`

PRs should:
- Have a clear title and description
- Pass all CI checks
- Include tests for new functionality
- Follow the commit format above

## Architecture

See the [Architecture](architecture.md) chapter for an overview of the module structure and design decisions.
