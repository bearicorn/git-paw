## MODIFIED Requirements

### Requirement: Dynamic pane count matches input

The number of panes in the session SHALL match the number of `PaneSpec` entries added via the builder. When broker is enabled, the builder SHALL receive an additional `PaneSpec` for the dashboard in pane 0, increasing the total pane count by one.

#### Scenario: Two agent panes plus dashboard created
- **GIVEN** broker is enabled and 2 agent pane specs are added
- **WHEN** the session is built
- **THEN** exactly 3 panes SHALL exist: pane 0 (dashboard) + panes 1-2 (agents)

#### Scenario: Two panes without broker
- **GIVEN** broker is disabled and 2 pane specs are added
- **WHEN** the session is built
- **THEN** exactly 2 panes SHALL exist (same as v0.2.0)

## ADDED Requirements

### Requirement: TmuxSession supports session-level environment variables

The `TmuxSessionBuilder` SHALL support setting session-level environment variables via a `set_environment(key, value)` method. The resulting `set-environment -t <session> <key> <value>` command SHALL be emitted before any `send-keys` commands to ensure all panes inherit the variable.

#### Scenario: set_environment emits correct tmux command
- **GIVEN** `set_environment("GIT_PAW_BROKER_URL", "http://127.0.0.1:9119")` is called on the builder
- **WHEN** the session is built
- **THEN** the command queue SHALL contain `set-environment -t <session> GIT_PAW_BROKER_URL http://127.0.0.1:9119`

#### Scenario: set_environment appears before send-keys
- **GIVEN** a builder with environment variables and pane specs
- **WHEN** the session is built
- **THEN** all `set-environment` commands SHALL appear before any `send-keys` commands in the command queue

#### Scenario: set_environment in dry-run output
- **GIVEN** a builder with `set_environment` called
- **WHEN** the session is rendered as dry-run
- **THEN** the output SHALL include the `tmux set-environment` command string

#### Scenario: Multiple environment variables
- **GIVEN** `set_environment("A", "1")` and `set_environment("B", "2")` are both called
- **WHEN** the session is built
- **THEN** both `set-environment` commands SHALL appear in the command queue
