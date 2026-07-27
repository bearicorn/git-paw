## ADDED Requirements

### Requirement: Export-agnosticism skill provided in the agentskills.io format

The repository SHALL provide an `export-agnosticism` agent skill at
`.agents/skills/export-agnosticism/SKILL.md` following the agentskills.io standard (`name` matching
the folder; non-empty `description`). The skill SHALL enumerate git-paw's export surfaces across all
formats — bundled skills, scripts and mirrored logic (code), the init default config,
allowlist/classifier presets, injected artifacts (AGENTS.md sections, boot blocks, git hooks,
settings seeding), and templates — state the agnosticism rule, give per-surface guidance, list the
enforcing audits with the standing "a new export surface needs its own guard" rule, and provide a
review-gate checklist.

#### Scenario: Skill is present and standard-conformant

- **GIVEN** the repository checkout
- **WHEN** `.agents/skills/export-agnosticism/SKILL.md` is loaded
- **THEN** it SHALL parse as a valid agentskills.io skill with `name = "export-agnosticism"` and a non-empty `description`

#### Scenario: The inventory spans more than skills

- **WHEN** the skill's export-surface inventory is read
- **THEN** it SHALL cover code (scripts / mirrored classifier), config (the init default), presets/allowlists, injected artifacts (AGENTS.md sections / boot blocks / git hooks / settings seeding), and templates — not only bundled skills

### Requirement: Cross-referenced from the code standards

The `code-standards` skill SHALL cross-link to the `export-agnosticism` skill so an agent editing
an exported asset is routed to it.

#### Scenario: code-standards points to the export skill

- **WHEN** `.agents/skills/code-standards/SKILL.md` is read
- **THEN** it SHALL reference the `export-agnosticism` skill for changes that touch an exported asset
