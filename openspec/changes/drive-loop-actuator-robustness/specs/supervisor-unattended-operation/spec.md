## MODIFIED Requirements

### Requirement: send-keys nudges send a follow-up Enter

When the drive loop sends a nudge to a pane (any text intended to be submitted), it SHALL submit it reliably rather than relying on a single combined text+Enter or a single follow-up `Enter` — because on paste-aware CLIs the text frequently lands in a paste buffer or on a stale input line that a bare `Enter` / `C-m` does not submit. The nudge path SHALL:

1. Send the text, then apply the per-CLI settle delay (`[clis.<name>].submit_delay_ms`, the same resolver the boot prompt uses, falling back to the agnostic default), then send a separate `Enter`.
2. Verify submission by capturing the pane after the `Enter`; if the nudge text is still present on the input line, perform a robust recovery — clear the input line (`C-u`), re-send the text, then `Enter` — up to a bounded number of attempts.
3. Never assume a single combined text+Enter, or a lone follow-up `Enter` against a stale line, submits the nudge.

#### Scenario: Nudge applies the per-CLI settle delay between text and Enter

- **GIVEN** the drive loop nudges a pane with submittable text
- **WHEN** the keystrokes are dispatched
- **THEN** the loop SHALL send the text, wait the per-CLI settle delay, then send a separate `Enter`
- **AND** SHALL NOT rely on a single combined text+Enter to submit the nudge

#### Scenario: A stale input line is recovered with clear + re-type + Enter

- **GIVEN** a nudge whose text remains on the pane's input line after the follow-up `Enter`
- **WHEN** the loop verifies submission via `capture-pane` and finds the text still present
- **THEN** it SHALL clear the input line (`C-u`), re-send the text, and send `Enter`, up to a bounded number of attempts
- **AND** a bare `Enter` / `C-m` alone SHALL NOT be treated as sufficient recovery

## ADDED Requirements

### Requirement: The drive loop stops nudging panes during wind-down

The drive loop SHALL stop sending pane nudges once it has entered its exit / wind-down state (wave complete, stopped, or bound session gone) — it SHALL NOT continue typing nudge text into idle agent panes during or after wind-down. No nudge SHALL be dispatched on a sweep after the loop has decided to exit, so agents are not left with unsubmitted nudge text re-accumulating in their input lines after the loop is gone.

#### Scenario: No nudge is sent after wind-down begins

- **GIVEN** a drive loop that has entered its exit / wind-down state
- **WHEN** a subsequent sweep would otherwise nudge an idle pane
- **THEN** no nudge keystrokes SHALL be dispatched to any pane

#### Scenario: Wind-down does not re-accumulate unsubmitted text

- **GIVEN** idle agent panes at the moment the drive loop winds down
- **WHEN** the loop completes its exit
- **THEN** it SHALL NOT have appended further nudge text to any idle pane's input line during wind-down
