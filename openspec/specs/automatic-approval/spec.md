# automatic-approval Specification

## Purpose
TBD - created by archiving change auto-approve-patterns. Update Purpose after archive.
## Requirements
### Requirement: Auto-approval keystroke sequence

When a detected prompt is classified safe, the system SHALL send the agent CLI's "approve and remember" keystroke sequence to the pane via `tmux send-keys`.

#### Scenario: Default Claude approval sequence

- **GIVEN** a Claude pane displaying a permission prompt for an allowlisted curl command
- **WHEN** auto-approval fires
- **THEN** the system SHALL send the keystroke sequence `BTab Down Enter` to the pane via `tmux send-keys -t <session>:<pane>`

#### Scenario: Each keystroke sent separately

- **GIVEN** auto-approval is firing
- **WHEN** the keystrokes are dispatched
- **THEN** the system SHALL invoke `tmux send-keys` once per logical key (`BTab`, `Down`, `Enter`) rather than as a single concatenated string
- **AND** SHALL allow tmux to translate special key names (e.g. `BTab` → back-tab)

### Requirement: No auto-approval for unsafe or unknown classes

The auto-approver SHALL only fire when classification returns a known-safe class.

#### Scenario: Unknown class is left for human

- **GIVEN** a detected prompt classified `PermissionType::Unknown`
- **WHEN** the supervisor poll loop runs
- **THEN** auto-approval SHALL NOT fire
- **AND** the prompt SHALL be surfaced to the human via the dashboard prompts inbox

#### Scenario: Disabled config disables firing

- **GIVEN** `[supervisor.auto_approve] enabled = false`
- **WHEN** any detected prompt arrives
- **THEN** auto-approval SHALL NOT fire regardless of classification

### Requirement: Auto-approval is logged

Every auto-approval action SHALL be recorded in the broker message log so the human can audit decisions after the session.

#### Scenario: Approval emits broker message

- **GIVEN** auto-approval fires for agent `feat-foo` for a `cargo test` prompt
- **WHEN** the keystrokes are sent
- **THEN** the broker SHALL receive an `agent.status` (or equivalent log) message tagged `auto_approved` containing the agent id and the matched whitelist entry

#### Scenario: Logged before keystrokes

- **GIVEN** auto-approval fires
- **WHEN** the action is recorded
- **THEN** the log entry SHALL be appended before the `tmux send-keys` call so a crash mid-action still leaves an audit trail

### Requirement: Integration with stall detection

Auto-approval SHALL be triggered from the stall-detection loop, not run as an independent timer, so it only fires when an agent is genuinely stuck.

#### Scenario: Healthy agent not approved

- **GIVEN** an agent that is publishing status updates within the stall threshold
- **WHEN** the supervisor poll loop runs
- **THEN** detection SHALL NOT capture its pane and auto-approval SHALL NOT fire

#### Scenario: Stalled agent approved

- **GIVEN** an agent whose `last_seen` is older than the configured stall threshold
- **WHEN** the supervisor poll loop runs
- **THEN** detection SHALL capture its pane and, if classification is safe, auto-approval SHALL fire

