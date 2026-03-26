## ADDED Requirements

### Requirement: Inject git-paw section into AGENTS.md

The system SHALL inject a git-paw instruction section delimited by `<!-- git-paw:start -->` and `<!-- git-paw:end -->` markers into the project's `AGENTS.md`.

#### Scenario: AGENTS.md exists without git-paw section
- **WHEN** `AGENTS.md` exists at repo root and does not contain `<!-- git-paw:start -->`
- **THEN** the git-paw section SHALL be appended to the file

#### Scenario: AGENTS.md does not exist
- **WHEN** no `AGENTS.md` exists at repo root
- **THEN** `AGENTS.md` SHALL be created containing the git-paw section

#### Scenario: AGENTS.md already contains git-paw section
- **WHEN** `AGENTS.md` exists and already contains `<!-- git-paw:start -->`
- **THEN** the existing section SHALL be replaced with the current version (update in place)

### Requirement: Marker-based section boundaries

The injected section SHALL be enclosed between `<!-- git-paw:start -->` and `<!-- git-paw:end -->` markers, each on their own line.

#### Scenario: Markers are present in output
- **WHEN** the git-paw section is injected
- **THEN** the output SHALL contain `<!-- git-paw:start -->` before the content and `<!-- git-paw:end -->` after

#### Scenario: Section replacement preserves surrounding content
- **WHEN** `AGENTS.md` has content before and after the git-paw section and the section is re-injected
- **THEN** content outside the markers SHALL be preserved unchanged

### Requirement: CLAUDE.md compatibility

The system SHALL handle repos that use `CLAUDE.md` instead of or in addition to `AGENTS.md`.

#### Scenario: Repo has CLAUDE.md but no AGENTS.md
- **WHEN** the repo has `CLAUDE.md` but no `AGENTS.md`
- **THEN** the git-paw section SHALL be appended to `CLAUDE.md` and `AGENTS.md` SHALL be created as a symlink to `CLAUDE.md`

#### Scenario: Repo has both CLAUDE.md and AGENTS.md
- **WHEN** the repo has both `CLAUDE.md` and `AGENTS.md`
- **THEN** the git-paw section SHALL be appended to `AGENTS.md` only and `CLAUDE.md` SHALL NOT be modified

#### Scenario: Repo has neither CLAUDE.md nor AGENTS.md
- **WHEN** the repo has neither file
- **THEN** `AGENTS.md` SHALL be created with the git-paw section and no symlink SHALL be created

#### Scenario: AGENTS.md is already a symlink to CLAUDE.md
- **WHEN** `AGENTS.md` already exists as a symlink to `CLAUDE.md`
- **THEN** the git-paw section SHALL be injected into `CLAUDE.md` (the symlink target) and the symlink SHALL be preserved

### Requirement: Injected content is meaningful

The git-paw section SHALL contain instructions relevant to AI coding CLIs operating in a git-paw worktree environment.

#### Scenario: Section contains worktree guidance
- **WHEN** the git-paw section is generated
- **THEN** it SHALL include guidance about worktree awareness, branch scope, and avoiding cross-worktree modifications

### Requirement: File write safety

The system SHALL handle file I/O errors gracefully when reading or writing `AGENTS.md` or `CLAUDE.md`.

#### Scenario: Read-only AGENTS.md
- **WHEN** `AGENTS.md` exists but is not writable
- **THEN** the system SHALL return a `PawError::AgentsMdError` with a message mentioning permissions

#### Scenario: Symlink creation failure
- **WHEN** the system cannot create a symlink (e.g., target path issue)
- **THEN** the system SHALL return a `PawError::AgentsMdError` with context about the failure
