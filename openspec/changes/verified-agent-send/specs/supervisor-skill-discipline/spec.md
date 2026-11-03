## ADDED Requirements

### Requirement: Typed delivery to an agent goes through sweep.sh tell

The bundled supervisor skill SHALL instruct the driving agent to deliver any message it types into a
coding agent's input — including `/tell` in `send-keys` delivery mode and the paste-buffer recovery
procedure — through `sweep.sh tell <agent> <message>`, and SHALL NOT instruct it to type a message
with raw `tmux send-keys`.

#### Scenario: The skill routes send-keys delivery through the helper

- **WHEN** the supervisor skill is rendered
- **THEN** its `/tell` send-keys delivery and its paste-buffer recovery guidance SHALL name `sweep.sh tell`
- **AND** SHALL NOT instruct typing a message with raw `tmux send-keys`

### Requirement: The skill explains the sweep.sh tell results

The bundled supervisor skill SHALL explain the three results of `sweep.sh tell`: `delivered`;
`wedged` (escalate or fall back to the broker feedback mode); and `refused` (resolve the stated reason
first; for a live permission prompt, relay it to the human and run `sweep.sh approve` only on the
human's instruction).

#### Scenario: The skill explains the results

- **WHEN** the supervisor skill is rendered
- **THEN** it SHALL describe what to do on `wedged` and on `refused`
- **AND** for a `refused` caused by a live permission prompt it SHALL say to relay the prompt to the human, not to answer it

## MODIFIED Requirements

### Requirement: Delivery mode selection

`/tell` SHALL select a delivery mode using this precedence:
1. When `[supervisor.tell] mode = "send-keys"` is configured AND
   the target's detected mode is `accept-edits`, use
   `sweep.sh tell` to type the prompt into the target's input
   with verified submission.
2. When `[supervisor.tell] mode = "feedback"` (default), publish
   an `agent.feedback` broker message targeted at the agent.
3. When the configured mode is `send-keys` but the target's
   detected mode is `interactive` or `unknown`, fall back to
   `agent.feedback` and emit a stderr note explaining the
   fallback.

#### Scenario: Default delivery uses agent.feedback

- **GIVEN** no `[supervisor.tell] mode` setting (default)
- **WHEN** `/tell feat/auth rebase onto main` runs
- **THEN** the supervisor SHALL publish an `agent.feedback`
  broker message targeted at `feat/auth` carrying the prompt

#### Scenario: send-keys mode targets accept-edits agents

- **GIVEN** `[supervisor.tell] mode = "send-keys"` and an agent
  whose detected mode is `accept-edits`
- **WHEN** `/tell` targets that agent
- **THEN** the supervisor SHALL use `sweep.sh tell` to type
  the prompt into the agent's input

#### Scenario: send-keys mode falls back when target mode is unknown

- **GIVEN** `[supervisor.tell] mode = "send-keys"` and an agent
  whose detected mode is `unknown`
- **WHEN** `/tell` targets that agent
- **THEN** the supervisor SHALL fall back to `agent.feedback`
  delivery and SHALL emit a stderr-side note explaining the
  fallback
