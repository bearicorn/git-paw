## MODIFIED Requirements

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

#### Scenario: Skill content contains rendered broker URL
- **WHEN** `generate_worktree_section()` is called with skill content rendered via `skills::render`
- **THEN** the skill section in the output contains the literal broker URL (e.g., `http://127.0.0.1:9119`) and no placeholder
