## ADDED Requirements

### Requirement: A CLI family may report whether a session's conversation exists

Each CLI family SHALL answer, for a session id, worktree and optional config directory, whether that session's conversation exists: yes, no, or unknown. The Claude family SHALL answer yes when its transcript `<id>.jsonl` exists under any Claude config directory it searches for usage, and no when none has it. A family without a transcript source SHALL answer unknown. The answer SHALL never fail.

#### Scenario: Claude transcript present

- **GIVEN** `<claude config>/projects/<worktree slug>/abc.jsonl` exists
- **WHEN** the Claude family is asked about session `abc` in that worktree
- **THEN** it SHALL answer yes

#### Scenario: Claude transcript absent

- **GIVEN** no Claude config directory holds `abc.jsonl`
- **WHEN** the Claude family is asked about session `abc`
- **THEN** it SHALL answer no

#### Scenario: Family without a source

- **GIVEN** a custom CLI with no family transcript source
- **WHEN** it is asked about any session
- **THEN** it SHALL answer unknown
