## ADDED Requirements

### Requirement: Supervisor pane approval flags

When building the supervisor pane's CLI launch command, the system SHALL append the flags resolved from the supervisor's effective approval level (per `supervisor-config`'s "Supervisor-specific approval level resolution") to the supervisor CLI command. This SHALL apply on every path that constructs the supervisor pane command: the `cmd_supervisor` auto-start flow AND session recovery. Coding-agent pane commands SHALL keep resolving their flags from `agent_approval`.

The `--dry-run` plan output SHALL report the supervisor's effective approval level and the agents' approval level as separate lines when they differ.

#### Scenario: Fresh start applies supervisor flags to pane 0 only

- **GIVEN** a config with `[supervisor]` containing `cli = "claude"`, `approval = "full-auto"`, `agent_approval = "auto"`
- **WHEN** `cmd_supervisor()` builds the tmux session
- **THEN** pane 0's command SHALL be `claude --dangerously-skip-permissions`
- **AND** the coding-agent panes' commands SHALL NOT contain `--dangerously-skip-permissions`

#### Scenario: Recovery rebuilds the supervisor pane with the same flags

- **GIVEN** a recoverable session whose config sets `approval = "full-auto"` and `cli = "claude"`
- **WHEN** the session is recovered
- **THEN** the rebuilt supervisor pane command SHALL include `--dangerously-skip-permissions`

#### Scenario: Dry run reports split approval levels

- **GIVEN** a config with `approval = "full-auto"` and `agent_approval = "auto"`
- **WHEN** `git paw start --supervisor --dry-run` prints the session plan
- **THEN** the plan SHALL report the supervisor approval level (`FullAuto`) and the agent approval level (`Auto`) distinctly

#### Scenario: No approval key produces byte-identical commands to v0.10.0

- **GIVEN** a config with `[supervisor]` containing `agent_approval = "auto"` and no `approval` key
- **WHEN** the supervisor session launch commands are built
- **THEN** the supervisor pane and agent pane commands SHALL be identical to those v0.10.0 would build for the same config
