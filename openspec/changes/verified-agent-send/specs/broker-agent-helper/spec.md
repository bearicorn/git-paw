## ADDED Requirements

### Requirement: sweep.sh tell delivers a message to an agent with verified submission

The bundled `sweep.sh` SHALL provide a `tell <agent> [<message…>]` subcommand that types a message
into the named coding agent's input by delegating to a hidden `git paw __send` subcommand that
receives the message on stdin, never in its arguments. It SHALL print one result line — `delivered`,
`wedged`, or `refused: <reason>` — and exit `0`, `2` or `3` respectively. `__send` SHALL be hidden
from `git paw --help`.

#### Scenario: The message is not in the process arguments

- **WHEN** `sweep.sh tell feat-x "<message>"` invokes `git paw __send`
- **THEN** the message text SHALL NOT appear in `__send`'s argument list

#### Scenario: __send is hidden from help

- **WHEN** `git paw --help` is inspected
- **THEN** `__send` SHALL NOT appear as a listed subcommand

### Requirement: sweep.sh tell reads stdin and reports a failing __send as an error

Given no message arguments, `sweep.sh tell <agent>` SHALL read the message from its own stdin, so the
message is in no process's arguments. It SHALL trust `__send`'s exit status only together with the
matching result line, and SHALL exit `4` when `__send` is missing or fails, so an older git-paw is
never reported as `wedged`.

#### Scenario: The message can stay out of the helper's own arguments

- **WHEN** a message is piped into `sweep.sh tell feat-x` with no message arguments
- **THEN** `__send` SHALL receive it on stdin and no argv SHALL contain it

#### Scenario: A missing or failing __send is an error, not wedged

- **GIVEN** a `git paw` that rejects `__send` as an unknown subcommand, or a `__send` that exits `1`
- **WHEN** `sweep.sh tell` runs
- **THEN** it SHALL exit `4` and print no result line

### Requirement: __send submits with the drive loop's actuator and the agent's profile

`git paw __send` SHALL use the same submit-and-verify actuator as the drive loop's nudges, with the
agent's own CLI profile: send the text, wait the CLI's settle delay, send a separate `Enter`, verify
the input is empty, and otherwise clear, re-type and submit up to the bounded number of attempts.

#### Scenario: A paragraph-length message is submitted

- **GIVEN** an idle coding-agent pane whose CLI renders a long typed message as a paste placeholder
- **WHEN** `sweep.sh tell feat-x "<a 400-character message>"` runs
- **THEN** the message SHALL be submitted without a manual `Enter`
- **AND** the subcommand SHALL print `delivered` and exit `0`

#### Scenario: A message that never submits is reported wedged

- **GIVEN** a pane whose input still shows the message after every bounded attempt
- **WHEN** `sweep.sh tell` runs against it
- **THEN** it SHALL print `wedged` and exit `2`

### Requirement: Delivered means a submitted user turn

`git paw __send` SHALL report `delivered` only when the message, or a new paste-buffer placeholder,
appears in the pane as a submitted user turn. An input box that merely emptied, or that the profile
does not recognise, SHALL be reported `wedged`, with a note on stderr saying why.

#### Scenario: An emptied input without a submitted turn is not delivered

- **GIVEN** a pane whose input box is empty after the `Enter` but which shows no new user turn
- **WHEN** `sweep.sh tell` runs against it
- **THEN** it SHALL print `wedged` and exit `2`

### Requirement: sweep.sh tell sends only to an idle coding agent it holds

`git paw __send` SHALL send keystrokes only to a coding-agent surface named in the session roster
and SHALL hold that surface's exclusive per-pane claim (the claim `sweep.sh approve` and the drive
loop's nudges and submits take) for the whole send-and-verify sequence. It SHALL refuse, sending
nothing, a target that is unknown or not a coding agent, and a claim held by another sender.

#### Scenario: A non-agent surface is refused

- **GIVEN** a target that resolves to the dashboard or to no roster agent
- **WHEN** `sweep.sh tell` targets it
- **THEN** no keystroke SHALL be sent and it SHALL exit `3`

#### Scenario: A held claim is refused, not raced

