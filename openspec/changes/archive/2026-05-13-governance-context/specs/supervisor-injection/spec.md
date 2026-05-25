## ADDED Requirements

### Requirement: Boot prompt includes governance documents section

When the supervisor agent's boot prompt is constructed AND `config.governance` has at least one path field set to `Some(_)`, the system SHALL append a "Governance documents" section to the boot prompt. The section SHALL list one bullet per configured path with the doc's canonical name and the configured path. Path fields whose value is `None` SHALL NOT appear in the bullet list.

When ALL `config.governance` path fields are `None`, the system SHALL omit the entire "Governance documents" section from the boot prompt (no header, no empty bullet list, no placeholder text).

The section SHALL be a plain-text block separated from preceding boot-prompt content by a blank line. The section heading SHALL be the literal string `## Governance documents`.

The section SHALL NOT contain a "gates" sub-line, gate-flag summaries, or any per-doc enforcement metadata. `governance-config` no longer ships a `[governance.gates]` table; the boot prompt has nothing to convey about enforcement beyond the path list.

#### Scenario: Section omitted when no paths configured

- **GIVEN** `config.governance` with all five path fields `None`
- **WHEN** the supervisor's boot prompt is constructed
- **THEN** the boot prompt SHALL NOT contain the substring `Governance documents`

#### Scenario: Section present with one path

- **GIVEN** `config.governance.dod = Some("docs/dod.md")` and the other path fields `None`
- **WHEN** the boot prompt is constructed
- **THEN** the boot prompt SHALL contain the heading `## Governance documents`
- **AND** the section SHALL contain a bullet referencing `dod` and `docs/dod.md`
- **AND** the section SHALL NOT contain bullets for `adr`, `test_strategy`, `security`, or `constitution`

#### Scenario: Section lists all configured paths in canonical order

- **GIVEN** `config.governance` with all five paths populated
- **WHEN** the boot prompt is constructed
- **THEN** the section SHALL list five bullets in canonical order: `adr`, `test_strategy`, `security`, `dod`, `constitution`

#### Scenario: Section contains no gates summary

- **GIVEN** any `config.governance` configuration with at least one path set
- **WHEN** the boot prompt is constructed
- **THEN** the "Governance documents" section SHALL NOT contain a "Gated docs" line, a "Governance gates" sub-section, or any text referencing per-doc gate flags

### Requirement: Governance section follows the supervisor skill content

The "Governance documents" section SHALL appear in the boot prompt *after* the supervisor skill content (rendered from `assets/agent-skills/supervisor.md` per the existing supervisor-launch capability) and BEFORE any per-agent task content. This positioning ensures the supervisor agent reads governance configuration in the same context where it reads its own skill instructions.

#### Scenario: Section position is between skill and task content

- **GIVEN** a configured `config.governance` and a supervisor session being launched
- **WHEN** the boot prompt is constructed
- **THEN** the position of `## Governance documents` SHALL come after the substring `## Supervisor Skills` (or whatever the skill heading is)
- **AND** SHALL come before any task-specific content
