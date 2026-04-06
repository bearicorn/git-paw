# AGENTS.md — git-paw

## Project Overview

**git-paw** (Parallel AI Worktrees) is a Rust CLI tool that orchestrates multiple AI coding CLI sessions across git worktrees from a single terminal using tmux.

Repository: `bearicorn/git-paw`
Crate: `git-paw`
Binary: `git-paw` (invokable as `git paw` via git subcommand convention)

## Command Set

```
git paw                        # Smart start (default)
git paw start                  # Same — reattach / recover / launch new
git paw start --cli claude     # Skip CLI picker
git paw start --cli claude --branches feat/a,feat/b  # Fully non-interactive
git paw start --from-specs     # Launch from spec files
git paw start --from-specs --cli claude  # Spec-driven, single CLI
git paw start --dry-run        # Preview without executing
git paw start --preset backend # Use config preset
git paw init                   # Initialize .git-paw/, config, gitignore
git paw stop                   # Kill tmux, keep worktrees + state for later
git paw purge                  # Nuclear: kill tmux, remove worktrees, delete state
git paw purge --force          # Skip confirmation
git paw status                 # Show session state for current repo
git paw replay --list          # List available session logs
git paw replay <branch>        # View session log, ANSI stripped
git paw replay <branch> --color # View session log with colors via less -R
git paw list-clis              # Show detected + custom CLIs
git paw add-cli <name> <cmd>   # Register a custom CLI globally
git paw remove-cli <name>      # Unregister a custom CLI
```

One session per repo. `start` is smart — it reattaches if active, recovers if stopped/crashed, or launches new if nothing exists.

## Architecture

```
src/
├── main.rs           # Entry point, command dispatch, session orchestration
├── cli.rs            # Clap derive structs and subcommands
├── config.rs         # .git-paw/config.toml and global config parsing
├── detect.rs         # AI CLI detection (scans PATH + custom CLIs)
├── git.rs            # Git operations (branches, worktrees, prune)
├── tmux.rs           # Tmux session/pane orchestration (builder pattern)
├── session.rs        # Session state persistence (~/.local/share/git-paw/)
├── interactive.rs    # Dialoguer prompts (branch picker, CLI picker, resolution chain)
├── error.rs          # PawError enum (thiserror)
├── init.rs           # git paw init — project bootstrapping
├── agents.rs         # Per-worktree AGENTS.md generation and injection
├── specs/            # Spec scanning and discovery
│   ├── mod.rs        # SpecEntry, SpecBackend trait, scan_specs()
│   ├── openspec.rs   # OpenSpec format backend (changes/ directory)
│   └── markdown.rs   # Markdown format backend (frontmatter-based)
├── logging.rs        # Session logging via tmux pipe-pane
└── replay.rs         # Replay captured session logs (ANSI/OSC stripping)
```

## Development Commands

Use `just` recipes — they mirror what CI runs:

```bash
just check                     # fmt + clippy + test — run this before pushing
just test                      # Run all tests (including tmux-dependent)
just lint                      # Format check + clippy with --all-targets
just deny                      # License, advisory, and duplicate dep checks
just audit                     # Vulnerability scan
just coverage                  # Generate HTML coverage report
just docs                      # Build mdBook and open
just api-docs                  # Build and open Rustdoc
just changelog                 # Regenerate CHANGELOG.md
just build                     # Release build
just install                   # Install from local source
just clean                     # Clean build artifacts
```

### Required dev tools

