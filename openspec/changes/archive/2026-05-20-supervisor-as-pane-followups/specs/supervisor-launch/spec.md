## MODIFIED Requirements

### Requirement: Supervisor self-registration

The system SHALL register the supervisor agent in the broker so it appears in the dashboard alongside other agents. Registration SHALL be performed by the supervisor agent itself, from inside the supervisor pane, via the existing skill-driven curl POST to `/publish`. The launcher (`cmd_supervisor`) SHALL NOT publish any `agent.status` on behalf of the supervisor before returning.

Specifically:

1. `cmd_supervisor` SHALL NOT call `publish_to_broker_http(..., build_status_message("supervisor", ...))` (or any equivalent self-registration POST) at any point in its flow.
2. The supervisor pane's boot block (rendered by `build_boot_block("supervisor", broker_url)`) and the supervisor skill SHALL together instruct the supervisor agent's CLI to publish an initial `agent.status` message as the very first action after reading `AGENTS.md`. The published message SHALL have `agent_id = "supervisor"`, a phase-appropriate `status` label, and a populated `cli` field identifying the supervisor's CLI (resolved by the supervisor agent from its environment or skill template substitution).
3. When the supervisor pane fails to start (e.g. layout error after `tmux_session.execute()`, missing CLI on PATH, system-level pane spawn failure, any abort path that does not actually launch a supervisor CLI process), no `agent.status` for `agent_id = "supervisor"` SHALL exist in the broker — the dashboard SHALL correctly render no supervisor row in such failure cases.

The "Supervisor row placement" rule on the dashboard (`dashboard` capability) governs how the row is rendered once it does appear; this requirement governs only WHEN the row is allowed to appear.

#### Scenario: cmd_supervisor does not self-publish on behalf of the supervisor

- **GIVEN** `cmd_supervisor()` is called with a valid supervisor config and broker enabled
- **WHEN** the launcher completes all of its tmux-session, save-state, and send-keys steps
- **AND** the launcher reaches its `Ok(())` return
- **THEN** no `agent.status` message with `agent_id = "supervisor"` SHALL have been published by the launcher process
- **AND** the broker's `/status` endpoint SHALL NOT yet contain an entry for `agent_id = "supervisor"` from the launcher's side
- **AND** the broker's message log SHALL NOT contain any entry whose origin is the launcher publishing as `agent_id = "supervisor"`

#### Scenario: Supervisor pane publishes its own initial agent.status

- **GIVEN** a successfully-launched supervisor session in which the supervisor pane's CLI has booted and read its AGENTS.md
- **WHEN** the supervisor agent executes its boot-block instructions
- **THEN** an `agent.status` message with `agent_id = "supervisor"` SHALL be published from inside the supervisor pane via curl
- **AND** the published message's payload SHALL include `cli = Some(<supervisor CLI name>)`
- **AND** the broker's `/status` endpoint SHALL then list `supervisor` among the known agents

#### Scenario: Aborted launch leaves no phantom supervisor row

- **GIVEN** a launch path where `cmd_supervisor()` returns an error (or follows any abort path) AFTER the broker is running but BEFORE the supervisor pane's CLI has executed its boot block
- **WHEN** the dashboard renders a frame
- **THEN** the agent table SHALL NOT contain a `supervisor` row
- **AND** no divider SHALL be rendered above the coding-agent rows

#### Scenario: Non-interactive launch leaves no phantom supervisor row before pane bootstrap

- **GIVEN** a launch path that successfully completes `cmd_supervisor()` but where the supervisor pane's CLI has not yet executed its boot-block curl (the time window between `cmd_supervisor` returning and the supervisor agent's first curl)
- **WHEN** the dashboard renders a frame during this window
- **THEN** the agent table SHALL NOT contain a `supervisor` row
- **AND** the dashboard SHALL render no divider
- **AND** any subsequent frame rendered AFTER the supervisor agent's first curl SHALL include the `supervisor` row pinned to the top (per the dashboard capability's supervisor-row-placement rule)
