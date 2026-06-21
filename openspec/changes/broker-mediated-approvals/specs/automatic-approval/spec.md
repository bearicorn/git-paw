## MODIFIED Requirements

### Requirement: Auto-approval keystroke sequence

When a detected prompt is classified safe, the system SHALL send the agent CLI's "approve and remember" keystroke sequence to the pane via `tmux send-keys`.

The keystroke send SHALL pass through the `broker-mediated-approvals` approval-send gate: immediately before dispatching the keystroke sequence, the system SHALL re-capture the target pane and confirm a live permission-prompt marker is present within the last 4 non-blank lines. When the re-confirm capture shows the prompt has cleared between classification and send, the system SHALL dispatch NO keystrokes (no stray input). The system SHALL also refuse to dispatch the sequence into pane index 0 (the supervisor's own pane) via this blind send-keys path.

#### Scenario: Default Claude approval sequence

- **GIVEN** a Claude pane displaying a permission prompt for an allowlisted curl command
- **WHEN** auto-approval fires AND the immediate-before-send re-confirm capture shows the prompt is still live in the last 4 non-blank lines
- **THEN** the system SHALL send the keystroke sequence `BTab Down Enter` to the pane via `tmux send-keys -t <session>:<pane>`

#### Scenario: Each keystroke sent separately

- **GIVEN** auto-approval is firing against a re-confirmed live prompt
- **WHEN** the keystrokes are dispatched
- **THEN** the system SHALL invoke `tmux send-keys` once per logical key (`BTab`, `Down`, `Enter`) rather than as a single concatenated string
- **AND** SHALL allow tmux to translate special key names (e.g. `BTab` → back-tab)

#### Scenario: Cleared prompt suppresses the keystroke sequence

- **GIVEN** a prompt that classified safe at detection time
- **WHEN** the immediate-before-send re-confirm capture no longer shows a permission-prompt marker in the last 4 non-blank lines
- **THEN** the system SHALL dispatch NO keystrokes to the pane
- **AND** the agent's CLI input SHALL receive no stray characters

#### Scenario: Auto-approval never types into pane 0

- **GIVEN** a safe-classified prompt whose resolved target pane index is 0
- **WHEN** auto-approval would fire
- **THEN** the system SHALL dispatch NO keystrokes via the blind send-keys path
- **AND** the prompt SHALL be left for the non-blind supervisor-pane path
