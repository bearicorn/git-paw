## ADDED Requirements

### Requirement: No-telemetry privacy guarantee

Learnings mode SHALL perform no telemetry. The learnings aggregator SHALL write only to the local `.git-paw/session-learnings.md` file and SHALL NOT transmit learnings content to any network destination outside the operator's own machine. git-paw SHALL NOT collect, upload, or phone home learnings data under any configuration.

#### Scenario: Learnings output stays local

- **GIVEN** a session running with `[supervisor] learnings = true`
- **WHEN** the aggregator records and flushes learnings
- **THEN** the only artifact produced SHALL be the local `.git-paw/session-learnings.md` file
- **AND** no learnings content SHALL be transmitted to any destination other than the operator's machine

### Requirement: Session-start learnings disclosure notice

When a session starts with learnings mode enabled (`[supervisor] learnings = true`), git-paw SHALL print a concise notice to the user that states: (a) the local path the learnings file is written to, (b) that nothing is sent anywhere / no telemetry, and (c) that the file may be reviewed and optionally shared with the maintainers via a GitHub issue to improve the tool, after reviewing it and stripping or anonymising any sensitive repo-specific details (a task the user's own LLM can assist with).

The notice SHALL NOT be printed when learnings mode is disabled (the default), so a session that has not opted in behaves identically to before this change.

#### Scenario: Notice prints when learnings is enabled

- **GIVEN** a configuration with `[supervisor] enabled = true` and `[supervisor] learnings = true`
- **WHEN** the session starts
- **THEN** git-paw SHALL print a notice that names the local `.git-paw/session-learnings.md` path, states that no telemetry is performed, and invites optional sharing via a GitHub issue with a review/anonymise caveat

#### Scenario: No notice when learnings is disabled

- **GIVEN** a configuration with `[supervisor] learnings = false` or the `[supervisor]` section absent
- **WHEN** the session starts
- **THEN** git-paw SHALL NOT print the learnings disclosure notice
- **AND** session start output SHALL be identical to the pre-change behavior

### Requirement: Documentation states privacy stance and sharing invitation

The learnings user-guide documentation SHALL state that learnings mode performs no telemetry, that its output is a local opt-in file, and SHALL invite users to optionally share the file with the maintainers via a GitHub issue to improve the tool — including the caveat that the file contains repo-specific details that should be reviewed and may be stripped or anonymised (e.g. with the user's own LLM) before sharing.

#### Scenario: Learnings doc carries the privacy and sharing section

- **WHEN** a reader opens the learnings user-guide chapter
- **THEN** it SHALL contain a section stating the no-telemetry / local / opt-in stance
- **AND** it SHALL contain the optional-sharing invitation with the review-and-anonymise caveat and a link to open a GitHub issue
