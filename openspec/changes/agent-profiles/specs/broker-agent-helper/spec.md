## ADDED Requirements

### Requirement: The authority helper refuses inside a delegate run

`git paw __authority` SHALL refuse — exiting non-zero, printing no credential — when its environment carries `GIT_PAW_DELEGATE`, in addition to its existing refusal from a linked worktree, so a delegate run's own tooling cannot obtain the session authority credential through it. `sweep.sh verified` and `sweep.sh feedback-gate` invoked inside a delegate run SHALL therefore fail without publishing.

#### Scenario: No credential inside a delegate run

- **GIVEN** an active broker-enabled session and `GIT_PAW_DELEGATE=spec-audit` in the environment at the main working tree
- **WHEN** `git paw __authority` runs
- **THEN** it SHALL exit non-zero and print no credential

#### Scenario: A delegate cannot publish a verdict through the helper

- **GIVEN** the same environment
- **WHEN** `sweep.sh verified feat-x` runs
- **THEN** it SHALL exit non-zero
- **AND** the broker SHALL record no `agent.verified` for `feat-x`

### Requirement: The agent helpers refuse inside a delegate run

The bundled `broker.sh` and `sweep.sh` helpers SHALL each refuse every subcommand when their environment carries `GIT_PAW_DELEGATE`: they SHALL exit non-zero with a message naming the delegate run, contact no broker, publish nothing and send no keystroke to any pane. A delegate runs at the main working tree, where the helpers would otherwise reach the session's broker and panes; a delegate's result is its standard output, never a broker message or a keystroke.

#### Scenario: broker.sh publishes nothing inside a delegate run

- **GIVEN** an active broker-enabled session and `GIT_PAW_DELEGATE=explorer` in the environment at the main working tree
- **WHEN** the bundled `broker.sh` runs `--agent feat-x status working`
- **THEN** it SHALL exit non-zero with a message naming the delegate run
- **AND** the broker SHALL record no message for `feat-x`

#### Scenario: sweep.sh publishes nothing inside a delegate run

- **GIVEN** an active broker-enabled session and `GIT_PAW_DELEGATE=spec-auditor` in the environment at the main working tree
- **WHEN** the bundled `sweep.sh` runs `learn` with a learning body
- **THEN** it SHALL exit non-zero with a message naming the delegate run
- **AND** the broker SHALL record no learning

#### Scenario: The helpers are unaffected outside a delegate run

- **GIVEN** the same session and no `GIT_PAW_DELEGATE` in the environment
- **WHEN** the bundled `broker.sh` runs `--agent feat-x status working`
- **THEN** it SHALL publish the status as before

### Requirement: The send helper refuses inside a delegate run

`git paw __send` SHALL refuse with its refused exit status, sending no keystroke to any pane, when its environment carries `GIT_PAW_DELEGATE`, in addition to its existing refusals, so a delegate at the main working tree cannot type into a worker by calling it directly instead of through `sweep.sh tell`.

#### Scenario: A direct send from a delegate run is refused

- **GIVEN** an active session with an idle coding agent `feat-x` and `GIT_PAW_DELEGATE=explorer` in the environment at the main working tree
- **WHEN** `git paw __send --agent=feat-x` runs with a message on stdin
- **THEN** it SHALL exit `3` with a `refused:` line naming the delegate run
- **AND** no keystroke SHALL reach any pane
