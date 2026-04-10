## Purpose

Generate and write per-worktree AGENTS.md files that combine the root repository's AGENTS.md content with worktree-specific assignment sections containing branch, CLI, spec content, and file ownership information, while excluding generated files from git tracking.
## Requirements
### Requirement: Generate worktree assignment section

The system SHALL generate a marker-delimited section containing the worktree's branch assignment, CLI name, optional spec content, optional file ownership, and optional coordination skill content.

When `skill_content` is provided on `WorktreeAssignment`, the rendered skill text SHALL be appended inside the markers after the file ownership subsection (or after the spec subsection if no file ownership is present, or after the assignment if neither is present) and before the end marker.

When `skill_content` is `None`, the generated section SHALL be identical to the v0.2.0 output.

#### Scenario: Assignment with all fields including skill content
- **WHEN** `generate_worktree_section()` is called with branch, CLI, spec content, owned files, and skill content
- **THEN** the result SHALL contain `<!-- git-paw:start -->` and `<!-- git-paw:end -->` markers, the branch name, CLI name, spec content, file ownership list, and the skill content
- **AND** the skill content appears after the file ownership section and before `<!-- git-paw:end -->`

#### Scenario: Assignment with skill content but no spec or files
- **WHEN** `generate_worktree_section()` is called with branch, CLI, and skill content, but no spec content and no owned files
- **THEN** the result SHALL contain the branch, CLI, and skill content
- **AND** the skill content appears after the assignment and before `<!-- git-paw:end -->`

#### Scenario: Assignment without skill content matches v0.2.0
- **WHEN** `generate_worktree_section()` is called with branch, CLI, spec content, and owned files, but `skill_content = None`
- **THEN** the result SHALL be identical to the v0.2.0 output (no skill section present)

#### Scenario: Assignment without spec content
- **WHEN** `generate_worktree_section()` is called with branch and CLI but no spec content
- **THEN** the result SHALL contain the branch and CLI but omit the Spec section

#### Scenario: Assignment without file ownership
- **WHEN** `generate_worktree_section()` is called with branch and CLI but no owned files
- **THEN** the result SHALL contain the branch and CLI but omit the File Ownership section

#### Scenario: Skill content contains rendered BRANCH_ID
- **WHEN** `generate_worktree_section()` is called with skill content that was rendered via `skills::render` for branch `feat/http-broker`
- **THEN** the skill section in the output contains `feat-http-broker` (the slugified branch) and does not contain the literal `{{BRANCH_ID}}`

#### Scenario: Skill content preserves broker URL placeholder
- **WHEN** `generate_worktree_section()` is called with skill content rendered via `skills::render`
- **THEN** the skill section in the output contains the literal `${GIT_PAW_BROKER_URL}` (not substituted)

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

