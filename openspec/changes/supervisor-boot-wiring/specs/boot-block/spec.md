## MODIFIED Requirements

### Requirement: Governance section follows the supervisor skill content

The "Governance documents" section SHALL appear in the boot prompt *after* the supervisor skill content (rendered from `assets/agent-skills/supervisor.md` per the existing supervisor-launch capability) and BEFORE any per-agent task content. This positioning ensures the supervisor agent reads governance configuration in the same context where it reads its own skill instructions.

The section SHALL be produced by the same assembly the supervisor launch uses to write the supervisor's boot context (the supervisor's managed block in the repository-root `AGENTS.md`), so a configured `[governance]` table reaches every launched supervisor session — not only a prompt assembled outside the launch path.

#### Scenario: Section position is between skill and task content

- **GIVEN** a configured `config.governance` and a supervisor session being launched
- **WHEN** the boot prompt is constructed
- **THEN** the position of `## Governance documents` SHALL come after the substring `## Supervisor Skills` (or whatever the skill heading is)
- **AND** SHALL come before any task-specific content

#### Scenario: A launched supervisor session carries the governance section

- **GIVEN** a repository whose config sets `[governance] dod = "docs/dod.md"` and `[supervisor] enabled = true`
- **WHEN** `git paw start --supervisor` launches the session
- **THEN** the supervisor's boot context written to the repository-root `AGENTS.md` SHALL contain the start-of-line heading `## Governance documents` and the bullet `- dod: docs/dod.md`
- **AND** the heading SHALL come after `## Supervisor Skills`

#### Scenario: A launched supervisor session without governance paths is unchanged

- **GIVEN** a repository whose config has no `[governance]` paths set and `[supervisor] enabled = true`
- **WHEN** `git paw start --supervisor` launches the session
- **THEN** the supervisor's boot context written to the repository-root `AGENTS.md` SHALL NOT contain a start-of-line `## Governance documents` heading
