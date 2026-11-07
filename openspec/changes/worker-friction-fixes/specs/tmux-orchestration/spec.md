## ADDED Requirements

### Requirement: start never ignores launch selection on a session with no agents

When `git paw start` is run with `--branches`, `--specs` or `--from-all-specs` and the repository's session records zero agents, it SHALL NOT silently ignore the selection. A stopped such session SHALL be discarded (as `purge` does) and the selection launched fresh. A live or paused one SHALL be refused, unchanged, with an error naming `git paw add`. With agents recorded, `start` SHALL behave as before.

#### Scenario: A stopped empty session is replaced by the selection

- **GIVEN** a session recording zero agents whose effective status is stopped
- **WHEN** the user runs `git paw start --specs a,b`
- **THEN** the empty session SHALL be discarded and a fresh session SHALL launch agents for `a` and `b`
- **AND** the output SHALL say the empty session was replaced, not that it was recovered

#### Scenario: A live empty session refuses and names add

- **GIVEN** a live session recording zero agents
- **WHEN** the user runs `git paw start --branches feat/x`
- **THEN** the command SHALL exit non-zero without changing the session
- **AND** its error SHALL name `git paw add`

#### Scenario: start without selection flags keeps its behaviour

- **GIVEN** a stopped session recording zero agents
- **WHEN** the user runs `git paw start` with no launch-selection flag
- **THEN** it SHALL recover the session as before

#### Scenario: A session with agents keeps its behaviour

- **GIVEN** a stopped session recording two agents
- **WHEN** the user runs `git paw start --specs a`
- **THEN** it SHALL recover the recorded session as before
