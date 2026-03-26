## ADDED Requirements

### Requirement: TmuxSession supports pipe-pane command

The `TmuxSession` builder SHALL support queuing a `pipe-pane` command to attach logging to a specific pane.

#### Scenario: pipe-pane queued in builder
- **WHEN** `pipe_pane()` is called on a `TmuxSession` with a pane target and log path
- **THEN** the command queue SHALL contain a `pipe-pane -o -t <pane> "cat >> <path>"` entry

#### Scenario: pipe-pane in dry-run output
- **WHEN** a session with `pipe_pane()` is rendered as dry-run
- **THEN** the output SHALL include the `tmux pipe-pane` command string

#### Scenario: pipe-pane executed after pane creation
- **WHEN** the session commands are executed
- **THEN** the `pipe-pane` command SHALL execute after the corresponding `split-window` and `send-keys` commands for that pane
