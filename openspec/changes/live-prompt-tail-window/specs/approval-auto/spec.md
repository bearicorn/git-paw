## MODIFIED Requirements

### Requirement: Live-prompt gate

The auto-approver SHALL act on a prompt only when it is LIVE, and SHALL reliably detect both single- and multi-option permission prompts. A prompt is live when the pane capture contains the prompt's structural markers — the numbered Yes/No option glyphs and/or the `Do you want to proceed?` marker, together with the `Esc to cancel` footer — within a window anchored on the modal block itself. Detection SHALL search upward from the end of the capture rather than inspecting a fixed count of trailing lines, and SHALL tolerate a bounded region of non-prompt content between the modal and the end of the capture, so a CLI that renders a task list, status strip, or progress footer below its input area does not hide a live modal. The inspected window SHALL remain wide enough to span a full multi-option prompt block (`Do you want to proceed?` + N options + footer), so a prompt offering a "don't ask again" option is detected rather than missed. The tolerated trailing region SHALL be bounded, so a prompt that has genuinely scrolled out of view still fails the gate. When these markers are absent, the system SHALL treat the capture as containing no live prompt and SHALL NOT send any keystrokes, regardless of classification — preventing a supervisor that is merely narrating about a pane (or a scrolled-away earlier prompt) from tripping a phantom approval.

Before dispatching approval keystrokes, the sender SHALL re-confirm the prompt is still live on a FRESH capture taken immediately prior to sending. If the prompt has cleared in the interim (the agent moved on, or another approver won the race), the sender SHALL send nothing, so approval digits can never land as stray chat input.

The in-tool auto-approver and the bundled `sweep.sh approve` helper SHALL use the same live-prompt markers and the same anchoring discipline so their detection agrees.

#### Scenario: Live single-option prompt fires

- **GIVEN** a capture whose tail includes `Esc to cancel` and whose command slice classifies safe
- **WHEN** the poll loop runs
- **THEN** the live-prompt gate SHALL pass and auto-approval MAY fire

#### Scenario: Live multi-option prompt is detected

- **GIVEN** a capture whose tail is a multi-option permission prompt (`Do you want to proceed?`, a numbered option list including a "don't ask again" option, and `Esc to cancel`)
- **WHEN** the poll loop (or `sweep.sh approve`) inspects it
- **THEN** the prompt SHALL be detected as live — the detection SHALL NOT report "no live prompt" merely because `Do you want to proceed?` sits above a narrow tail window

#### Scenario: Trailing task region does not hide a live modal

- **GIVEN** two captures of the same live multi-option prompt, identical except that one appends a six-line task-list region below the `Esc to cancel` footer
- **WHEN** the live-prompt gate evaluates each
- **THEN** both SHALL be detected as live
- **AND** `sweep.sh approve` SHALL dispatch keystrokes for both rather than reporting that the prompt cleared

#### Scenario: Footer absent does not fire

- **GIVEN** a capture that mentions a safe command in prose but whose tail does NOT contain the prompt's option glyphs or `Esc to cancel`
- **WHEN** the poll loop runs
- **THEN** the live-prompt gate SHALL fail and the system SHALL NOT send keystrokes

#### Scenario: Scrolled-away prompt still fails the gate

- **GIVEN** a capture in which a permission prompt appears only in scrolled-away history, separated from the end of the capture by more content than the bounded trailing region allows
- **WHEN** the live-prompt gate evaluates it
- **THEN** the gate SHALL fail and the system SHALL NOT send keystrokes

#### Scenario: Prompt cleared before send sends nothing

- **GIVEN** the gate classified a prompt live and decided to approve
- **WHEN** a fresh capture taken immediately before sending shows the prompt has cleared
- **THEN** the sender SHALL send no keystrokes, so no approval digit lands as stray chat input

#### Scenario: Detection agrees across auto-approver and sweep helper

- **WHEN** the same multi-option prompt capture is evaluated by the in-tool auto-approver and by `sweep.sh approve`
- **THEN** both SHALL agree it is a live prompt (shared marker set and shared anchoring discipline)

#### Scenario: Parity holds for captures carrying a trailing region

- **WHEN** a capture carrying a trailing task region is evaluated by the in-tool auto-approver and by the bundled `sweep.sh` mirror
- **THEN** both SHALL return the same liveness verdict
