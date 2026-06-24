## MODIFIED Requirements

### Requirement: Combine root content with worktree assignment

The system SHALL read the root repo's AGENTS.md and append the worktree assignment section to produce the worktree's effective agent-instruction view. This combined content SHALL be written to a gitignored sidecar instruction file (e.g. `.git-paw/AGENTS.local.md`), NOT the worktree's tracked `AGENTS.md`. The agent's effective instruction view SHALL equal the tracked `AGENTS.md` content followed by the managed git-paw block, and the CLI's instruction file SHALL be pointed at this combined sidecar view.

#### Scenario: Root AGENTS.md exists
- **WHEN** `setup_worktree_agents_md()` is called and the root repo has an AGENTS.md
- **THEN** the sidecar instruction file SHALL contain the root content followed by the assignment section

#### Scenario: Root AGENTS.md does not exist
- **WHEN** `setup_worktree_agents_md()` is called and the root repo has no AGENTS.md
- **THEN** the sidecar instruction file SHALL contain only the assignment section

#### Scenario: Root AGENTS.md has existing git-paw section
- **WHEN** the root AGENTS.md contains a `<!-- git-paw:start -->` section
- **THEN** the root section SHALL be replaced with the worktree assignment section (not duplicated) in the sidecar content

#### Scenario: Managed block reaches the agent via the sidecar
- **WHEN** `setup_worktree_agents_md()` completes successfully
- **THEN** the CLI's instruction file SHALL resolve to the combined view containing the `<!-- git-paw:start -->` block
- **AND** the agent SHALL receive the managed block without it being present in the tracked `AGENTS.md`

### Requirement: Write worktree AGENTS.md to worktree root

The system SHALL write the generated combined content to a gitignored sidecar instruction file in the worktree, leaving the worktree's tracked `AGENTS.md` unmodified by git-paw.

#### Scenario: Sidecar written to worktree
- **WHEN** `setup_worktree_agents_md()` completes successfully
- **THEN** the gitignored sidecar instruction file SHALL exist in the worktree with the combined content

#### Scenario: Tracked AGENTS.md remains committable
- **WHEN** `setup_worktree_agents_md()` completes successfully
- **THEN** the worktree's tracked `AGENTS.md` SHALL NOT be marked `assume-unchanged`
- **AND** a hand edit to the tracked `AGENTS.md` SHALL appear in `git status` and stage via `git add -A`

#### Scenario: Write failure
- **WHEN** writing the sidecar instruction file to the worktree fails
- **THEN** the system SHALL return `PawError::AgentsMdError` with context about the failure

### Requirement: Exclude worktree AGENTS.md from git

The system SHALL add the sidecar instruction file path (e.g. `.git-paw/AGENTS.local.md`) to the worktree's ignore set (`.git/info/exclude` or `.gitignore`) to prevent accidental commits of the ephemeral injection. The system SHALL NOT add the tracked `AGENTS.md` to the worktree's `.git/info/exclude`.

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
