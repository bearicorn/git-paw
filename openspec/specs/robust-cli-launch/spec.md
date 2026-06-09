# robust-cli-launch Specification

## Purpose
TBD - created by archiving change supervisor-cli-launch-robustness-v0-6-x. Update Purpose after archive.
## Requirements
### Requirement: Clean the shell input line before the CLI-launch command

The system SHALL ensure a pane's shell input line is clean before sending
the CLI-launch command — by sending a clearing keystroke (e.g. `C-u`/`C-c`)
and/or a leading newline — so a pending shell startup prompt (auto-update
confirmation, MOTD, etc.) cannot swallow the leading character of the launch
command and strand the pane at a bare shell.

#### Scenario: Launch keystroke is not corrupted by a startup prompt

- **GIVEN** a pane whose interactive shell shows a startup prompt (e.g.
  `[oh-my-zsh] Would you like to update? [Y/n]`) at launch time
- **WHEN** git-paw sends the CLI-launch command
- **THEN** the pane SHALL clear the pending prompt first so the full launch
  command (not a keystroke-truncated variant like `laude-oss`) reaches the
  shell and the CLI starts

### Requirement: Suppress shell startup prompts in the launched pane

The system SHALL suppress known shell auto-update / confirmation prompts in
the pane it launches where it controls the pane environment (e.g. exporting
`DISABLE_AUTO_UPDATE=true` or the equivalent), so such a prompt cannot fire
mid-launch. The system SHALL NOT modify the user's global shell
configuration.

#### Scenario: Auto-update prompt suppressed for the launched pane

- **WHEN** git-paw launches a CLI pane
- **THEN** it SHALL set the pane environment so the shell's auto-update
  prompt does not fire during launch, without editing the user's `~/.zshrc`
  or global oh-my-zsh settings

### Requirement: Verify the CLI started and retry once

The system SHALL verify, within a bounded window after the launch keystroke,
that the pane's CLI actually started (the shell prompt was replaced by the
CLI), and SHALL retry the launch once if the first attempt did not take.

#### Scenario: Failed launch is retried

- **GIVEN** a pane where the first CLI-launch attempt did not start the CLI
  (the shell prompt is still present after the bounded window)
- **THEN** git-paw SHALL send the launch command once more before giving up,
  so a single swallowed attempt does not permanently strand the pane

