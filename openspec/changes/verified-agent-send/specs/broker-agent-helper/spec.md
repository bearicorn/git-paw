## ADDED Requirements

### Requirement: sweep.sh tell delivers a message to an agent with verified submission

The bundled `sweep.sh` SHALL provide a `tell <agent> <message…>` subcommand that types a message into
the named coding agent's input and submits it, by delegating to a hidden `git paw __send` subcommand
that receives the message on stdin, never in its arguments. `__send` SHALL use the same
submit-and-verify actuator as the drive loop's nudges, with the agent's own CLI profile: send the
text, wait the CLI's settle delay, send a separate `Enter`, verify the input is empty, and otherwise
clear, re-type and submit up to the bounded number of attempts. It SHALL print one result line —
`delivered`, `wedged`, or `refused: <reason>` — and exit `0`, `2` or `3` respectively. `__send` SHALL
be hidden from `git paw --help`.

#### Scenario: A paragraph-length message is submitted

- **GIVEN** an idle coding-agent pane whose CLI renders a long typed message as a paste placeholder
- **WHEN** `sweep.sh tell feat-x "<a 400-character message>"` runs
- **THEN** the message SHALL be submitted without a manual `Enter`
- **AND** the subcommand SHALL print `delivered` and exit `0`

#### Scenario: A message that never submits is reported wedged

- **GIVEN** a pane whose input still shows the message after every bounded attempt
- **WHEN** `sweep.sh tell` runs against it
- **THEN** it SHALL print `wedged` and exit `2`

#### Scenario: The message is not in the process arguments

- **WHEN** `sweep.sh tell feat-x "<message>"` invokes `git paw __send`
- **THEN** the message text SHALL NOT appear in `__send`'s argument list

#### Scenario: __send is hidden from help

- **WHEN** `git paw --help` is inspected
- **THEN** `__send` SHALL NOT appear as a listed subcommand

### Requirement: sweep.sh tell sends only to an idle coding agent it holds

`git paw __send` SHALL send keystrokes only to a coding-agent surface named in the session roster,
SHALL hold that surface's exclusive per-pane claim (the claim `sweep.sh approve` and the drive loop's
nudges and submits take) for the whole send-and-verify sequence, and SHALL refuse — sending nothing —
when it is invoked from a linked worktree rather than the repository's main working tree, when the
target is unknown or is not a coding agent, when the claim is held by another sender, when the pane
shows a live permission prompt, or when the message exceeds the injected-text limit the drive loop
applies to its own nudges.

#### Scenario: A live permission prompt blocks the send

- **GIVEN** a coding-agent pane showing a live permission prompt
- **WHEN** `sweep.sh tell` targets it
- **THEN** no keystroke SHALL be sent
- **AND** it SHALL print `refused: permission prompt showing` and exit `3`

#### Scenario: A send from a linked worktree is refused

- **GIVEN** a shell whose cwd is a linked worktree of the repository with a running session
- **WHEN** `sweep.sh tell feat-y "<message>"` runs there
- **THEN** no keystroke SHALL be sent to any pane
- **AND** it SHALL print a `refused:` line naming the main working tree and exit `3`

#### Scenario: A non-agent surface is refused

- **GIVEN** a target that resolves to the dashboard or to no roster agent
- **WHEN** `sweep.sh tell` targets it
- **THEN** no keystroke SHALL be sent and it SHALL exit `3`

#### Scenario: A held claim is refused, not raced

- **GIVEN** the drive loop holds the target pane's claim while it submits a nudge
- **WHEN** `sweep.sh tell` targets the pane
- **THEN** no keystroke SHALL be sent and it SHALL print `refused: pane busy`

#### Scenario: An over-long message is refused

- **GIVEN** a message longer than the drive loop's injected-text limit
- **WHEN** `sweep.sh tell` is invoked with it
- **THEN** no keystroke SHALL be sent and the refusal SHALL point at the broker feedback delivery mode
