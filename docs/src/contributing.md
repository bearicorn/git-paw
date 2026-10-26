# Contributing

Contributions to git-paw are welcome! This guide covers the development workflow.

## Prerequisites

- Rust (stable, per `rust-toolchain.toml`; the MSRV is 1.96, per `rust-version` in `Cargo.toml`)
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
| `just test` | Run all tests (tmux tests included — tmux is a hard dependency) |
| `just verify` | Lint + `cargo deny` + the full suite with `--no-fail-fast` + `cargo audit` |
| `just lint` | Run `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` |
| `just deny` | Run `cargo deny check` |
| `just audit` | Run `cargo audit` |
| `just smoke-all` | Cold-start integration tests (host, plus a container on macOS) |
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
# All tests (unit + integration; tmux tests run normally, so tmux must be installed)
cargo test

# Coverage report
cargo llvm-cov --html
# Open: target/llvm-cov/html/index.html
```

Tests are organized as:
- **Unit tests** — `#[cfg(test)]` modules within each source file
- **Integration tests** — `tests/` directory (CLI binary tests, worktree lifecycle, session round-trips)
- **Tmux-dependent tests** — run normally (tmux is a hard dependency)

## Code Style

- **Formatting:** `cargo fmt` (config in `rustfmt.toml`)
- **Linting:** `cargo clippy --all-targets -- -D warnings` with pedantic lints enabled
- **No panics:** No `unwrap()` or `expect()` in non-test `src/` code
- **Documentation:** `//!` module-level doc comments, `///` on all public items

The no-panics rule is enforced by the lint gate. `src/lib.rs` and `src/main.rs` carry
`#![warn(clippy::unwrap_used, clippy::expect_used)]`, so `cargo clippy --all-targets -- -D warnings`
fails on a new `unwrap()` or `expect()` anywhere in library or binary code. The repo-root
`clippy.toml` sets `allow-unwrap-in-tests = true` and `allow-expect-in-tests = true`, keeping
`#[test]` functions and `#[cfg(test)]` modules exempt; integration-test crates under `tests/` are
outside the rule.

The gate has two limits. Platform-gated code is linted only on a host where that `cfg` is active,
and CI lints on Linux, so a violation inside a macOS-only block will not be caught there. And an
`#[allow(clippy::expect_used)]` silences the lint by construction, so the suppression set is pinned
by a test (`static_regex_helper_is_the_only_panic_lint_suppression`) rather than by the gate.

Regex patterns fixed at build time are compiled through `static_regex` in `src/static_regex.rs`,
the single audited helper that holds the only `#[allow(clippy::expect_used)]` in non-test code.
Every pattern passed to it is compiled by a unit test, so an invalid static pattern fails
`cargo test` rather than panicking at run time. Patterns that arrive at run time (configuration or
other input) are compiled directly, with the error propagated.

## The library boundary

Command bodies live in the library, in `git_paw::commands`, so the CLI, the MCP server and the
tests drive the same use cases. A command takes a `CommandContext` (repository root and config), an
options struct and an injected `Output` sink, and returns a typed outcome. It never reads the
current directory or stdin, prompts, prints — no `print!`-family macro and no
`std::io::stdout()` / `std::io::stderr()` handle, not even one passed to a helper — attaches a
terminal client, or runs the unattended drive loop. It writes through `Output` and returns; `main.rs`
does the rest. A source audit (`tests/command_library_audit.rs`) enforces this over
`src/commands/`, exempting only `completions.rs` and `broker_host.rs` by name, and a `main.rs`
guard keeps command bodies from creeping back into the entry layer.

## Where agent-facing text lives

Instructions, rules, directives, framing text and task-prompt scaffolding that an agent reads —
in its boot prompt, sidecar, or skill content — belong in a bundled asset under `assets/`
(embedded at compile time via `include_str!`), never assembled from a Rust `push_str!`/`format!`
call. Compiled code keeps the *assembly*: placeholder substitution, conditional inclusion,
per-backend selection, dedupe, and region gating. Assets are what the no-language-leak audit
(`tests/lang_agnostic_skill_audit.rs`) can see; prose compiled into a `&str` literal is invisible
to it, which is exactly how a git-paw-specific assumption once leaked into every consumer's
session. See the `export-agnosticism` dev skill (`.agents/skills/export-agnosticism/`) for the
full export-surface inventory.

## Commit Format

This project uses [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]

[optional footer(s)]
```

**Types:** `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `ci`, `chore`, `perf`

**Scopes:** `detect`, `git`, `tmux`, `session`, `config`, `interactive`, `error`, `cli`, `docs`, `ci`, `specs`, `agents`, `logging`, `replay`, `init`, `broker`, `dashboard`, `skills`, `supervisor`, `merge-loop`, `user-guide`, `worktree`, `governance`, `learnings`, `pause`

Compound scopes are written `(scope1,scope2,...)` when one change spans several scopes, e.g. `feat(cli,config): add new flag with config wiring`.

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

New code follows the [Target Architecture](architecture-direction.md): decisions are pure functions, and tmux, git, the broker and the filesystem are reached only through ports named in git-paw's own terms.
