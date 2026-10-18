## ADDED Requirements

### Requirement: Escalation framing prose is a bundled agnostic asset

The framing sentences of the headless escalation context SHALL be authored in the bundled asset `assets/escalation-framing.md` (with placeholders for the escalation body, agent id, repository root, skill and sidecar paths, and broker URL) rather than compiled string literals, and the no-language-leak audit SHALL cover the rendered context for every escalation kind.

#### Scenario: Rendered escalation context passes the audit

- **WHEN** the escalation context is rendered for a question, a merge decision and an exhausted branch
- **THEN** the audit SHALL find no forbidden stack token, commit-convention prefix, or vendor name/path in any rendering

#### Scenario: Framing is not a compiled literal

- **WHEN** the context renderer's source is inspected
- **THEN** its framing prose SHALL be loaded via `include_str!("../assets/escalation-framing.md")`
