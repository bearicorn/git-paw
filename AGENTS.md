# AGENTS.md — git-paw

## Project Overview

**git-paw** (Parallel AI Worktrees) is a Rust CLI tool that orchestrates multiple AI coding CLI sessions across git worktrees from a single terminal using tmux.

Repository: `bearicorn/git-paw`
Crate: `git-paw`
Binary: `git-paw` (invokable as `git paw` via git subcommand convention)

## General Workflow

This project follows a spec-driven development approach where all changes must be defined in OpenSpec format before implementation. The AGENTS.md file describes the general workflow and standards that apply to all changes, regardless of specific features.

## Behavioral Guidelines

Behavioral guidelines to reduce common LLM coding mistakes. Merge with the project-specific instructions above and below as needed.

**Tradeoff:** these guidelines bias toward caution over speed. For trivial tasks, use judgment.

### 1. Think Before Coding

Don't assume. Don't hide confusion. Surface tradeoffs. Before implementing:

- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them — don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

### 2. Simplicity First

Minimum code that solves the problem. Nothing speculative.

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

The test: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

### 3. Surgical Changes

Touch only what you must. Clean up only your own mess. When editing existing code:

- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it — don't delete it.

When your changes create orphans:

- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: every changed line should trace directly to the user's request.

### 4. Goal-Driven Execution

Define success criteria. Loop until verified. Transform tasks into verifiable goals:

- "Add validation" → "write tests for invalid inputs, then make them pass"
- "Fix the bug" → "write a test that reproduces it, then make it pass"
- "Refactor X" → "ensure tests pass before and after"

For multi-step tasks, state a brief plan with a verification check per step:

```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

These guidelines are working if: fewer unnecessary changes in diffs, fewer rewrites due to overcomplication, and clarifying questions come before implementation rather than after mistakes.

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
Scopes: `detect`, `git`, `tmux`, `session`, `config`, `interactive`, `error`, `cli`, `docs`, `ci`, `specs`, `agents`, `logging`, `replay`, `init`, `broker`, `dashboard`, `skills`, `supervisor`, `merge-loop`, `user-guide`, `worktree`, `governance`, `learnings`, `pause`

Compound scopes are written as `(scope1,scope2,...)` when a single change cuts across multiple scopes. Example: `feat(cli,config): add new flag with config wiring`.

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
| `tokio` | Async runtime (broker HTTP server) |
| `axum` | HTTP server framework (broker endpoints) |
| `ratatui` | TUI framework (dashboard) |
| `crossterm` | Terminal backend for ratatui |
| `schemars` | JSON Schema derivation for governance config |
| `serde_yaml` | Spec Kit frontmatter parsing |
| `chrono` | ISO timestamp formatting in broker messages and learnings aggregator |
| `regex` | Broker `agent_id` validation + supervisor `sweep.sh` phantom filter |
| `rmcp` v1.7 | Official MCP Rust SDK (Apache-2.0) — stdio MCP server for `git paw mcp`; needs active upstream version tracking (protocol churn) |

Dev: `assert_cmd`, `predicates`, `tempfile`, `serial_test`, `tower`, `hyper`, `hyper-util`, `http-body-util`

Do not add other dependencies without explicit approval.

#### Notable exclusions

- `dirs` — Replaced by homegrown `src/dirs.rs` because the upstream crate's license is not FOSS-compatible. Do not re-add.

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

### opsx skills are the canonical interface

Agents MUST drive the OpenSpec workflow through the `opsx:*` slash-command skills, not hand-rolled file writes:

| Stage | Skill | When to invoke |
|---|---|---|
| Start a new change | `/opsx:new <kebab-name>` | Creating a new change directory + scaffold |
| Create next artifact (proposal → specs → design → tasks) | `/opsx:continue <change>` | After the previous artifact lands; ONE artifact per invocation |
| Generate all artifacts in one pass | `/opsx:ff <change>` | When you know the change shape up-front and want to skip the per-artifact prompt cycle |
| Implement tasks | `/opsx:apply <change>` | After tasks.md is complete; walks tasks one at a time and marks `- [ ]` → `- [x]` |
| Verify before archive | `/opsx:verify <change>` | **Supervisor-only.** Coding agents do NOT invoke this — verification is supervisor responsibility (five-gate framework). |
| Archive a complete change | `/opsx:archive <change>` | **Supervisor-only**, post-cherry-pick on the release branch. Coding agents do NOT invoke this. |

Direct file writes to `openspec/changes/<change>/{proposal,design,tasks}.md` or `specs/<capability>/spec.md` are reserved for amendments to an already-validated change (e.g. folding new findings into an in-flight change) — they SHALL NOT be the primary authoring path. When in doubt, run `/opsx:continue` and let the skill prompt.

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
- **Manual:** `cargo publish` to crates.io is **not** wired into cargo-dist
  (`dist-workspace.toml` lists only `publish-jobs = ["homebrew"]`). The
  maintainer runs `cargo publish` locally after the tag — see step 7 below.

### Cutting a release

The release flow assumes **every OpenSpec change for this release has
already been applied + archived** on the feature branch (or on `main`
if changes landed continuously). The `chore: prepare vX.Y.Z release`
commit only bumps the version, refreshes `Cargo.lock`, and regenerates
the changelog. Archives are NOT part of the prep commit.

Rationale (changed during v0.5.0 cycle): the v0.2.0-v0.4.0 prep
commits bundled archive moves with the version bump. This made the
commit reviewable as a single release-readiness checkpoint but had
real downsides — archive moves at release time meant validation errors
surfaced under deadline pressure; the commit body had to list every
archived change, which got long; and it gave the false impression that
the work being released was happening now (it was not, it had landed
incrementally). v0.5.0 onwards: archive each change as part of its
own merge into the release branch.

1. **Merge each feature branch into `feat/vX.Y.0-*` (or `main`)** with
   the change's implementation commits. Then in the SAME branch
   immediately archive the change:

   ```bash
   openspec archive <change-name> -y
   git add openspec/
   git commit -m "chore(specs): archive <change-name>; sync deltas to main specs"
   ```

   The archive commit is a sibling of the implementation commits and
   lands well before release prep. If a delta references a requirement
   that doesn't exist in the target spec (or duplicates one), fix the
   delta header (`## ADDED Requirements` vs `## MODIFIED Requirements`)
   before re-running the archive. As a last resort,
   `openspec archive <change> -y --skip-specs` archives the change
   without touching main specs — only use when the implementation is
   already in code and the spec content is informational.

