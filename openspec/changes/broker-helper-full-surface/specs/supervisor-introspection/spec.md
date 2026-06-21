## MODIFIED Requirements

### Requirement: Supervisor phase taxonomy

The bundled supervisor skill SHALL document a phase taxonomy
covering at least: `sweep`, `audit`, `merge`, `feedback`,
`intent_watch`, `learnings`, `idle`. Each phase SHALL have a
documented `detail` shape so the supervisor LLM emits
consistent structured data across sessions.

The skill SHALL deliver every phase-tagged `agent.status` — including
the boot self-register, each documented phase transition, and the
`checkpoint` emission — through the bundled `sweep.sh status-publish`
helper (`--phase <phase>` plus, when the taxonomy specifies a detail body,
`--detail '<json-object>'`), NOT through a raw `curl …/publish` call. The
skill's phase-taxonomy examples SHALL show the `sweep.sh status-publish`
form so the documented taxonomy reaches the broker by the least-privilege,
by-path helper grant rather than a broad curl allowlist.

#### Scenario: Taxonomy table documents all seven phases

- **WHEN** the bundled supervisor.md is inspected
- **THEN** the introspection section SHALL contain a table
  listing at least the seven phase values with their
  documented detail field names

#### Scenario: Audit phase detail names the five gates

- **WHEN** the audit phase's detail documentation is read
- **THEN** the detail's `audit_step` field SHALL enumerate
  the v0.5.0 five gates (tests, spec, docs, security,
  regression)

#### Scenario: Phase emission examples use the helper, not raw curl

- **WHEN** the introspection section's phase-emission examples are read
- **THEN** each `agent.status` emission example SHALL invoke
  `sweep.sh status-publish` with `--phase` (and `--detail` where the
  taxonomy specifies a detail body)
- **AND** no example SHALL emit an `agent.status` via a raw
  `curl …/publish` call
