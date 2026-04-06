## Why

git-paw v0.1.0 requires manual setup — users must create config files and understand the project structure before launching sessions. There is no single command to bootstrap a repo for git-paw use. `git paw init` provides a one-command onboarding experience that creates the `.git-paw/` directory, generates a default config, and sets up gitignore rules.

## What Changes

- New `init` subcommand added to the CLI (`git paw init`)
- Creates `.git-paw/` directory with a default `config.toml`
- Creates `.git-paw/logs/` directory for session logging
- Appends `.git-paw/logs/` to the repo's `.gitignore`
- Idempotent: running `init` twice produces the same result as running it once

## Capabilities

### New Capabilities
- `project-initialization`: Bootstrapping `.git-paw/` directory, default config generation, gitignore setup, and idempotency checks

### Modified Capabilities
- `cli-parsing`: New `Init` subcommand variant added to the `Command` enum
- `configuration`: New config fields for v0.2.0 (`default_spec_cli`, `branch_prefix`, `[specs]` section, `[logging]` section)

## Impact

- **New files**: `src/init.rs` (init command logic)
- **Modified files**: `src/cli.rs` (new `Init` subcommand), `src/main.rs` (wire init), `src/config.rs` (new config fields), `src/error.rs` (new error variants)
- **No new dependencies** — uses only `std::fs`, `std::path`, and existing `serde`/`toml` for config generation
- **No breaking changes** to existing CLI commands or config format (new fields are additive and optional)
