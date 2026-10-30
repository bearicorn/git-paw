## ADDED Requirements

### Requirement: Typed delivery to an agent goes through sweep.sh tell

The bundled supervisor skill SHALL instruct the driving agent to deliver any message it types into a
coding agent's input — including `/tell` in `send-keys` delivery mode and the paste-buffer recovery
procedure — through `sweep.sh tell <agent> <message>`, and SHALL NOT instruct it to type a message
with raw `tmux send-keys`. The skill SHALL explain the three results: `delivered`; `wedged` (escalate
or fall back to the broker feedback mode); and `refused` (resolve the stated reason first; for a live
permission prompt, relay it to the human and run `sweep.sh approve` only on the human's
instruction).

#### Scenario: The skill routes send-keys delivery through the helper

- **WHEN** the supervisor skill is rendered
- **THEN** its `/tell` send-keys delivery and its paste-buffer recovery guidance SHALL name `sweep.sh tell`
- **AND** SHALL NOT instruct typing a message with raw `tmux send-keys`

#### Scenario: The skill explains the results

- **WHEN** the supervisor skill is rendered
- **THEN** it SHALL describe what to do on `wedged` and on `refused`
- **AND** for a `refused` caused by a live permission prompt it SHALL say to relay the prompt to the human, not to answer it
