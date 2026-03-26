## ADDED Requirements

### Requirement: Detect agent file state

The system SHALL detect which combination of `CLAUDE.md` and `AGENTS.md` exists in a directory, distinguishing between regular files and symlinks.

#### Scenario: Only CLAUDE.md exists
- **WHEN** a directory contains `CLAUDE.md` but no `AGENTS.md`
- **THEN** the detected state SHALL be `ClaudeMdOnly`

#### Scenario: Only AGENTS.md exists
- **WHEN** a directory contains `AGENTS.md` but no `CLAUDE.md`
- **THEN** the detected state SHALL be `AgentsMdOnly`

#### Scenario: Both files exist
- **WHEN** a directory contains both `CLAUDE.md` and `AGENTS.md`
- **THEN** the detected state SHALL be `BothExist`

#### Scenario: Neither file exists
- **WHEN** a directory contains neither `CLAUDE.md` nor `AGENTS.md`
- **THEN** the detected state SHALL be `NeitherExists`

#### Scenario: AGENTS.md is a symlink to CLAUDE.md
- **WHEN** `AGENTS.md` is a symlink pointing to `CLAUDE.md`
- **THEN** the detected state SHALL be `BothExist` (symlink counts as existing)

### Requirement: Config-driven CLAUDE.md mode

The system SHALL support a `claude_md` config field with values `"symlink"`, `"copy"`, or `"skip"`, defaulting to `"skip"` when absent.

#### Scenario: Config absent defaults to skip
- **WHEN** `claude_md` is not set in config
- **THEN** the mode SHALL default to `Skip`

#### Scenario: Config set to symlink
- **WHEN** `claude_md = "symlink"` is set in config
- **THEN** the mode SHALL be `Symlink`

#### Scenario: Config set to copy
- **WHEN** `claude_md = "copy"` is set in config
- **THEN** the mode SHALL be `Copy`

#### Scenario: Config set to skip
- **WHEN** `claude_md = "skip"` is set in config
- **THEN** the mode SHALL be `Skip`

### Requirement: Init prompts for CLAUDE.md mode

When `git paw init` detects that `CLAUDE.md` exists and no `claude_md` mode is configured, the system SHALL prompt the user to choose how to handle the relationship between CLAUDE.md and AGENTS.md.

#### Scenario: Init with CLAUDE.md only — user chooses symlink
- **WHEN** `git paw init` runs, only `CLAUDE.md` exists, no `claude_md` in config, and the user selects "Symlink"
- **THEN** `claude_md = "symlink"` SHALL be written to `.git-paw/config.toml`, the git-paw section SHALL be injected into `CLAUDE.md`, and `AGENTS.md` SHALL be created as a symlink to `CLAUDE.md`

#### Scenario: Init with CLAUDE.md only — user chooses copy
- **WHEN** `git paw init` runs, only `CLAUDE.md` exists, no `claude_md` in config, and the user selects "Copy"
- **THEN** `claude_md = "copy"` SHALL be written to `.git-paw/config.toml`, `CLAUDE.md` content SHALL be copied to `AGENTS.md`, and the git-paw section SHALL be injected into both files

#### Scenario: Init with CLAUDE.md only — user chooses skip
- **WHEN** `git paw init` runs, only `CLAUDE.md` exists, no `claude_md` in config, and the user selects "Skip"
- **THEN** `claude_md = "skip"` SHALL be written to `.git-paw/config.toml`, `AGENTS.md` SHALL be created with only the git-paw section, and `CLAUDE.md` SHALL have the git-paw section injected

#### Scenario: Init with both files — user chooses symlink
- **WHEN** `git paw init` runs, both files exist, no `claude_md` in config, and the user selects "Symlink"
- **THEN** `claude_md = "symlink"` SHALL be written to config and the git-paw section SHALL be injected into `AGENTS.md` only (no symlink created when both files already exist as regular files)

#### Scenario: Init with both files — user chooses copy
- **WHEN** `git paw init` runs, both files exist, no `claude_md` in config, and the user selects "Copy"
- **THEN** `claude_md = "copy"` SHALL be written to config and the git-paw section SHALL be injected into both files independently

