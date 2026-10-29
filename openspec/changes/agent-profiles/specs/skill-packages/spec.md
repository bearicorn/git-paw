## ADDED Requirements

### Requirement: Bundled delegate role packages

The system SHALL ship four bundled delegate role packages registered in the package registry: `explorer`, `spec-auditor`, `doc-auditor` and `security-auditor`. Each SHALL state that the delegate is read-only unless its approval level permits edits, SHALL instruct it to return only a compact result on stdout, and SHALL name no vendor product, model or project toolchain. `explorer` SHALL instruct a broad read-only sweep returning a map — paths with a one-line role each — rather than file contents. Each auditor SHALL instruct returning the compressed verdict of the orchestrator's delegation contract (gate name, `pass` or `fail`, one line per finding carrying the gate's bracketed prefix) and never publishing a verdict, and its rendered text SHALL contain its gate's procedure rendered from the orchestrator package's reference — `references/gates/spec-audit.md`, `references/gates/doc-audit.md` or `references/gates/security-audit.md` respectively — through a registry inclusion, so that each gate procedure has exactly one source file. These packages SHALL never be selected for an agent worktree.

#### Scenario: Auditors render the orchestrator's gate references

- **GIVEN** a configured `spec_validate_command`
- **WHEN** `spec-auditor` is rendered
- **THEN** its text SHALL contain the rendered `references/gates/spec-audit.md` of the orchestrator package, including that command

#### Scenario: Each gate procedure has one source

- **WHEN** the registered package files are scanned
- **THEN** no auditor package file SHALL contain a sentence of its gate reference's source text

#### Scenario: Auditors define the verdict and never publish

- **WHEN** each auditor package is rendered
- **THEN** it SHALL define the verdict as gate name, `pass`/`fail` and prefixed one-line findings
- **AND** it SHALL instruct the delegate never to publish `agent.verified` or `agent.feedback`

#### Scenario: Explorer returns a map

- **WHEN** `explorer` is rendered
- **THEN** it SHALL instruct returning paths with one-line roles and not returning file contents

#### Scenario: Delegate role packages never reach a worktree

- **WHEN** the worktree package selection is computed for any combination of inputs
- **THEN** it SHALL contain none of `explorer`, `spec-auditor`, `doc-auditor` or `security-auditor`

#### Scenario: The role packages are vendor-neutral

- **WHEN** the no-language-leak audit renders the four packages
- **THEN** none SHALL contain a vendor product name, a model name or a project-specific toolchain command
