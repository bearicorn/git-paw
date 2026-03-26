## Context

git-paw v0.1.0 works but requires users to manually create `.git-paw/config.toml` and understand the directory layout. There is no `AGENTS.md` integration — AI coding CLIs have no awareness of git-paw's presence in a repo. The `init` command solves both problems with a single bootstrapping step.

The existing codebase has `config.rs` (TOML parsing, merge logic, CLI management), `cli.rs` (clap subcommands), `error.rs` (PawError enum), and `main.rs` (dispatch). All four need minor additions. Two new modules (`init.rs`, `agents.rs`) contain the core logic.

## Goals / Non-Goals

**Goals:**
- One-command repo setup: `git paw init` creates everything needed
- AGENTS.md injection with idempotent markers so agents discover git-paw automatically
- CLAUDE.md compatibility for repos already using Claude Code
- Default config generation with v0.2.0 fields (`[specs]`, `[logging]`, `default_spec_cli`, `branch_prefix`)
- Gitignore management for `.git-paw/logs/`

**Non-Goals:**
- Per-worktree AGENTS.md generation (separate change: `worktree-agents-md`)
- Spec scanning or `--from-specs` flag (separate change: `spec-scanner`)
- Session logging implementation (separate change: `session-logging`)
- Interactive prompts during init (init is non-interactive, always succeeds)

## Decisions

### Decision 1: Two new modules — `init.rs` and `agents.rs`

`init.rs` owns the init command orchestration (directory creation, config generation, gitignore). `agents.rs` owns all AGENTS.md read/write/inject logic including CLAUDE.md compatibility.

**Why separate:** `agents.rs` is reused by the `worktree-agents-md` change (Wave 2) for per-worktree AGENTS.md generation. Coupling it to `init.rs` would force that change to depend on init internals.

**Alternative considered:** Single `init.rs` module with everything. Rejected because worktree AGENTS.md generation needs the same marker-based injection logic but in a different context.

### Decision 2: Marker-based injection for idempotency

Use HTML comment markers `<!-- git-paw:start -->` and `<!-- git-paw:end -->` to delimit the injected section. On re-run, check for the start marker — if present, replace the existing block; if absent, append.

**Why:** Markers are invisible in rendered markdown, work with any markdown parser, and make the injected section machine-identifiable for updates. This is the same pattern used by tools like Terraform (for `.gitignore`), Homebrew, and other CLI bootstrappers.

**Alternative considered:** Separate file (e.g., `.git-paw/agents-instructions.md`) instead of injecting into AGENTS.md. Rejected because AI CLIs look for `AGENTS.md` at repo root automatically — a separate file would require per-CLI configuration.

### Decision 3: CLAUDE.md symlink strategy

When a repo has `CLAUDE.md` but no `AGENTS.md`:
1. Append the git-paw section to `CLAUDE.md` (so Claude Code sees it)
2. Create `AGENTS.md` as a symlink → `CLAUDE.md` (so other CLIs see it too)

When both exist: append to `AGENTS.md` only (CLAUDE.md is the user's domain).
When neither exists: create `AGENTS.md` with the git-paw section.

**Why:** Claude Code reads `CLAUDE.md`, other CLIs read `AGENTS.md`. The symlink ensures both point to the same content without duplication. Appending to the existing file preserves user content.

**Alternative considered:** Always create both files independently. Rejected because content drift between the two files would cause confusion.

### Decision 4: Default config includes v0.2.0 fields as comments

The generated `config.toml` includes new v0.2.0 fields (`default_spec_cli`, `branch_prefix`, `[specs]`, `[logging]`) as commented-out examples, not as active values. This teaches users what's available without activating features they haven't opted into.

**Why:** Active defaults (like `[logging] enabled = false`) would be serialized by `PawConfig` and clutter the config. Comments are documentation that doesn't affect behavior.

**Alternative considered:** Generate a minimal config with only `default_cli` and `mouse`. Rejected because discoverability of v0.2.0 features matters for adoption.

### Decision 5: New PawError variants

Add `InitError(String)` for init-specific failures (directory creation, file writes) and `AgentsMdError(String)` for AGENTS.md operations (read/write/symlink failures).

**Why:** Existing error variants (`ConfigError`, `SessionError`) are semantically wrong for init operations. Specific variants produce better error messages.

### Decision 6: Config struct additions are additive and optional

New fields on `PawConfig` (`default_spec_cli`, `branch_prefix`, `specs: Option<SpecsConfig>`, `logging: Option<LoggingConfig>`) use `Option` with `serde(default)`. Existing configs parse without changes. The `merged_with()` method is extended to handle the new fields.

**Why:** Backward compatibility. Users with v0.1.0 configs should not see errors after upgrading.

## Risks / Trade-offs

**[Symlink portability]** → Symlinks work on macOS/Linux but have edge cases on Windows (even WSL). Since git-paw is WSL-only on Windows and tmux is Unix-only, this is acceptable. Document the WSL requirement.

**[Gitignore append race]** → If `.gitignore` doesn't end with a newline, appending `.git-paw/logs/` could merge with the last line. → Mitigation: read the file, check if it ends with `\n`, prepend one if not.

**[Marker collision]** → Another tool could theoretically use `<!-- git-paw:start -->`. → Extremely unlikely given the tool-specific prefix. No mitigation needed.

**[Config field ownership across parallel branches]** → The `cli-selection` change also modifies `config.rs` to add `default_spec_cli`. → Mitigation: `init-command` adds the struct fields; `cli-selection` adds the behavioral logic. Both are additive and merge cleanly.
