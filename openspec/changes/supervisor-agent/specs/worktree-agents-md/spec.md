## MODIFIED Requirements

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
