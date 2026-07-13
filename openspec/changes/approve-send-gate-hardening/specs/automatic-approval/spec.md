## MODIFIED Requirements

### Requirement: Live-prompt gate

The auto-approver SHALL act on a prompt only when it is LIVE, and SHALL reliably detect both single- and multi-option permission prompts. A prompt is live when the tail of the pane capture contains the prompt's structural markers — the numbered Yes/No option glyphs and/or the `Do you want to proceed?` marker, together with the `Esc to cancel` footer. The inspected window SHALL be wide enough to span a full multi-option prompt block (`Do you want to proceed?` + N options + footer), so a prompt offering a "don't ask again" option is detected rather than missed. When these markers are absent from the tail, the system SHALL treat the capture as containing no live prompt and SHALL NOT send any keystrokes, regardless of classification — preventing a supervisor that is merely narrating about a pane (or a scrolled-away earlier prompt) from tripping a phantom approval.

Before dispatching approval keystrokes, the sender SHALL re-confirm the prompt is still live on a FRESH capture taken immediately prior to sending. If the prompt has cleared in the interim (the agent moved on, or another approver won the race), the sender SHALL send nothing, so approval digits can never land as stray chat input.

The in-tool auto-approver and the bundled `sweep.sh approve` helper SHALL use the same live-prompt markers so their detection agrees.

#### Scenario: Live single-option prompt fires

- **GIVEN** a capture whose tail includes `Esc to cancel` and whose command slice classifies safe
- **WHEN** the poll loop runs
- **THEN** the live-prompt gate SHALL pass and auto-approval MAY fire

#### Scenario: Live multi-option prompt is detected

- **GIVEN** a capture whose tail is a multi-option permission prompt (`Do you want to proceed?`, a numbered option list including a "don't ask again" option, and `Esc to cancel`)
- **WHEN** the poll loop (or `sweep.sh approve`) inspects it
- **THEN** the prompt SHALL be detected as live — the detection SHALL NOT report "no live prompt" merely because `Do you want to proceed?` sits above a narrow tail window

#### Scenario: Footer absent does not fire

- **GIVEN** a capture that mentions a safe command in prose but whose tail does NOT contain the prompt's option glyphs or `Esc to cancel`
- **WHEN** the poll loop runs
- **THEN** the live-prompt gate SHALL fail and the system SHALL NOT send keystrokes

#### Scenario: Prompt cleared before send sends nothing

- **GIVEN** the gate classified a prompt live and decided to approve
- **WHEN** a fresh capture taken immediately before sending shows the prompt has cleared
- **THEN** the sender SHALL send no keystrokes, so no approval digit lands as stray chat input

#### Scenario: Detection agrees across auto-approver and sweep helper

- **WHEN** the same multi-option prompt capture is evaluated by the in-tool auto-approver and by `sweep.sh approve`
- **THEN** both SHALL agree it is a live prompt (shared marker set)

### Requirement: Auto-approval keystroke sequence

When a detected prompt is classified safe, the system SHALL send the agent CLI's "approve and remember" keystroke sequence to the pane via `tmux send-keys`.

The keystroke send SHALL pass through the `broker-mediated-approvals` approval-send gate: immediately before dispatching the keystroke sequence, the system SHALL re-capture the target pane and confirm the prompt's live structural markers are present at the capture's tail, per the Live-prompt gate — a window wide enough to span a full multi-option prompt block, not a fixed ~4-line tail. When the re-confirm capture shows the prompt has cleared between classification and send, the system SHALL dispatch NO keystrokes (no stray input). The system SHALL also refuse to dispatch the sequence into pane index 0 (the supervisor's own pane) via this blind send-keys path.

#### Scenario: Default Claude approval sequence

- **GIVEN** a Claude pane displaying a permission prompt for an allowlisted curl command
- **WHEN** auto-approval fires AND the immediate-before-send re-confirm capture shows the prompt is still live (structural markers present at the tail)
- **THEN** the system SHALL send the keystroke sequence `BTab Down Enter` to the pane via `tmux send-keys -t <session>:<pane>`

#### Scenario: Each keystroke sent separately

- **GIVEN** auto-approval is firing against a re-confirmed live prompt
- **WHEN** the keystrokes are dispatched
- **THEN** the system SHALL invoke `tmux send-keys` once per logical key (`BTab`, `Down`, `Enter`) rather than as a single concatenated string
- **AND** SHALL allow tmux to translate special key names (e.g. `BTab` → back-tab)

#### Scenario: Cleared prompt suppresses the keystroke sequence

- **GIVEN** a prompt that classified safe at detection time
- **WHEN** the immediate-before-send re-confirm capture no longer shows the prompt's live structural markers at the tail
- **THEN** the system SHALL dispatch NO keystrokes to the pane
- **AND** the agent's CLI input SHALL receive no stray characters

#### Scenario: Auto-approval never types into pane 0

- **GIVEN** a safe-classified prompt whose resolved target pane index is 0
- **WHEN** auto-approval would fire
- **THEN** the system SHALL dispatch NO keystrokes via the blind send-keys path
- **AND** the prompt SHALL be left for the non-blind supervisor-pane path

### Requirement: Option-index selection for Yes/No prompts

When dispatching approval keystrokes, the auto-approver SHALL select the option index according to the prompt shape. For a 2-option Yes/No prompt, option 1 SHALL be "Yes" and SHALL be selected. For a 3-option Yes / Yes-and-don't-ask-again / No prompt, the auto-approver SHALL select option 2 (the broad grant) only when the broad-grant rule permits (per "Broad grant restricted to allowlisted non-arbitrary-code verbs"), and otherwise SHALL select option 1 (one-time Yes).

The bundled `sweep.sh approve` helper SHALL follow the same option-index selection: it SHALL parse the numbered option list from the fresh pre-send capture and dispatch the selected option's digit followed by `Enter`. It SHALL NOT dispatch a blind cursor-movement sequence (e.g. `Down` + `Enter`) whose landing option depends on the prompt shape — on a 2-option Yes/No prompt such a sequence selects "No", and on a 3-option prompt it takes the permanent broad grant without consulting the broad-grant rule.

#### Scenario: Two-option prompt selects Yes

- **GIVEN** a live 2-option Yes/No prompt for a safe command
- **WHEN** the auto-approver fires
- **THEN** it SHALL select option 1 (Yes)

#### Scenario: Three-option prompt with allowlisted verb selects the broad grant

- **GIVEN** a live 3-option prompt for an allowlisted non-arbitrary-code command
- **WHEN** the auto-approver fires
- **THEN** it SHALL select option 2 (Yes, and don't ask again)

#### Scenario: Three-option prompt with arbitrary-code runner selects one-time Yes

- **GIVEN** a live 3-option prompt for `python3 -c "..."`
- **WHEN** the auto-approver fires
- **THEN** it SHALL select option 1 (one-time Yes), not option 2

#### Scenario: Sweep helper approves a 2-option prompt affirmatively

- **GIVEN** a live 2-option Yes/No prompt on an agent pane
- **WHEN** the operator runs `sweep.sh approve <pane>`
- **THEN** the helper SHALL send the digit `1` then `Enter` (selecting Yes)
- **AND** SHALL NOT send `Down` + `Enter`

#### Scenario: Sweep helper respects the broad-grant rule on 3-option prompts

- **GIVEN** a live 3-option prompt for an arbitrary-code runner (e.g. `bash -c "..."`)
- **WHEN** the operator runs `sweep.sh approve <pane>`
- **THEN** the helper SHALL select option 1 (one-time Yes), agreeing with the in-tool auto-approver's option resolution
