## ADDED Requirements

### Requirement: Managed block indexes the provisioned skill packages

The system SHALL include in an agent worktree's managed git-paw block a `## Skills` index, generated from the registry and the worktree's package selection, with one entry per provisioned package giving its name, frontmatter description, installed `SKILL.md` path and helper package path. The index SHALL sit inside the markers before the inlined skill bodies, which stay inlined. With no package provisioned, no index SHALL appear.

#### Scenario: Index lists the coordination package

- **GIVEN** a broker-enabled session with `docs_base_url` unset
- **WHEN** an agent's managed block is generated
- **THEN** it SHALL contain a `## Skills` section with one entry naming `coordination`, `.git-paw/skills/coordination/SKILL.md` and `.git-paw/skills/coordination/scripts/broker.sh`
- **AND** that section SHALL appear before the inlined coordination skill content and before `<!-- git-paw:end -->`

#### Scenario: Index follows the selection

- **GIVEN** a session with the broker enabled and `docs_base_url` configured
- **WHEN** an agent's managed block is generated
- **THEN** the `## Skills` index SHALL list `coordination` and `docs-fetch`, in that order

#### Scenario: No packages, no index

- **GIVEN** a session with the broker disabled and `docs_base_url` unset
- **WHEN** an agent's managed block is generated
- **THEN** it SHALL NOT contain a `## Skills` section
