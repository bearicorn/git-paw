## ADDED Requirements

### Requirement: Supervisor skill — Governance verification sub-step

The embedded `supervisor.md` skill SHALL include a "Governance verification" section (or sub-section within the existing Spec Audit Procedure) instructing the supervisor agent how to handle governance documents. The section SHALL include the following content:

1. **Activation condition** — the section's instructions apply only when the boot prompt's "Governance documents" section is present (i.e. at least one `[governance]` path is configured). When the boot-prompt section is absent, the supervisor SHALL skip governance reading entirely.
2. **Ordering** — governance reading runs as a sub-step *inside* the existing Spec Audit Procedure (step 7 in the supervisor flow), NOT as a separate flow step. The skill SHALL state this explicitly.
3. **Per-doc examples** — the section SHALL provide brief examples of what to look for per doc type (DoD walk against branch state, ADR drift detection in the diff, security checklist walk, test-strategy proportion check, constitution conformance check). The examples SHALL be illustrative, not exhaustive rubrics. The skill SHALL state that the supervisor agent applies judgment given the project's conventions.
4. **Findings flow through `agent.feedback`** — the section SHALL state that governance findings are surfaced as standard `agent.feedback` errors, mixed in with spec-audit findings. There is NO governance-specific tag prefix, NO `[governance-gate:<doc>]` token, NO separate broker variant.
5. **Missing-doc handling** — the section SHALL instruct the supervisor that a configured path with no readable file is a finding (added to the `agent.feedback` errors list) but NOT a separate failure type.
6. **No gating semantics** — the skill SHALL NOT instruct the supervisor to consult any `[governance.gates]` table, since that table does not exist. The skill SHALL NOT use the language of "gating" or "blocking on governance failures" — governance findings are audit findings, treated like any other.

The section SHALL be inserted in `supervisor.md` within or immediately after the existing Spec Audit Procedure.

#### Scenario: Supervisor skill mentions Governance verification

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains the substring `Governance verification` (or equivalent heading)
- **AND** it states that the section's instructions apply only when the boot prompt's "Governance documents" section is present

#### Scenario: Supervisor skill specifies the ordering

- **WHEN** the embedded supervisor skill's flow is inspected
- **THEN** the governance reading is described as a sub-step of the existing Spec Audit Procedure
- **AND** is NOT presented as a separate workflow step (no "step 7.5" framing)

#### Scenario: Supervisor skill provides per-doc examples

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains illustrative examples for DoD walks, ADR drift, security checklist walks, test-strategy checks, and constitution conformance
- **AND** the skill states the examples are illustrative, not exhaustive rubrics
- **AND** the skill states the supervisor agent applies judgment given the project's conventions

#### Scenario: Supervisor skill states findings flow through agent.feedback

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it states that governance findings are reported as `agent.feedback` errors (alongside other audit findings)

#### Scenario: Supervisor skill does NOT introduce governance-specific tag

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it does NOT contain the substring `[governance-gate:`
- **AND** it does NOT introduce a tag prefix or categorisation token specific to governance findings

#### Scenario: Supervisor skill does NOT reference governance gates

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it does NOT contain the substring `[governance.gates]`
- **AND** it does NOT instruct the supervisor to consult per-doc gate flags
- **AND** it does NOT use the language of "gating" or "blocking on governance failures"

#### Scenario: Supervisor skill instructs missing-doc handling

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it instructs the supervisor that a configured path pointing at a non-existent file becomes a finding in the `agent.feedback` errors list
- **AND** it does NOT treat missing files as a distinct failure type
