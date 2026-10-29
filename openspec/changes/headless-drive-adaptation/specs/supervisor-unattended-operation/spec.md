## ADDED Requirements

### Requirement: Permission prompts are neither scanned nor escalated on a non-interactive surface

The drive loop SHALL NOT scan a surface whose backend reports it non-interactive for permission prompts: for such a surface the prompt decision SHALL return no effects and SHALL leave its state unchanged whatever the surface's captured output contains. It SHALL record no `permission-prompt` escalation, manual-decision record or learning for that surface, and SHALL send it no key, text or buffered-input submission. Surfaces of an interactive backend SHALL be evaluated exactly as before: a live permission prompt on them is escalated per *Every live permission prompt is escalated without blocking the wave*, and no keystroke answers it.

#### Scenario: Prompt-shaped output on a headless surface yields no effect

- **GIVEN** a non-interactive surface whose captured output ends in text shaped like a live permission prompt
- **WHEN** the prompt decision is called for it
- **THEN** it SHALL return no effects and an unchanged state
- **AND** no `permission-prompt` escalation SHALL be among the effects

#### Scenario: A sweep sends nothing to a headless surface

- **GIVEN** the fake backend in non-interactive mode, scripted with prompt-shaped output and a buffered-input-shaped tail on its only surface
- **WHEN** the drive loop runs one sweep
- **THEN** the fake SHALL have recorded no text or key operation for that surface
- **AND** no escalation SHALL have been published for that surface

#### Scenario: tmux surfaces still escalate

- **GIVEN** an interactive surface showing a live permission prompt
- **WHEN** the prompt decision is called for it
- **THEN** it SHALL return the same `permission-prompt` escalation effects it returned before this requirement
- **AND** no effect SHALL send a keystroke to that surface

### Requirement: Drive-loop text reaches a non-interactive surface as a message

When the drive loop delivers text to a non-interactive surface — a correction re-engagement, a continuation, or any other drive-loop text — it SHALL send the text and then `Enter` through the terminal port and SHALL count the delivery as done when both operations succeed and as failed, with the port's error recorded in the effect log, otherwise. For such a surface it SHALL NOT wait a settle delay, re-capture the surface to verify submission, clear and re-type the input, or escalate `nudge-wedged`.

#### Scenario: Re-engagement on a headless surface is one message

- **GIVEN** `auto_loopback` enabled, a gate failure for a worker on a non-interactive surface, and the fake backend
- **WHEN** the correction pass runs
- **THEN** the fake SHALL have recorded the re-engagement text followed by `Enter` for that surface and nothing else
- **AND** the effect log SHALL record the delivery as done

#### Scenario: A failed delivery is logged, not retried by keystrokes

- **GIVEN** a non-interactive surface whose backend fails the `Enter` operation
- **WHEN** the drive loop delivers a re-engagement text
- **THEN** the effect log SHALL record the delivery as failed with the backend's error
- **AND** no clear-line key SHALL be sent

### Requirement: Headless turn outcomes escalate and never destroy work

For each non-interactive surface the drive loop SHALL decide, from the surface's reported activity, its captured output, its resolved CLI's unavailable markers and the current time, as follows: a *failed* turn whose output tail contains one of the CLI's unavailable markers SHALL be escalated `cli-unavailable`, deduplicated on the agent and the marker; any other *failed* turn SHALL be escalated `turn-failed` with its exit code, once per failed turn; the first *interrupted* turn of an agent SHALL be answered by delivering the bundled continuation text, and any later interrupted turn of the same agent SHALL be escalated `turn-interrupted`; a *busy* surface whose output has not grown for `[drive] no_progress_window_seconds` SHALL be escalated `no-progress`, at most once per window. The drive loop SHALL NOT kill a surface, re-run a message, or retry a failed turn on its own.

#### Scenario: Turn outcomes map to effects

- **WHEN** the turn decision is called for each of these observations
  - failed with exit code 1, output tail containing the CLI's usage-limit marker
  - failed with exit code 1, no marker in the tail
  - interrupted, first time for the agent
  - interrupted, second time for the agent
  - busy, output last grew longer ago than the no-progress window
  - busy, output grew within the window
  - awaiting input
- **THEN** the effects SHALL be: a `cli-unavailable` escalation; a `turn-failed` escalation carrying code 1; delivery of the continuation text; a `turn-interrupted` escalation; a `no-progress` escalation; none; and none, respectively

#### Scenario: A repeated failure observation escalates once

- **GIVEN** a surface whose last turn failed and was already escalated `turn-failed`
- **WHEN** the turn decision is called again with the same observation
- **THEN** it SHALL return no effects

#### Scenario: Nothing is killed

- **GIVEN** the fake backend with a surface scripted busy with no output for twice the no-progress window
- **WHEN** the drive loop runs several sweeps
- **THEN** the fake SHALL have recorded no kill for that surface
- **AND** exactly one `no-progress` escalation SHALL have been published for each elapsed window
