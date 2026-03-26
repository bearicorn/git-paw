## Why

git-paw v0.1.0 requires manual setup — users must create config files and understand the project structure before launching sessions. There is no single command to bootstrap a repo for git-paw use. `git paw init` provides a one-command onboarding experience that creates the `.git-paw/` directory, generates a default config, sets up gitignore rules, and injects a git-paw section into the project's `AGENTS.md` so that AI coding CLIs are aware of git-paw from the start.

## What Changes

- New `init` subcommand added to the CLI (`git paw init`)
- Creates `.git-paw/` directory with a default `config.toml`
- Creates `.git-paw/logs/` directory for session logging
- Appends `.git-paw/logs/` to the repo's `.gitignore`
- Injects a git-paw section into root `AGENTS.md` using `<!-- git-paw:start -->` / `<!-- git-paw:end -->` markers
  - If `AGENTS.md` exists → appends the section
  - If `AGENTS.md` does not exist → creates it with the section
- Handles `CLAUDE.md` compatibility:
  - If repo has `CLAUDE.md` but no `AGENTS.md` → creates `AGENTS.md` as symlink to `CLAUDE.md`, appends git-paw section to `CLAUDE.md`
  - If repo has both → appends to `AGENTS.md` only
  - If repo has neither → creates `AGENTS.md` with git-paw section
- Idempotent: running `init` twice does not duplicate the injected section (markers prevent re-injection)

## Capabilities

### New Capabilities
- `project-initialization`: Bootstrapping `.git-paw/` directory, default config generation, gitignore setup, and idempotency checks
- `agents-md-injection`: Reading, creating, and appending to `AGENTS.md` with `<!-- git-paw:start/end -->` markers, including `CLAUDE.md` symlink/merge handling

### Modified Capabilities
- `cli-parsing`: New `Init` subcommand variant added to the `Command` enum
- `configuration`: New config fields for v0.2.0 (`default_spec_cli`, `branch_prefix`, `[specs]` section, `[logging]` section)

## Impact

- **New files**: `src/init.rs` (init command logic), `src/agents.rs` (AGENTS.md generation and injection)
- **Modified files**: `src/cli.rs` (new `Init` subcommand), `src/main.rs` (wire init), `src/config.rs` (new config fields), `src/error.rs` (new error variants)
- **No new dependencies** — uses only `std::fs`, `std::path`, and existing `serde`/`toml` for config generation
- **No breaking changes** to existing CLI commands or config format (new fields are additive and optional)