- [just](https://github.com/casey/just) — task runner
- [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) — license/advisory checks
- [cargo-audit](https://github.com/rustsec/rustsec) — vulnerability scanning
- [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) — code coverage
- [mdbook](https://github.com/rust-lang/mdBook) — docs site
- [git-cliff](https://github.com/orhun/git-cliff) — changelog generation

## Conventions

### Code Style

- Formatting configured in `rustfmt.toml`
- Clippy pedantic lints enabled (configured in `Cargo.toml` under `[lints.clippy]`)
- All public functions and types must have doc comments (`///`)
- All modules must have module-level doc comments (`//!`)
- No `unwrap()` or `expect()` in non-test code — propagate errors with `?`
- Use `PawError` variants from `error.rs` for all error cases
- Use `thiserror` for library-level error types (`error.rs`)
- Prefer `std::process::Command` for calling external tools (git, tmux)

### Linting & Supply Chain

- **rustfmt** — code formatting. Config: `rustfmt.toml`
- **clippy** — pedantic mode, `--all-targets` (lints test code too)
- **deny** — license compliance, duplicate deps, advisory checks. Config: `deny.toml`
- **audit** — vulnerability scanning
- All four run in CI and must pass for PRs to merge
- Run `just lint` for fmt + clippy, `just deny` for supply chain, `just audit` for vulnerabilities

### Commit Conventions

This project follows **Conventional Commits** (Commitizen compatible).

Format: `<type>(<scope>): <description>`

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `ci`, `chore`, `perf`
Scopes: `detect`, `git`, `tmux`, `session`, `config`, `interactive`, `error`, `cli`, `docs`, `ci`, `specs`, `agents`, `logging`, `replay`, `init`

Examples:
```
feat(specs): add spec scanning and discovery module
fix(git): prune stale worktree registrations
test(e2e): add integration tests for init and replay
docs(readme): add quick start section
```

Breaking changes: add `!` after type/scope and `BREAKING CHANGE:` footer.
All commit messages must be lowercase descriptions (no period at end).

### CLI Help Text

Every subcommand needs `about` + `long_about` with examples.
Every flag/option needs a `help` string.
The root command has `after_help` with a quick-start guide.

### Testing

- Unit tests in `#[cfg(test)] mod tests {}` at bottom of each module
- Integration tests in `tests/` directory
- `tempfile` for filesystem-touching tests
- `assert_cmd` for CLI binary tests
- tmux is a hard dependency — tmux tests run normally, not ignored
- All tests independent — no shared mutable state
- All tests must test behavior, not implementation

### Dependencies

Only add dependencies listed in the approved set:

| Crate | Purpose |
|-------|---------|
| `clap` v4 | CLI parsing with derive |
| `dialoguer` | Interactive terminal prompts |
| `console` | Terminal colors/styling |
| `which` | PATH binary detection |
| `thiserror` | Error derive macros |
| `anyhow` | Application error handling |
| `serde` + `serde_json` | Session state serialization |
| `toml` + `serde` | Config file parsing |
| `dirs` | Platform XDG directories |

Dev: `assert_cmd`, `predicates`, `tempfile`, `serial_test`

Do not add other dependencies without explicit approval.

## Config Fields

All fields in `PawConfig` (`src/config.rs`):

| Field | Type | Purpose |
|-------|------|---------|
| `default_cli` | `Option<String>` | Pre-select CLI in interactive picker |
| `default_spec_cli` | `Option<String>` | Bypass CLI picker for `--from-specs` |
| `branch_prefix` | `Option<String>` | Prefix for spec-derived branches (default: `"spec/"`) |
| `mouse` | `Option<bool>` | Enable tmux mouse mode (default: `true`) |
| `specs` | `Option<SpecsConfig>` | `[specs]` section: `dir`, `type` |
| `logging` | `Option<LoggingConfig>` | `[logging]` section: `enabled` |
| `clis` | `HashMap<String, CustomCli>` | Custom CLI definitions |
| `presets` | `HashMap<String, Preset>` | Named presets (branches + cli) |

## External Tool Dependencies

git-paw has two hard runtime dependencies:

- **tmux** — required for all session operations (start, stop, purge, status). Not optional.
- **git** — required for worktree and branch operations. Must support `git worktree` (v2.5+).

Both are expected to be on PATH. All tests run normally, including tmux-dependent ones.

### Git
- Call via `std::process::Command::new("git")`
- Always capture stderr for error messages
- Parse stdout for branch lists, worktree info
- Run `git worktree prune` before creating new worktrees

### Tmux
- Call via `std::process::Command::new("tmux")`
- Builder pattern: accumulate ops, execute or return as strings (for testing/dry-run)
- Session names: `paw-<project-name>`
- Use `-c` flag on `new-session` to set pane 0's working directory
- **Critical: apply `tiled` layout before each new split**, not just at the end
- Apply final `tiled` layout after all panes for clean alignment
- Enable `mouse on` per-session (not globally)
- Set pane titles to `<branch> → <cli>` via `select-pane -T`
- Enable `pane-border-status top` and `pane-border-format " #{pane_title} "` per-session

## Spec-Driven Development

This project uses OpenSpec-style specifications in `openspec/changes/`.

Specs use RFC 2119 keywords: **SHALL/MUST** (mandatory), **SHOULD** (recommended), **MAY** (optional).
Requirements include GIVEN/WHEN/THEN scenarios. Each scenario maps to at least one test.

## Testing Conventions

### Unit Tests
- In `#[cfg(test)] mod tests {}` at bottom of each module
- Every OpenSpec scenario maps to at least one test
- `tempfile` for filesystem tests
- No system side effects

### Integration Tests
- In `tests/` directory
- `assert_cmd` for CLI binary tests
- `predicates` for output assertions
- Tmux-dependent tests run normally (tmux is a hard dependency)

### Coverage
- Run: `just coverage`
- Target: >= 80% line coverage

## Documentation

### Four Layers
1. `--help` text — comprehensive with examples
2. README.md — landing page with badges, quick starts, CLI table
3. mdBook site — full user guide at `https://bearicorn.github.io/git-paw/`
4. `just api-docs` / Rustdoc — API docs for contributors

All layers must be consistent.

## Platform Support

- **macOS** (ARM + x86) — fully supported
- **Linux** (x86_64 + ARM64) — fully supported
- **Windows** — WSL only. Native Windows is not supported (tmux is Unix-only).

## Release & Distribution

Handled by cargo-dist. Config: `[workspace.metadata.dist]` in `Cargo.toml`.

- **Trigger:** push tag `v*`
- **Automatic:** cross-platform binaries, checksums, shell installer, Homebrew formula
- **Homebrew tap:** `bearicorn/homebrew-tap`

## Project Metadata

- License: MIT
- MSRV: current stable

## Commits

Commits should not include any reference to AI assistants. It should also be one clean linear commit. The commit should also resolve the issue that you are working on.
