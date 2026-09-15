## MODIFIED Requirements

### Requirement: send-keys nudges send a follow-up Enter

When the drive loop sends a nudge to a pane (any text intended to be submitted), it SHALL submit it reliably rather than relying on a single combined text+Enter or a single follow-up `Enter` — because on paste-aware CLIs the text frequently lands in a paste buffer or on a stale input line that a bare `Enter` / `C-m` does not submit. The nudge path SHALL:

1. Send the text, then apply the per-CLI settle delay (`[clis.<name>].submit_delay_ms`, the same resolver the boot prompt uses, falling back to the agnostic default), then send a separate `Enter`.
2. Verify submission by capturing the pane after the `Enter`, allowing the pane to redraw first (the same settle delay), and judging by the **input box's content, never by exact equality with the sent text**: the nudge counts as still present when the input box is non-empty and shows the nudge's leading text (a wrapped rendering shows only its first visual row on the sigil line; continuation rows carry no sigil) or a paste-buffer placeholder matching the profile's `paste_buffer_pattern`; it counts as submitted only when the input box is empty or the pane has left its idle state (a mid-response marker is showing).
3. When the nudge is still present, perform a robust recovery — clear the input line (`C-u`), re-send the text, wait the settle delay, then `Enter` — up to a bounded number of attempts.
4. Never assume a single combined text+Enter, or a lone follow-up `Enter` against a stale line, submits the nudge.
5. Report a nudge still present after every recovery attempt as wedged: escalate it to the broker with verdict `nudge-wedged` AND include that escalation in the exit summary's escalation list, deduped per pane so one stuck line is reported once.

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

#### Scenario: A wrapped nudge still on the input line is recognised as unsubmitted

- **GIVEN** a nudge longer than one input-box row, and a post-`Enter` capture whose input box shows the nudge's first row on the sigil line followed by continuation rows without a sigil
- **WHEN** the loop verifies submission
- **THEN** it SHALL judge the nudge still present and run the clear + re-type + `Enter` recovery
- **AND** SHALL NOT report the nudge delivered merely because the sigil row differs from the full text

#### Scenario: A paste-buffer placeholder is recognised as unsubmitted

- **GIVEN** a post-`Enter` capture whose input box shows a paste-buffer placeholder (matching `paste_buffer_pattern`) instead of the nudge text
- **WHEN** the loop verifies submission
- **THEN** it SHALL judge the nudge still present and run the recovery

#### Scenario: An empty input box means the nudge was submitted

- **GIVEN** a post-`Enter` capture whose input box is empty, or whose pane now shows a mid-response marker
- **WHEN** the loop verifies submission
- **THEN** it SHALL report the nudge delivered and send no further keystrokes

#### Scenario: Verification waits for the pane to redraw

- **GIVEN** a nudge dispatched with a non-zero per-CLI settle delay
- **WHEN** the loop verifies submission
- **THEN** the verification capture SHALL be taken after waiting the settle delay following the `Enter`, not immediately

#### Scenario: A wedged nudge appears in the exit summary

- **GIVEN** a nudge still present on the input line after every recovery attempt
- **WHEN** the loop exits and prints its summary
- **THEN** the escalation list SHALL contain a `nudge-wedged` entry naming that pane's agent
- **AND** the same wedged line SHALL NOT produce more than one entry across successive sweeps

### Requirement: Idle pane with buffered input is submitted

The drive loop SHALL detect a pane that is idle — showing no live-prompt marker and
no mid-response marker — while its input box holds non-empty unsubmitted text, and
SHALL submit that text: first with a follow-up `Enter` (the keystroke that flushes a
swallowed-`Enter` paste), then — when a fresh capture shows the text still present — with
the same bounded clear (`C-u`) + re-type + `Enter` recovery the nudge path uses, re-typing
the text read from the whole input box (the sigil row and its continuation rows). The loop
SHALL NOT clear text it cannot re-type verbatim: for a paste-buffer placeholder it SHALL send
the `Enter` and, if the placeholder remains, escalate rather than send `C-u`. A line still
present after every attempt SHALL be escalated as `nudge-wedged`, deduped per pane, and
SHALL appear in the exit summary. A lone `Enter` SHALL NOT be the only remedy. This recovers
a directive left buffered when the CLI swallowed the first `Enter`, which neither the
live-prompt scan nor the long stuck-detector would otherwise surface.

#### Scenario: A buffered directive on an idle pane is submitted

- **GIVEN** a pane with no live-prompt marker and no mid-response marker whose input box holds non-empty text
- **WHEN** the drive loop sweeps that pane
- **THEN** it SHALL send a follow-up `Enter` to submit the buffered text
- **AND** if a fresh capture then shows the input box empty, it SHALL send nothing further

#### Scenario: A stale line that survives the Enter is cleared, re-typed, and submitted

- **GIVEN** an idle pane whose input box still shows the same text after the follow-up `Enter`
- **WHEN** the drive loop verifies the pane
- **THEN** it SHALL send `C-u`, re-type the text read from the whole input box, and send `Enter`, up to a bounded number of attempts
- **AND** the re-typed text SHALL include every row of the input box, not only the sigil row

#### Scenario: A paste-buffer placeholder is never cleared

- **GIVEN** an idle pane whose input box shows a paste-buffer placeholder matching `paste_buffer_pattern`
- **WHEN** the drive loop sweeps that pane and the placeholder survives the follow-up `Enter`
- **THEN** it SHALL NOT send `C-u`
- **AND** it SHALL escalate the pane as `nudge-wedged`

#### Scenario: A mid-response pane is left alone

- **GIVEN** a pane showing a mid-response marker (the agent is actively generating)
- **WHEN** the drive loop sweeps it
- **THEN** it SHALL NOT send an `Enter` — the buffered-input detector applies only to an idle pane

#### Scenario: A persistently stranded line is escalated once

- **GIVEN** an idle pane whose stranded text survives the `Enter` and every recovery attempt
- **WHEN** the loop sweeps that pane on this and later ticks
- **THEN** it SHALL record one `nudge-wedged` escalation for that pane
- **AND** the exit summary SHALL list it
- **AND** later sweeps SHALL NOT add duplicate entries for the same line
