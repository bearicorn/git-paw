## MODIFIED Requirements

### Requirement: Initial prompt injection via tmux send-keys

After the tmux session is created in detached mode, the system SHALL wait approximately 2 seconds for all panes to reach an interactive state, then inject the initial task prompt for each coding agent pane via a single `tmux send-keys` invocation.

The initial prompt SHALL be derived from the agent's spec content if available, or a default "Begin your assigned task." message if no spec is configured.

The single `tmux send-keys` invocation SHALL pass the prompt text followed by the `Enter` keystroke. On non-paste-aware CLIs (or short prompts that don't trip a paste buffer) this submits the prompt directly. On paste-aware CLIs (e.g. Claude Code v2.1.x) the prompt is captured as a paste buffer that requires additional `Enter` keystrokes to fully submit; the system SHALL NOT attempt blind retry keystrokes at launch — recovery from the paste-buffer state is the responsibility of the supervisor agent via the paste-buffer-recovery sub-case under stall detection in the embedded supervisor skill (see the `agent-skills` capability).

Sending more than one `Enter` at launch SHALL NOT be done, because on fast CLIs (or short prompts that have already submitted) any extra keystroke could accidentally accept a follow-on permission prompt that the agent's first action triggered. The supervisor skill's pane-state inspection (`tmux capture-pane`) provides the only safe way to disambiguate "still in paste buffer" from "submitted and now at a permission prompt".

#### Scenario: Initial prompt is injected after boot delay

- **GIVEN** two coding agent panes have been created
- **WHEN** `cmd_supervisor()` injects initial prompts
- **THEN** `tmux send-keys` SHALL be called for each agent pane with the task prompt followed by `Enter`

#### Scenario: Default prompt when no spec content

- **GIVEN** an agent pane with no spec file assigned
- **WHEN** the initial prompt is injected
- **THEN** the injected text SHALL be a default task prompt (not empty)

#### Scenario: Launch flow sends exactly one Enter per pane

- **GIVEN** N coding agent panes
- **WHEN** the supervisor launch flow runs through the prompt-injection loop
- **THEN** the system SHALL invoke `tmux send-keys` exactly once per pane
- **AND** the invocation SHALL include the prompt text and the `Enter` keystroke
- **AND** the system SHALL NOT emit any additional standalone `Enter` keystrokes to the pane during the launch flow

#### Scenario: Paste-buffer recovery is delegated to the supervisor skill

- **GIVEN** a coding agent pane on a paste-aware CLI (e.g. Claude Code v2.1.x) whose injected long prompt has been captured as a paste-buffer placeholder rather than submitted
- **WHEN** the supervisor agent's monitoring loop next inspects the pane via `tmux capture-pane`
- **THEN** the supervisor SHALL apply the paste-buffer-recovery sub-case from the embedded skill (`agent-skills` capability)
- **AND** the launch flow itself SHALL have already exited; the launch flow is NOT responsible for retrying the keystroke
