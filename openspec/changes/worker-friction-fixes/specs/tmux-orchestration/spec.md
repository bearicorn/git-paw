## ADDED Requirements

### Requirement: start never silently ignores a launch selection

When `git paw start` gets `--branches`, `--specs` or `--from-all-specs` for a session recording zero agents, a stopped one SHALL be discarded (as `purge` does) and the selection launched fresh; a live or paused one SHALL be refused, unchanged, naming `git paw add`. With agents recorded, `start` SHALL revive as before and print a one-line notice that the selection was not applied, naming `git paw add`.

#### Scenario: A stopped empty session is replaced by the selection

- **GIVEN** a session recording zero agents whose effective status is stopped
- **WHEN** the user runs `git paw start --specs a,b`
- **THEN** the empty session SHALL be discarded and a fresh session SHALL launch agents for `a` and `b`
- **AND** the output SHALL say the empty session was replaced, not that it was recovered

Test: `commands::start::tests::existing_session_action_table`, `tests/emptied_session_refill_e2e.rs::a_stopped_empty_session_is_replaced_by_the_selection_and_otherwise_recovered`

#### Scenario: A live empty session refuses and names add

- **GIVEN** a live session recording zero agents
- **WHEN** the user runs `git paw start --branches feat/x`
- **THEN** the command SHALL exit non-zero without changing the session
- **AND** its error SHALL name `git paw add`

Test: `tests/emptied_session_refill_e2e.rs::start_with_a_selection_on_a_live_empty_session_refuses_naming_add`

#### Scenario: start without selection flags keeps its behaviour

- **GIVEN** a stopped session recording zero agents
- **WHEN** the user runs `git paw start` with no launch-selection flag
- **THEN** it SHALL recover the session as before

Test: `tests/emptied_session_refill_e2e.rs::a_stopped_empty_session_is_replaced_by_the_selection_and_otherwise_recovered`

#### Scenario: A session with agents keeps its behaviour

- **GIVEN** a stopped session recording two agents
- **WHEN** the user runs `git paw start --specs a`
- **THEN** it SHALL recover the recorded session as before

Test: `commands::start::tests::existing_session_action_table`

#### Scenario: A selection given to a session with agents is reported as not applied

- **GIVEN** a running session recording one agent
- **WHEN** the user runs `git paw start --branches feat/b`
- **THEN** `start` SHALL revive (reattach to) the session as before
- **AND** SHALL print a one-line notice that the launch selection was not applied, naming `git paw add`

Test: `tests/emptied_session_refill_e2e.rs::a_selection_on_a_session_with_agents_keeps_the_old_behaviour`
