# Contributing to git-paw

Thanks for your interest in contributing! This guide will help you get started.

## Prerequisites

- **Rust** (stable) — install via [rustup](https://rustup.rs/)
- **tmux** — `brew install tmux` (macOS) or `apt install tmux` (Linux)
- **just** — `cargo install just` ([casey/just](https://github.com/casey/just))

## Getting Started

```bash
# Clone the repo
git clone https://github.com/bearicorn/git-paw.git
cd git-paw

# Build
cargo build

# Run all checks (lint + test)
just check
```

## Development Commands

| Command | Description |
|---------|-------------|
| `just check` | Run lint + tests |
| `just test` | Run all tests |
| `just test-all` | Run all tests including tmux-dependent tests |
| `just lint` | Run `cargo fmt --check` + `cargo clippy` |
| `just coverage` | Generate HTML coverage report |
| `just build` | Build release binary |
| `just install` | Install from local source |
| `just docs` | Build and open mdBook docs |
| `just api-docs` | Build and open Rustdoc |

## Making Changes

### Branch Naming

Use prefixes that match the type of change:

- `feat/` — new feature (e.g., `feat/preset-import`)
- `fix/` — bug fix (e.g., `fix/worktree-cleanup`)
- `docs/` — documentation (e.g., `docs/config-examples`)
- `refactor/` — code restructuring (e.g., `refactor/session-state`)
- `test/` — test additions (e.g., `test/tmux-builder`)
- `ci/` — CI/CD changes (e.g., `ci/coverage-threshold`)

### Commit Messages

We use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]
```

**Types:** `feat`, `fix`, `docs`, `refactor`, `test`, `ci`, `chore`

**Examples:**

```
feat(detect): add support for custom CLI detection
fix(tmux): handle pane split failure on small terminals
docs(readme): add per-branch CLI example
test(session): add recovery round-trip test
```

### Code Style

- Run `just lint` before committing
- Clippy is configured with `pedantic` warnings — address them
- No `unwrap()` or `expect()` in non-test code
- Every public item needs a `///` doc comment
- Every module needs a `//!` module doc comment

## Pull Request Process

1. Fork the repo and create your branch from `main`
2. Make your changes with tests
3. Ensure `just check` passes
4. Write a clear PR description explaining the "why"
5. Link related issues (e.g., "Closes #14")

### PR Checklist

- [ ] `cargo build` compiles without warnings
- [ ] `cargo clippy -- -D warnings` passes
- [ ] `cargo fmt --check` passes
- [ ] Tests added/updated for changes
- [ ] Commit messages follow Conventional Commits

## Adding a Custom CLI to the Default List

To add a new AI CLI to the auto-detection list:

1. Add the binary name to `KNOWN_CLIS` in `src/detect.rs`
2. Add a row to the "Supported AI CLIs" table in `README.md`
3. Add a test case in the detection tests
4. Submit a PR with a link to the CLI's homepage

## Questions?

Open an [issue](https://github.com/bearicorn/git-paw/issues) or start a [discussion](https://github.com/bearicorn/git-paw/discussions).
