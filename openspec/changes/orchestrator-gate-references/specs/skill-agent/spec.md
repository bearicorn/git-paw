## ADDED Requirements

### Requirement: The orchestrator package carries the audit gates as references

The orchestrator package SHALL carry the spec-audit, doc-audit and security-audit gate procedures as references `references/gates/spec-audit.md`, `references/gates/doc-audit.md` and `references/gates/security-audit.md`, moved out of `SKILL.md` rather than copied. Each reference SHALL be self-contained: it SHALL state the inputs the gate needs (the re-resolved branch tip, the merge-base diff, the change id, and the gate's rendered tooling command or `(not configured)`), the procedure, and the finding format with the gate's bracketed prefix. The orchestrator `SKILL.md` SHALL keep the testing and regression gates inline and SHALL keep one heading per audit gate that names the reference's installed path and instructs reading it before running that gate. References SHALL be rendered with the same placeholder set as `SKILL.md`.

#### Scenario: The three audit references exist and are rendered

- **WHEN** the orchestrator package is rendered with a configured `spec_validate_command`
- **THEN** `references/gates/spec-audit.md`, `references/gates/doc-audit.md` and `references/gates/security-audit.md` SHALL be present
- **AND** the spec-audit reference SHALL contain the rendered spec-validate command

#### Scenario: Each reference is self-contained

- **WHEN** any audit-gate reference is read on its own
- **THEN** it SHALL state that the gate runs against the re-resolved branch tip, that change contribution is diffed against the merge-base, and the bracketed prefix its findings use

#### Scenario: Gate content is moved, not duplicated

- **WHEN** the rendered orchestrator `SKILL.md` is inspected
- **THEN** it SHALL NOT contain the Spec Audit Procedure body or the doc-audit surface list
- **AND** it SHALL contain one heading per audit gate naming that gate's reference path

#### Scenario: Testing and regression stay inline

- **WHEN** the rendered orchestrator `SKILL.md` is inspected
- **THEN** it SHALL contain the testing and regression-analysis gate procedures

### Requirement: The orchestrator skill delegates audit gates through a compressed-verdict contract

The orchestrator `SKILL.md` SHALL include a delegation section stating that, when the orchestrator's CLI can run isolated subagents, the three audit gates MAY run concurrently, each in its own isolated context given only that gate's reference plus the branch, re-resolved tip and change id, and only after the testing and regression gates have passed. Each delegated gate SHALL return a compressed verdict — the gate name, `pass` or `fail`, and one line per finding in the form `[<gate prefix>] <location>: <issue>` — and SHALL NOT return raw tool output. Only the orchestrator itself SHALL publish verdicts: `agent.verified` only when testing and regression passed and all three audit verdicts are `pass`, otherwise one `feedback-gate` call per finding. When the CLI cannot run isolated subagents, the orchestrator SHALL read each reference and run the gate inline. The section MAY also direct broad read-only exploration to an isolated read-only subagent that returns a map rather than file contents. The section SHALL name no vendor product, model, or persona file format or path.

#### Scenario: The delegation section defines the verdict shape

- **WHEN** the rendered orchestrator `SKILL.md` is inspected
- **THEN** its delegation section SHALL define a verdict of gate name, `pass`/`fail`, and one-line findings carrying the gate's bracketed prefix
- **AND** SHALL state that delegated gates return no raw tool output

#### Scenario: Only the orchestrator publishes

- **WHEN** the delegation section is inspected
- **THEN** it SHALL state that a delegated gate never publishes `agent.verified` or `agent.feedback`
- **AND** that `agent.verified` follows only when testing, regression and all three audit verdicts pass

#### Scenario: Inline fallback without subagents

- **WHEN** the delegation section is inspected
- **THEN** it SHALL instruct an orchestrator whose CLI cannot run isolated subagents to read each reference and run the gate inline

#### Scenario: The delegation section is vendor-neutral

- **WHEN** the delegation section is audited
- **THEN** it SHALL contain no vendor product name, model name, or vendor persona path

### Requirement: Orchestrator skill content requirements are satisfied package-wide

Any requirement that the orchestrator (formerly supervisor) skill include, contain or enumerate content SHALL be satisfied by the rendered orchestrator package as a whole — its `SKILL.md` followed by every reference that `SKILL.md` names — so content moved into a reference continues to satisfy the requirement that pinned it.

#### Scenario: A moved section still satisfies its requirement

- **GIVEN** the requirement that the orchestrator skill include a "Spec Audit Procedure" section
- **WHEN** its covering test renders the orchestrator package
- **THEN** the procedure SHALL be found in `references/gates/spec-audit.md`
- **AND** the requirement SHALL be satisfied
