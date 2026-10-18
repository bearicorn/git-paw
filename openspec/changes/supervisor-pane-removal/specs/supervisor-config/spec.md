## MODIFIED Requirements

### Requirement: Supervisor-specific approval level resolution

The system SHALL continue to parse `[supervisor].approval` and SHALL treat it as the orchestrator's own approval level, distinct from `agent_approval`. With no orchestrator pane inside the session, this field SHALL NOT influence any pane's launch command in this release; it is reserved for git-paw-initiated orchestrator invocations (bound by the `orchestrator-escalation-handoff` change). Coding-agent panes SHALL resolve their flags from `agent_approval` alone — setting `approval` SHALL NOT change any coding-agent pane's launch flags. Setting `approval` SHALL NOT produce a warning or an error.

#### Scenario: approval does not change any pane command

- **GIVEN** a config with `[supervisor]` containing `agent_approval = "auto"` and `approval = "full-auto"` and `cli = "claude"`
- **WHEN** the supervised session launch commands are built
- **THEN** every pane command SHALL be identical to the commands built for the same config without the `approval` key
- **AND** no pane command SHALL include `--dangerously-skip-permissions`

#### Scenario: Coding agents resolve from agent_approval

- **GIVEN** a config with `[supervisor]` containing `agent_approval = "full-auto"` and `approval = "manual"`
- **WHEN** the coding-agent pane commands are built for `cli = "claude"`
- **THEN** each SHALL include `--dangerously-skip-permissions`

#### Scenario: Pre-relocation config with approval still loads

- **GIVEN** a `.git-paw/config.toml` written before this change with `[supervisor] approval = "full-auto"`
- **WHEN** the config is loaded and a session launched
- **THEN** loading SHALL succeed without error or warning
- **AND** the launch SHALL succeed
