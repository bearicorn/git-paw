## ADDED Requirements

### Requirement: Exported skills and the AGENTS.md pointer pass the no-leak audit

The no-language-leak audit (`tests/lang_agnostic_skill_audit.rs`) SHALL cover the rendered orchestrator `SKILL.md` — frontmatter included — for every spec backend, the exported `cli-setup` `SKILL.md`, and the bundled `assets/agents-md-orchestrator-section.md` pointer asset. None SHALL contain a forbidden stack token, a hard-coded commit-message convention, or a vendor product name or path outside an `<!-- allowlist-prose -->` span.

#### Scenario: Rendered SKILL.md passes the audit for every backend

- **WHEN** the orchestrator skill is rendered to the standardized `SKILL.md` for the OpenSpec, Spec Kit, Markdown, Superpowers and no-backend cases
- **THEN** the audit SHALL find no forbidden stack token, commit-convention prefix, or vendor name/path in any rendering

#### Scenario: Exported cli-setup SKILL.md passes the audit

- **WHEN** the exported `.git-paw/skills/cli-setup/SKILL.md` is audited
- **THEN** it SHALL contain no forbidden stack token, commit-convention prefix, or vendor name/path

#### Scenario: Pointer asset passes the audit

- **WHEN** `assets/agents-md-orchestrator-section.md` is audited
- **THEN** it SHALL contain no forbidden stack token, commit-convention prefix, or vendor name/path

#### Scenario: A vendor path regression is caught

- **GIVEN** a copy of the pointer asset with `.claude/skills/` inserted outside an allowlist span
- **WHEN** the audit runs against it
- **THEN** it SHALL fail naming the offending token
