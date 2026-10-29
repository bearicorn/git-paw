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

The bundled `broker.sh` and `sweep.sh` helpers SHALL each refuse to run any subcommand — exiting non-zero with a message naming the delegate run, contacting no broker, publishing nothing and sending no keystroke to any pane — when their environment carries `GIT_PAW_DELEGATE`, as `git paw __authority` does. A delegate runs at the repository's main working tree, where the helpers would resolve the session's broker address from the repository configuration, and where a `setup-cli` grant lets them run without a prompt: `broker.sh` could publish as any agent id it names, and `sweep.sh` could publish orchestrator status or learnings, or type into a worker's pane. A delegate's result is its standard output, never a broker message or a keystroke.

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