- **GIVEN** the drive loop holds the target pane's claim while it submits a nudge
- **WHEN** `sweep.sh tell` targets the pane
- **THEN** no keystroke SHALL be sent and it SHALL print `refused: pane busy`

### Requirement: __send refuses an unsafe send without typing

`git paw __send` SHALL refuse, sending nothing, when it is invoked from a linked worktree rather than
the repository's main working tree, when the target pane shows a live permission prompt or a
selection menu, when the message exceeds the injected-text limit the drive loop applies to its own
nudges, or when the message is empty or contains a control character.

#### Scenario: A live permission prompt blocks the send

- **GIVEN** a coding-agent pane showing a live permission prompt
- **WHEN** `sweep.sh tell` targets it
- **THEN** no keystroke SHALL be sent
- **AND** it SHALL print `refused: permission prompt showing` and exit `3`

#### Scenario: A selection menu blocks the send

- **GIVEN** a coding-agent pane showing a numbered menu with a cursor or an `Enter to select` footer
- **WHEN** `sweep.sh tell` targets it
- **THEN** no keystroke SHALL be sent and it SHALL print `refused: selection menu showing`

#### Scenario: A send from a linked worktree is refused

- **GIVEN** a shell whose cwd is a linked worktree of the repository with a running session
- **WHEN** `sweep.sh tell feat-y "<message>"` runs there
- **THEN** no keystroke SHALL be sent to any pane
- **AND** it SHALL print a `refused:` line naming the main working tree and exit `3`

#### Scenario: An over-long message is refused

- **GIVEN** a message longer than the drive loop's injected-text limit
- **WHEN** `sweep.sh tell` is invoked with it
- **THEN** no keystroke SHALL be sent and the refusal SHALL point at the broker feedback delivery mode

#### Scenario: An empty or control-character message is refused

- **GIVEN** a message that is empty after flattening, or that contains a control character
- **WHEN** `sweep.sh tell` is invoked with it
- **THEN** no keystroke SHALL be sent and it SHALL exit `3`

### Requirement: __send re-checks the pane before every key

`git paw __send` SHALL re-read the pane before every `Enter` and every `C-u` it sends, and SHALL
stop — reporting `wedged`, since the text is already typed — when a permission prompt or selection
menu has appeared, so no key it sends can answer one.

#### Scenario: A menu that appears mid-send stops the next key

- **GIVEN** a pane that shows a selection menu after the message text was typed
- **WHEN** `__send` is about to send its `Enter`
- **THEN** no `Enter` or `C-u` SHALL be sent and it SHALL print `wedged`

### Requirement: __send types only into an idle agent with a free input line

`git paw __send` SHALL refuse, sending nothing, when the target agent is mid-response or its input
line holds typed text other than a paste-buffer placeholder, so it never appends to or clears a line
someone else typed. Text the CLI draws dim or grey is a suggestion, not typed text. A multi-line
message SHALL be typed as one line, with a note saying so.

#### Scenario: A mid-response agent is refused

- **GIVEN** a coding-agent pane showing its mid-response marker
- **WHEN** `sweep.sh tell` targets it
- **THEN** no keystroke SHALL be sent and it SHALL print `refused: agent is mid-response`

#### Scenario: Text already in the input line is refused

- **GIVEN** a coding-agent pane whose input line holds text that is not a paste placeholder
- **WHEN** `sweep.sh tell` targets it
- **THEN** no keystroke SHALL be sent and it SHALL print `refused: input line holds unsubmitted text`

#### Scenario: A dim prompt suggestion does not block the send

- **GIVEN** an idle pane whose input line shows a prompt suggestion drawn dim or grey
- **WHEN** `sweep.sh tell` targets it
- **THEN** the message SHALL be typed and verified as for an empty input line

#### Scenario: A stuck paste placeholder does not block a re-send

- **GIVEN** a coding-agent pane whose input line shows a paste-buffer placeholder
- **WHEN** `sweep.sh tell` re-sends the intended text
- **THEN** the text SHALL be typed and verified as for an empty input line

#### Scenario: A multi-line message is typed as one line

- **WHEN** `sweep.sh tell` is given a message containing newlines
- **THEN** the text typed SHALL be the message with each run of whitespace collapsed to one space
- **AND** a note on stderr SHALL say it was typed as one line
