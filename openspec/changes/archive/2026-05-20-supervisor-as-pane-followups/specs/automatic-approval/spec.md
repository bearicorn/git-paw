## MODIFIED Requirements

### Requirement: No auto-approval for unsafe or unknown classes

The auto-approver SHALL only fire when classification returns a known-safe class.

When auto-approval declines to fire because the classification is `Unknown` (or any other non-safe class), the prompt SHALL be surfaced to the human via the supervisor pane — by publishing an `agent.question` whose `agent_id` is the originating coding-agent slug and whose `payload.question` describes the unclassified prompt. The supervisor agent (running inside its own tmux pane) consumes that question from its inbox and replies via `tmux send-keys` to the agent pane. The dashboard's role is observation only — the v0.4 "prompts inbox" panel that surfaced these questions inline was removed in this change (see the `dashboard` capability's "No prompt-inbox panel" requirement).

#### Scenario: Unknown class is left for human

- **GIVEN** a detected prompt classified `PermissionType::Unknown`
- **WHEN** the supervisor poll loop runs
- **THEN** auto-approval SHALL NOT fire
- **AND** the prompt SHALL be surfaced to the human via the supervisor pane (typically by publishing an `agent.question` to the broker, which the supervisor agent then handles in its own pane)

#### Scenario: Disabled config disables firing

- **GIVEN** `[supervisor.auto_approve] enabled = false`
- **WHEN** any detected prompt arrives
- **THEN** auto-approval SHALL NOT fire regardless of classification
