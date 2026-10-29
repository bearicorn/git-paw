## MODIFIED Requirements

### Requirement: TmuxSession supports pipe-pane command

The `TmuxSession` builder SHALL support queuing a `pipe-pane` command that attaches the session
recorder to a specific pane. The queued command SHALL be `pipe-pane -o -t <pane>` with a shell
command that runs the installed binary's hidden `__record` subcommand with the capture path and the
pane's size, both paths quoted as `safe-process-invocation` requires.

#### Scenario: pipe-pane queued in builder
- **WHEN** `pipe_pane()` is called on a `TmuxSession` with a pane target and capture path
- **THEN** the command queue SHALL contain a `pipe-pane -o -t <pane>` entry whose shell command invokes `__record` with the quoted capture path

#### Scenario: pipe-pane in dry-run output
- **WHEN** a session with `pipe_pane()` is rendered as dry-run
- **THEN** the output SHALL include the `tmux pipe-pane` command string

#### Scenario: pipe-pane executed after pane creation
- **WHEN** the session commands are executed
- **THEN** the `pipe-pane` command SHALL execute after the corresponding `split-window` and `send-keys` commands for that pane
