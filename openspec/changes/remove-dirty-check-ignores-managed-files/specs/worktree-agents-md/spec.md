## MODIFIED Requirements

### Requirement: Exclude worktree AGENTS.md from git

The system SHALL add the sidecar instruction file path (e.g. `.git-paw/AGENTS.local.md`) to the worktree's ignore set (`.git/info/exclude` or `.gitignore`) to prevent accidental commits of the ephemeral injection. The system SHALL NOT add the tracked `AGENTS.md` to the worktree's `.git/info/exclude`.

The system SHALL add the sidecar path to the worktree's ignore set BEFORE writing the sidecar instruction file to disk, so the file is excluded from `git status` the instant it lands. This closes the write-then-exclude race in which a `git status --porcelain` issued between the sidecar write and the exclude registration would report the injected sidecar as an untracked file (the v0.8.0 regression that made `git paw remove` refuse a just-started clean worktree).

#### Scenario: Sidecar exclude entry added
- **WHEN** worktree setup runs for a worktree
- **THEN** the sidecar instruction file path SHALL appear in the worktree's ignore set

#### Scenario: Exclude entry already present
- **WHEN** the worktree's ignore set already contains the sidecar path
- **THEN** the entry SHALL NOT be duplicated

#### Scenario: Tracked AGENTS.md is not excluded
- **WHEN** worktree setup completes
- **THEN** `AGENTS.md` SHALL NOT appear in the worktree's `.git/info/exclude` as a result of git-paw setup

#### Scenario: Stale assume-unchanged bit cleared on start
- **WHEN** a worktree's tracked `AGENTS.md` carries an `assume-unchanged` bit set by a prior git-paw version
- **THEN** the next worktree setup SHALL clear it (`git update-index --no-assume-unchanged AGENTS.md`) so the tracked file becomes committable

#### Scenario: Sidecar is excluded the moment it is written
- **GIVEN** a freshly created worktree whose ignore set does not yet contain the sidecar path
- **WHEN** `setup_worktree_agents_md()` runs to completion
- **THEN** the sidecar exclude entry SHALL have been registered before the sidecar file was written
- **AND** a `git status --porcelain` run in the worktree immediately after setup SHALL NOT report the sidecar instruction file as an untracked or modified entry
