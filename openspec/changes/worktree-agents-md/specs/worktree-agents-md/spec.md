## ADDED Requirements

### Requirement: Generate worktree assignment section

The system SHALL generate a marker-delimited section containing the worktree's branch assignment, CLI name, and optional spec content and file ownership.

#### Scenario: Assignment with all fields
- **WHEN** `generate_worktree_section()` is called with branch, CLI, spec content, and owned files
- **THEN** the result SHALL contain `<!-- git-paw:start -->` and `<!-- git-paw:end -->` markers, the branch name, CLI name, spec content, and file ownership list

#### Scenario: Assignment without spec content
- **WHEN** `generate_worktree_section()` is called with branch and CLI but no spec content
- **THEN** the result SHALL contain the branch and CLI but omit the Spec section

#### Scenario: Assignment without file ownership
- **WHEN** `generate_worktree_section()` is called with branch and CLI but no owned files
- **THEN** the result SHALL contain the branch and CLI but omit the File Ownership section

### Requirement: Combine root content with worktree assignment

The system SHALL read the root repo's AGENTS.md and append the worktree assignment section to produce the worktree's AGENTS.md content.

#### Scenario: Root AGENTS.md exists
- **WHEN** `setup_worktree_agents_md()` is called and the root repo has an AGENTS.md
- **THEN** the worktree AGENTS.md SHALL contain the root content followed by the assignment section

#### Scenario: Root AGENTS.md does not exist
- **WHEN** `setup_worktree_agents_md()` is called and the root repo has no AGENTS.md
- **THEN** the worktree AGENTS.md SHALL contain only the assignment section

#### Scenario: Root AGENTS.md has existing git-paw section
- **WHEN** the root AGENTS.md contains a `<!-- git-paw:start -->` section
- **THEN** the root section SHALL be replaced with the worktree assignment section (not duplicated)

### Requirement: Write worktree AGENTS.md to worktree root

The system SHALL write the generated AGENTS.md to the worktree's root directory.

#### Scenario: AGENTS.md written to worktree
- **WHEN** `setup_worktree_agents_md()` completes successfully
- **THEN** an `AGENTS.md` file SHALL exist at the worktree root with the combined content

#### Scenario: Write failure
- **WHEN** writing AGENTS.md to the worktree fails
- **THEN** the system SHALL return `PawError::AgentsMdError` with context about the failure

### Requirement: Exclude worktree AGENTS.md from git

The system SHALL add `AGENTS.md` to the worktree's `.git/info/exclude` to prevent accidental commits.

#### Scenario: Exclude entry added
- **WHEN** `exclude_from_git()` is called for a worktree
- **THEN** `AGENTS.md` SHALL appear in `.git/info/exclude` for that worktree

#### Scenario: Exclude entry already present
- **WHEN** `.git/info/exclude` already contains `AGENTS.md`
- **THEN** the entry SHALL NOT be duplicated

#### Scenario: .git/info directory does not exist
- **WHEN** `.git/info/` does not exist in the worktree
- **THEN** the directory SHALL be created before writing the exclude file

#### Scenario: Exclude file does not exist
- **WHEN** `.git/info/exclude` does not exist
- **THEN** the file SHALL be created containing `AGENTS.md`
