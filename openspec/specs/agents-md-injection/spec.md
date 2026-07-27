# agents-md-injection Specification

## Purpose
Inject and manage a marker-delimited git-paw section in AGENTS.md files, supporting detection, generation, replacement, and file-level injection of git-paw configuration content for AI coding CLIs, and generate and write per-worktree AGENTS.md files that combine the root repository's AGENTS.md content with worktree-specific assignment sections containing branch, CLI, spec content, and file ownership information, while excluding generated files from git tracking.

## Requirements
### Requirement: Detect existing git-paw section

The system SHALL detect whether a markdown string contains a git-paw section by checking for the `<!-- git-paw:start` prefix.

#### Scenario: Content with git-paw section
- **WHEN** `has_git_paw_section()` is called with content containing `<!-- git-paw:start`
- **THEN** it SHALL return `true`

#### Scenario: Content without git-paw section
- **WHEN** `has_git_paw_section()` is called with content that does not contain `<!-- git-paw:start`
- **THEN** it SHALL return `false`

#### Scenario: Empty content
- **WHEN** `has_git_paw_section()` is called with an empty string
- **THEN** it SHALL return `false`

### Requirement: Generate git-paw section content

The system SHALL generate a marker-delimited section containing git-paw instructions for AI coding CLIs.

#### Scenario: Generated section has markers
- **WHEN** `generate_git_paw_section()` is called
- **THEN** the result SHALL start with a line containing `<!-- git-paw:start` and end with a line containing `<!-- git-paw:end -->`

#### Scenario: Generated section contains guidance
- **WHEN** `generate_git_paw_section()` is called
- **THEN** the result SHALL contain guidance about git-paw configuration and parallel sessions

### Requirement: Replace existing git-paw section

The system SHALL replace the content between markers (inclusive) with a new section.

#### Scenario: Replace section with both markers present
- **WHEN** `replace_git_paw_section()` is called with content that has both start and end markers
- **THEN** the content from start marker through end marker (inclusive) SHALL be replaced with the new section

#### Scenario: Content before and after markers is preserved
- **WHEN** content exists both before `<!-- git-paw:start` and after `<!-- git-paw:end -->`
- **THEN** `replace_git_paw_section()` SHALL preserve all content outside the markers

#### Scenario: Replace when end marker is missing
- **WHEN** `replace_git_paw_section()` is called with content that has a start marker but no end marker
- **THEN** everything from the start marker to EOF SHALL be replaced with the new section

### Requirement: Inject section into content string

The system SHALL append a section if no git-paw section exists, or replace the existing one.

#### Scenario: Inject into content without existing section
- **WHEN** `inject_into_content()` is called with content that has no git-paw section
- **THEN** the section SHALL be appended to the content

#### Scenario: Inject into content with existing section
- **WHEN** `inject_into_content()` is called with content that already has a git-paw section
- **THEN** the existing section SHALL be replaced with the new one

#### Scenario: Inject into empty content
- **WHEN** `inject_into_content()` is called with an empty string
- **THEN** the result SHALL contain only the new section

### Requirement: Inject section into file

The system SHALL read the injection-target file, inject the section, and write the result back. The injection target SHALL be a gitignored sidecar instruction file (e.g. `.git-paw/AGENTS.local.md`), NOT the worktree's tracked `AGENTS.md`. The system SHALL NOT set `git update-index --assume-unchanged` on the tracked `AGENTS.md`.

#### Scenario: File exists without git-paw section
- **WHEN** `inject_section_into_file()` is called on a file without a git-paw section
- **THEN** the section SHALL be appended and the file written

#### Scenario: File exists with git-paw section
- **WHEN** `inject_section_into_file()` is called on a file with an existing git-paw section
- **THEN** the section SHALL be replaced and the file written

#### Scenario: File does not exist
- **WHEN** `inject_section_into_file()` is called with a path that does not exist
- **THEN** the file SHALL be created containing only the section

#### Scenario: File is not writable
- **WHEN** `inject_section_into_file()` is called on a read-only file
- **THEN** it SHALL return `PawError::AgentsMdError` with a message mentioning the file path

#### Scenario: Injection target is the sidecar, not the tracked AGENTS.md
- **WHEN** the managed git-paw block is injected during worktree setup
- **THEN** the block SHALL be written to the gitignored sidecar instruction file
- **AND** the worktree's tracked `AGENTS.md` SHALL NOT contain the managed git-paw block written by git-paw

#### Scenario: Tracked AGENTS.md is not marked assume-unchanged
- **WHEN** worktree setup completes
- **THEN** the system SHALL NOT have run `git update-index --assume-unchanged AGENTS.md`
- **AND** a hand edit to the tracked `AGENTS.md` SHALL appear in `git status`

### Requirement: Appended section is separated from existing content

When appending a section to existing content, the system SHALL ensure proper spacing.

#### Scenario: Existing content ends with newline
- **WHEN** content ends with `\n` and the section is appended
- **THEN** a blank line SHALL separate the existing content from the section

#### Scenario: Existing content does not end with newline
- **WHEN** content does not end with `\n` and the section is appended
- **THEN** a newline and blank line SHALL separate the existing content from the section

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