#### Scenario: Init with both files — user chooses skip
- **WHEN** `git paw init` runs, both files exist, no `claude_md` in config, and the user selects "Skip"
- **THEN** `claude_md = "skip"` SHALL be written to config and the git-paw section SHALL be injected into `AGENTS.md` only

#### Scenario: Init without CLAUDE.md — no prompt
- **WHEN** `git paw init` runs and no `CLAUDE.md` exists
- **THEN** no CLAUDE.md mode prompt SHALL be shown

#### Scenario: Re-init with mode already in config — no prompt
- **WHEN** `git paw init` runs and `claude_md` is already set in `.git-paw/config.toml`
- **THEN** no CLAUDE.md mode prompt SHALL be shown and the existing config value SHALL be used

### Requirement: Root repo handling by mode

The system SHALL handle root AGENTS.md/CLAUDE.md according to the configured mode.

#### Scenario: Skip — AGENTS.md only
- **WHEN** mode is `Skip` and only `AGENTS.md` exists
- **THEN** the git-paw section SHALL be injected into `AGENTS.md`

#### Scenario: Skip — neither exists
- **WHEN** mode is `Skip` and neither file exists
- **THEN** `AGENTS.md` SHALL be created with the git-paw section

#### Scenario: Symlink — AGENTS.md already symlinked to CLAUDE.md
- **WHEN** `AGENTS.md` is already a symlink to `CLAUDE.md`
- **THEN** the git-paw section SHALL be injected into `CLAUDE.md` (the symlink target) and the symlink SHALL be preserved

### Requirement: Worktree CLAUDE.md handling

The system SHALL ensure Claude Code can read worktree instructions when the pane CLI is `claude`.

#### Scenario: Symlink mode — worktree with claude CLI
- **WHEN** mode is `Symlink` and CLI is `"claude"`
- **THEN** `CLAUDE.md` SHALL be created in the worktree as a symlink to `AGENTS.md`

#### Scenario: Copy mode — worktree with claude CLI
- **WHEN** mode is `Copy` and CLI is `"claude"`
- **THEN** `CLAUDE.md` SHALL be created in the worktree as a copy of the worktree's `AGENTS.md` content

#### Scenario: Skip mode — worktree with claude CLI
- **WHEN** mode is `Skip` and CLI is `"claude"`
- **THEN** `CLAUDE.md` SHALL be created in the worktree with only the git-paw assignment section

#### Scenario: Worktree with non-claude CLI
- **WHEN** CLI is not `"claude"`
- **THEN** no `CLAUDE.md` SHALL be created in the worktree regardless of mode

#### Scenario: Worktree CLAUDE.md already exists
- **WHEN** `CLAUDE.md` already exists in the worktree (as regular file or symlink)
- **THEN** the existing file SHALL NOT be overwritten

### Requirement: Symlink safety

The system SHALL check for existing files and symlinks before creating new symlinks.

#### Scenario: Target path already exists as regular file
- **WHEN** a symlink would overwrite an existing regular file
- **THEN** the symlink SHALL NOT be created

#### Scenario: Target is already correct symlink
- **WHEN** a symlink already points to the intended target
- **THEN** no new symlink SHALL be created (idempotent)

#### Scenario: Symlink creation failure
- **WHEN** symlink creation fails (e.g., permission error)
- **THEN** the system SHALL return `PawError::AgentsMdError` with context about the failure

### Requirement: Idempotent across file state transitions

The system SHALL produce correct results regardless of what state existed before.

#### Scenario: Init after CLAUDE.md added to repo
- **WHEN** init was previously run (creating AGENTS.md), then the user adds CLAUDE.md, then init runs again
- **THEN** the system SHALL detect the current file state and prompt for mode if not yet configured

#### Scenario: Init after AGENTS.md symlink replaced with file
- **WHEN** init previously created AGENTS.md as a symlink, then the user replaces it with a regular file, then init runs again
- **THEN** the system SHALL detect the current state and handle correctly without errors
