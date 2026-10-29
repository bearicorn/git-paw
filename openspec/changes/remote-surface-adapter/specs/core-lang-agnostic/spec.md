## ADDED Requirements

### Requirement: Remote agent-facing texts are bundled agnostic assets

The texts git-paw adds when it addresses a remote worker or reports one — the remote task addendum sent with a remote worker's task, the framing of a verify-continuation message, and the framing of an escalated remote question — SHALL be authored in bundled assets (`assets/remote-task-addendum.md`, `assets/remote-verify-continuation.md`, `assets/remote-question.md`) and loaded at compile time, not written as string literals in code. They SHALL name no vendor, agent CLI, programming-language stack, build or test tool, commit convention or git-paw helper-script path. The no-language-leak audit SHALL cover all three.

#### Scenario: The remote texts pass the audit

- **GIVEN** the three remote assets
- **WHEN** the no-language-leak audit runs over them
- **THEN** it SHALL find no forbidden stack token, commit-convention prefix, vendor name or helper-script path in any of them

#### Scenario: The texts are not compiled literals

- **WHEN** the code that renders the remote task addendum, the verify-continuation framing and the question framing is inspected
- **THEN** each text SHALL be loaded from its asset with `include_str!`