2. **Verify the archive backlog is empty** before starting release
   prep: `openspec list` SHALL show no pending changes for this
   release. If pending changes remain that you intentionally want to
   defer (e.g. spec-only changes that will land in vX.Y.Z+1), move
   their directories out of `openspec/changes/` (or document the
   deferral in `MILESTONE.md`'s v0.5.0 implementation-status table).

3. **Bump the version** in `Cargo.toml`, then `cargo build` to refresh
   `Cargo.lock`.

4. **Regenerate the changelog** with `git cliff`:

   ```bash
   just changelog vX.Y.Z   # writes CHANGELOG.md
   ```

   The justfile recipe expands to
   `git cliff --tag vX.Y.Z -o CHANGELOG.md`. The new section appears
   under a `## [X.Y.Z] - YYYY-MM-DD` header at the top.

5. **One commit captures version + changelog only** (NOT archives):

   ```bash
   git add Cargo.toml Cargo.lock CHANGELOG.md
   git commit -m "chore: prepare vX.Y.Z release

   Bump version to X.Y.Z. The changelog summarises what was archived
   into openspec/specs/ across this cycle."
   ```

   `openspec/` SHALL NOT appear in this commit's diff. If `git status`
   shows pending `openspec/specs/` changes here, an archive step was
   skipped earlier — back up and archive it as its own commit before
   continuing.

6. **Open a PR, merge to `main`, then tag and push**:

   Push the release branch and open a PR into `main`. The four CI gates
   (fmt, clippy, deny, audit) must pass and the PR should be reviewed
   before merging. Prefer a fast-forward or rebase merge so the
   `chore: prepare vX.Y.Z release` commit becomes `main`'s tip; a merge
   commit also works as long as `main` ends up containing the prep
   commit. (Releases through v0.5.0 pushed `main` directly without a
   PR — the PR adds a CI + review gate before `main` moves. The PR is
   the default from v0.6.0 onward; a direct push is still acceptable
   for a hotfix.)

   ```bash
   # after the PR is merged and main is updated locally:
   git checkout main && git pull
   git tag vX.Y.Z
   git push origin main vX.Y.Z
   ```

   Pushing the tag triggers cargo-dist on GitHub Actions, which builds
   cross-platform binaries, publishes the release, and updates the
   Homebrew tap. Do **not** push the tag before `main` includes the
   prep commit — the tag MUST be on a `main` commit that carries the
   version bump, or cargo-dist sees a mismatched manifest version and
   the release fails.

7. **Publish to crates.io** (manual — not part of cargo-dist):

   ```bash
   cargo publish --dry-run   # verify package contents
   cargo publish             # upload vX.Y.Z to crates.io
   ```

   Requires `cargo login` with a token from
   `https://crates.io/settings/tokens` (one-time per machine). The
   working tree should be clean and on the prep commit; cargo runs its
   own packaging build, so a stale `target/release/` is fine.

   Publishing is **irreversible**: a published version can only be
   yanked, never deleted, and the same `vX.Y.Z` can never be re-uploaded.
   Always run `--dry-run` first.

8. **Verify** the release on every channel:
   - GitHub: `https://github.com/bearicorn/git-paw/releases` shows the
     new tag with binaries for `aarch64-apple-darwin`,
     `x86_64-apple-darwin`, `aarch64-unknown-linux-gnu`,
     `x86_64-unknown-linux-gnu`, plus checksums and the shell installer.
   - Homebrew tap: `bearicorn/homebrew-tap` has a commit bumping the
     formula to `vX.Y.Z`.
   - crates.io: `https://crates.io/crates/git-paw` shows `vX.Y.Z` as
     `max_version`.

   Then sanity-check each install path resolves to the new version:

   ```bash
   cargo install git-paw                      # crates.io
   curl --proto '=https' --tlsv1.2 -LsSf \
     https://github.com/bearicorn/git-paw/releases/latest/download/git-paw-installer.sh | sh
   brew install bearicorn/tap/git-paw         # Homebrew
   git-paw --version                          # should print X.Y.Z
   ```

If the prep commit needs to be amended (e.g. a missed archive, a typo in
the changelog), do it **before** tagging. Once `vX.Y.Z` is pushed,
treat it as immutable: ship a `vX.Y.Z+1` follow-up rather than
re-tagging. The same applies to `cargo publish` — a botched upload
means cutting `vX.Y.Z+1`, not re-publishing `vX.Y.Z`.

### Historical archives are gitignored, canonical state lives in main specs

`openspec/changes/archive/` and `openspec/changes/_release-notes/` are
**gitignored** from v0.5.0 onwards. The canonical post-archive state
lives in `openspec/specs/` (updated as part of each `openspec archive`
run's delta application). The per-change archive directories carry
only the original proposal/design/tasks — useful as local audit trail
during a cycle but redundant with the merged main specs once the
deltas are in place.

Each developer's local archive view is regenerated by running
`openspec archive <change>` during the cycle. The directories are
not shared via git, so each developer can have a slightly different
local archive view depending on which changes they have archived
locally — this is acceptable because the authoritative state is in
`openspec/specs/`.

The release-prep commit (`chore: prepare vX.Y.Z release`) does not
need to delete these directories any more — they were never tracked
in the first place under the v0.5.0+ convention.

Rationale for the v0.5.0 policy change: tracking-then-pruning meant
the release-prep commit had to list every archived change in its
body (got long), and reviewers saw thousands of archive files in
each release PR even though the deltas were already merged into the
main specs incrementally. Gitignoring the archive directory removes
that churn without losing any information — the merged specs in
`openspec/specs/` and the implementation commits (one per change
scope, per the release commit-shape convention) together capture
the full release.

If you need to refer back to a prior release's archive content:
- During the cycle: check your local `openspec/changes/archive/`
  (it's gitignored but not deleted).
- Post-release: run `openspec archive --list` against an earlier
  checkout, or re-apply the delta locally from the change branch.

## Project Metadata

- License: MIT
- MSRV: current stable

## Commits

Commits should not include any reference to AI assistants. It should also be one clean linear commit. The commit should also resolve the issue that you are working on.

**Every commit must be buildable and releasable.** `just check` must pass at each commit. Do not commit code that breaks the build, fails tests, or deviates from specs with the intent to "fix it later." If your implementation doesn't match the spec, fix it before committing — or update the spec first if the deviation is intentional.

**Match specs exactly.** Field names, function signatures, and wire formats must match the OpenSpec requirements precisely. If the spec says `exports: Vec<String>`, use that name. Read the spec before coding, not after.

## MCP
When you need to search docs, use `context7` tools.


