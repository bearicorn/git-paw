## Purpose

Generate and write per-worktree AGENTS.md files that combine the root repository's AGENTS.md content with worktree-specific assignment sections containing branch, CLI, spec content, and file ownership information, while excluding generated files from git tracking.
## Requirements
### Requirement: Generate worktree assignment section

The `WorktreeAssignment` struct SHALL support an optional `inter_agent_rules: Option<String>` field. When provided, the system SHALL append a `## Inter-Agent Rules` subsection inside the git-paw markers after the skill content (or after the assignment if no skill content is present).

The inter-agent rules section SHALL be rendered verbatim from the `inter_agent_rules` string. The supervisor populates this field with rules about file ownership, commit behavior, status publishing requirements, and cherry-pick instructions.

When `inter_agent_rules` is `None`, the generated section SHALL be identical to the pre-supervisor output. No `## Inter-Agent Rules` section SHALL appear.

#### Scenario: Assignment with inter-agent rules section

- **WHEN** `generate_worktree_section()` is called with `inter_agent_rules = Some(rules_text)`
- **THEN** the result SHALL contain `## Inter-Agent Rules` followed by the rules text
- **AND** the rules section SHALL appear after the skill content (if present) and before `<!-- git-paw:end -->`

#### Scenario: Assignment without inter-agent rules has no rules section

- **WHEN** `generate_worktree_section()` is called with `inter_agent_rules = None`
- **THEN** the result SHALL NOT contain `## Inter-Agent Rules`

#### Scenario: Inter-agent rules include file ownership constraint

- **GIVEN** the supervisor provides standard inter-agent rules
- **WHEN** the rules are inspected
- **THEN** they SHALL include a statement that agents MUST NOT edit files owned by other agents

#### Scenario: Inter-agent rules include never-push constraint

- **GIVEN** the supervisor provides standard inter-agent rules
- **WHEN** the rules are inspected
- **THEN** they SHALL include a statement that agents MUST commit to their worktree branch and MUST NOT push

#### Scenario: Inter-agent rules include proactive status publishing requirement

- **GIVEN** the supervisor provides standard inter-agent rules
- **WHEN** the rules are inspected
- **THEN** they SHALL state that `agent.status` MUST be published when starting work, editing files, and after each commit

#### Scenario: Inter-agent rules include match-spec requirement

- **GIVEN** the supervisor provides standard inter-agent rules
- **WHEN** the rules are inspected
- **THEN** they SHALL state that agents MUST match spec field names exactly

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

