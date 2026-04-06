## Context

git-paw v0.1.0 works but requires users to manually create `.git-paw/config.toml` and understand the directory layout. The `init` command solves this with a single bootstrapping step.

The existing codebase has `config.rs` (TOML parsing, merge logic, CLI management), `cli.rs` (clap subcommands), `error.rs` (PawError enum), and `main.rs` (dispatch). All four need minor additions. One new module (`init.rs`) contains the core logic.

## Goals / Non-Goals

**Goals:**
- One-command repo setup: `git paw init` creates everything needed
- Default config generation with v0.2.0 fields (`[specs]`, `[logging]`, `default_spec_cli`, `branch_prefix`)
- Gitignore management for `.git-paw/logs/`

**Non-Goals:**
- Per-worktree AGENTS.md generation (separate change: `worktree-agents-md`)
- Spec scanning or `--from-specs` flag (separate change: `spec-scanner`)
- Session logging implementation (separate change: `session-logging`)
- Interactive prompts during init (init is non-interactive, always succeeds)

## Decisions

### Decision 1: New `init.rs` module

`init.rs` owns the init command orchestration (directory creation, config generation, gitignore).

### Decision 2: Default config includes v0.2.0 fields as comments

The generated `config.toml` includes new v0.2.0 fields (`default_spec_cli`, `branch_prefix`, `[specs]`, `[logging]`) as commented-out examples, not as active values. This teaches users what's available without activating features they haven't opted into.

**Why:** Active defaults (like `[logging] enabled = false`) would be serialized by `PawConfig` and clutter the config. Comments are documentation that doesn't affect behavior.

**Alternative considered:** Generate a minimal config with only `default_cli` and `mouse`. Rejected because discoverability of v0.2.0 features matters for adoption.

### Decision 3: New PawError variants

Add `InitError(String)` for init-specific failures (directory creation, file writes).

**Why:** Existing error variants (`ConfigError`, `SessionError`) are semantically wrong for init operations. Specific variants produce better error messages.

### Decision 4: Config struct additions are additive and optional

New fields on `PawConfig` (`default_spec_cli`, `branch_prefix`, `specs: Option<SpecsConfig>`, `logging: Option<LoggingConfig>`) use `Option` with `serde(default)`. Existing configs parse without changes. The `merged_with()` method is extended to handle the new fields.

**Why:** Backward compatibility. Users with v0.1.0 configs should not see errors after upgrading.

## Risks / Trade-offs

**[Gitignore append race]** → If `.gitignore` doesn't end with a newline, appending `.git-paw/logs/` could merge with the last line. → Mitigation: read the file, check if it ends with `\n`, prepend one if not.

**[Marker collision]** → Another tool could theoretically use `<!-- git-paw:start -->`. → Extremely unlikely given the tool-specific prefix. No mitigation needed.

**[Stale worktree registrations]** → After `purge`, git retains worktree registrations for deleted directories. The `start` command (both interactive and `--from-specs`) must call `git worktree prune` before creating new worktrees. Added `prune_worktrees()` to `src/git.rs`.

**[Config field ownership across parallel branches]** → The `cli-selection` change also modifies `config.rs` to add `default_spec_cli`. → Mitigation: `init-command` adds the struct fields; `cli-selection` adds the behavioral logic. Both are additive and merge cleanly.
