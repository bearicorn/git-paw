# AGENTS.md — git-paw

## Project Overview

**git-paw** (Parallel AI Worktrees) is a Rust CLI tool that orchestrates multiple AI coding CLI sessions across git worktrees from a single terminal using tmux.

Repository: `bearicorn/git-paw`
Crate: `git-paw`
Binary: `git-paw` (invokable as `git paw` via git subcommand convention)

## General Workflow

This project follows a spec-driven development approach where all changes must be defined in OpenSpec format before implementation. The AGENTS.md file describes the general workflow and standards that apply to all changes, regardless of specific features.

## Project Structure

The project follows a modular architecture with clear separation of concerns. Detailed architecture documentation can be found in the technical documentation.

## Development Tools

The project uses standard Rust development tools along with additional quality assurance tools. Refer to CONTRIBUTING.md for detailed setup instructions and development workflows.

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
Scopes: `detect`, `git`, `tmux`, `session`, `config`, `interactive`, `error`, `cli`, `docs`, `ci`, `specs`, `agents`, `logging`, `replay`, `init`, `broker`, `dashboard`, `skills`, `supervisor`, `merge-loop`

Examples:
```
feat(specs): add spec scanning and discovery module
fix(git): prune stale worktree registrations
test(e2e): add integration tests for init and replay
docs(readme): add quick start section
```

**Commit message rules:**
- Do not reference TODO.md, MILESTONE.md, or other project management files
- Focus on the technical change, not the task tracking
- Reference specifications and requirements directly (e.g., "Implements openspec/specs/dashboard/spec.md:239")
- Keep messages concise and technical

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
- **All tests must be behavioral** — test observable inputs/outputs and public API contracts, not internal implementation details. Do not test private struct field values, internal function calls, or module-private state.
- **Every OpenSpec scenario maps to at least one test** — if a spec requirement has a WHEN/THEN scenario, there must be a corresponding test asserting that behavior
- **E2E tests required for cross-module features** — any feature that spans multiple modules (e.g. publish → delivery → poll → HTTP response) must have an integration test exercising the full flow

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
| `tokio` | Async runtime (broker HTTP server) |
| `axum` | HTTP server framework (broker endpoints) |
| `ratatui` | TUI framework (dashboard) |
| `crossterm` | Terminal backend for ratatui |

Dev: `assert_cmd`, `predicates`, `tempfile`, `serial_test`, `tower`, `hyper`, `hyper-util`, `http-body-util`

Do not add other dependencies without explicit approval.

## Configuration

Project configuration follows standard patterns with a main configuration file. Refer to the technical documentation for specific configuration options and their purposes.

## External Dependencies

The project has external tool dependencies that are required for core functionality. These tools must be available on the system PATH for the application to work properly.

### Tool Integration

External tools are integrated using standard process invocation patterns. Error handling and output parsing follow consistent conventions throughout the codebase.

## Change Checklist

Every change (feature, fix, refactor) must complete ALL of the following before it is considered done. This applies whether the work is done by a human or an AI agent.

### 1. Specs updated
- If the change adds new behavior: create or update OpenSpec specs under `openspec/changes/` or `openspec/specs/`
- If the change modifies existing behavior: write a MODIFIED requirement in a delta spec referencing the exact existing requirement name
- Every requirement must have at least one WHEN/THEN scenario

### 2. Implementation matches specs
- Every SHALL/MUST requirement in the spec is implemented
- No behavior exists that contradicts a spec requirement
- If the implementation deviates from the spec, update the spec first

### 3. Tests are behavioral
- Every spec scenario has a corresponding test
- Tests assert observable behavior (inputs → outputs, error conditions, public API contracts)
- Tests do NOT assert implementation details (private field values, internal function calls, mock interactions)
- Cross-module features have E2E integration tests exercising the full flow (e.g. HTTP request → internal routing → HTTP response)

### 4. Docs updated
- `--help` text updated if CLI surface changed
- README.md updated if user-facing features added
- mdBook chapters updated or created (`docs/src/`)
- Configuration reference updated if config fields added
- Architecture docs updated if module structure changed
- `mdbook build docs/` must succeed

### 5. Quality gates pass
- `just check` — fmt + clippy + all tests
- `just deny` — license/advisory/duplicate-dep checks
- No `unwrap()`/`expect()` in non-test code
- All public items have doc comments
- Coverage >= 80% on logic (TUI draw loops exempt)

### 6. Backward compatibility preserved
- New optional fields use `#[serde(default)]` and `skip_serializing_if`
- Existing v0.2.0 configs/sessions load without error
- When a feature is disabled (e.g. `[broker] enabled = false`), behavior is identical to the previous version
- Existing tests pass unchanged

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
- Each unit test MUST test behavior and not implmentation

### Integration Tests
- In `tests/` directory
- `assert_cmd` for CLI binary tests
- `predicates` for output assertions
- Tmux-dependent tests run normally (tmux is a hard dependency)
- E2E tests required for cross-module features (HTTP round-trips, session lifecycle, etc.)

### Coverage
- Run: `just coverage`
- Target: >= 80% line coverage
- TUI draw loops and terminal I/O exempt from coverage gate (tested manually via smoke tests)

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

### Cutting a release

The release flow follows a single `chore: prepare vX.Y.Z release` commit
on `main`, mirroring the v0.2.0, v0.3.0, and v0.4.0 prep commits.

1. **Merge the feature branch into `main`** (rebase-merge or fast-forward
   so the per-commit history is preserved).

2. **Archive completed OpenSpec changes** in dependency order. The
   `feat/vX.Y.0-*` branch should ship a
   `openspec/changes/_release-notes/vX.Y.0-archive-order.md` plan
   identifying the safe archive sequence. For each change:

   ```bash
   openspec archive <change-name> -y
   ```

   When a delta references a requirement that doesn't exist in the
   target spec (or duplicates one), fix the delta header
   (`## ADDED Requirements` vs `## MODIFIED Requirements`) before
   re-running the archive. As a last resort,
   `openspec archive <change> -y --skip-specs` archives the change
   without touching main specs — only use when the implementation is
   already in code and the spec content is informational.

3. **Bump the version** in `Cargo.toml`, then `cargo build` to refresh
   `Cargo.lock`.

4. **Regenerate the changelog** with `git cliff`:

   ```bash
   just changelog vX.Y.Z   # writes CHANGELOG.md
   ```

   The justfile recipe expands to
   `git cliff --tag vX.Y.Z -o CHANGELOG.md`. The new section appears
   under a `## [X.Y.Z] - YYYY-MM-DD` header at the top.

5. **One commit captures the whole release prep**:

   ```bash
   git add Cargo.toml Cargo.lock CHANGELOG.md openspec/
   git commit -m "chore: prepare vX.Y.Z release

   Bump version to X.Y.Z. Archive N OpenSpec changes and sync delta
   specs to main specs:
   - <list of capabilities>"
   ```

   Do **not** split this into separate "bump", "changelog", "archive"
   commits — the changelog should describe the contents of the release,
   the archive moves are part of "what shipped in vX.Y.Z", and reviewers
   read this commit as a single release-readiness checkpoint.

6. **Tag and push**:

   ```bash
   git tag vX.Y.Z
   git push origin main vX.Y.Z
   ```

   Pushing the tag triggers cargo-dist on GitHub Actions, which builds
   cross-platform binaries, publishes the release, and updates the
   Homebrew tap. Do **not** push the tag separately from `main`; if
   `main` doesn't include the prep commit yet, cargo-dist sees a
   mismatched manifest version and the release fails.

7. **Verify** the release at `https://github.com/bearicorn/git-paw/releases`
   and the published Homebrew formula at `bearicorn/homebrew-tap`.

If the prep commit needs to be amended (e.g. a missed archive, a typo in
the changelog), do it **before** tagging. Once `vX.Y.Z` is pushed,
treat it as immutable: ship a `vX.Y.Z+1` follow-up rather than
re-tagging.

### Historical archives are pruned at release time, not gitignored

`openspec/changes/archive/` and `openspec/changes/_release-notes/` are
**tracked** during a release cycle so contributors share the same
archive state and review the planning docs together — gitignoring them
would silently diverge each contributor's local archive view.

The deletion happens **only at release-prep time**, as part of the
`chore: prepare vX.Y.Z release` commit. After that commit, the canonical
post-archive state lives in `openspec/specs/`, the prep-commit body
lists which changes were archived, and the next development cycle starts
with a clean `openspec/changes/` directory. New `openspec archive`
runs during the next cycle re-create `archive/` (and `_release-notes/`
if you write a new plan) and those new files **are** committed and
shared with the team — the cycle repeats.

If you need to refer back to a prior release's archive content, check
out the relevant `vX.Y.Z` tag's parent commit — the archive is in the
git history before the prep commit pruned it.

## Project Metadata

- License: MIT
- MSRV: current stable

## Commits

Commits should not include any reference to AI assistants. It should also be one clean linear commit. The commit should also resolve the issue that you are working on.

**Every commit must be buildable and releasable.** `just check` must pass at each commit. Do not commit code that breaks the build, fails tests, or deviates from specs with the intent to "fix it later." If your implementation doesn't match the spec, fix it before committing — or update the spec first if the deviation is intentional.

**Match specs exactly.** Field names, function signatures, and wire formats must match the OpenSpec requirements precisely. If the spec says `exports: Vec<String>`, use that name. Read the spec before coding, not after.

## MCP
When you need to search docs, use `context7` tools.

