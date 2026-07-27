# cli-launch Specification

## Purpose
Reliably submits a boot block across CLIs by injecting the prompt text literally and then sending `Enter` as a separate keystroke after a settle delay resolved per CLI from `[clis.<name>].submit_delay_ms` (with a single CLI-agnostic default and no hardcoded CLI names), and hardens CLI-pane launch so a shell startup prompt cannot strand the pane at a bare shell — clearing the shell input line before sending the launch command, suppressing known auto-update/confirmation prompts in the launched pane's environment, and verifying the CLI started within a bounded window, retrying the launch once on failure — so a fresh supervisor session boots and all agents self-register unattended.

## Requirements
### Requirement: Boot prompt submitted via split-send + settle delay

The boot-injection path SHALL inject the boot block into a pane
literally and then submit it with a SEPARATE `Enter` sent after
a settle delay, rather than a same-call trailing `Enter`. This
split is what reliably submits a large paste across CLIs
(W15-1: a same-call trailing `Enter` left the boot block
unsubmitted on a custom CLI). The mechanism SHALL contain no
hardcoded CLI names.

#### Scenario: Boot block is injected then submitted separately

- **WHEN** a boot block is injected into a pane
- **THEN** the system SHALL first send the prompt text
  (literally, no `Enter`), then after the settle delay send
  `Enter` as a separate keystroke

#### Scenario: Mechanism is CLI-name-free

- **WHEN** the submit path is inspected
- **THEN** it SHALL NOT branch on any specific CLI name — the
  same split-send applies to every CLI

### Requirement: Settle delay is config-driven with an agnostic default

The settle delay SHALL be resolved per CLI from
`[clis.<name>].submit_delay_ms`, falling back to a single
CLI-agnostic default (`DEFAULT_SUBMIT_DELAY_MS`) for any CLI
without an override. The resolver SHALL key on the leading
binary token of the CLI command (so a CLI string carrying
flags still matches its config entry).

#### Scenario: Unconfigured CLI uses the agnostic default

- **GIVEN** a CLI with no `[clis.<name>].submit_delay_ms`
  configured (or no `[clis.<name>]` entry at all)
- **WHEN** the settle delay is resolved
- **THEN** it SHALL equal `DEFAULT_SUBMIT_DELAY_MS`

#### Scenario: Per-CLI override is honoured

- **GIVEN** `[clis.mycli].submit_delay_ms = 2500`
- **WHEN** the settle delay for `mycli` is resolved
- **THEN** it SHALL be 2500

#### Scenario: Resolver keys on the binary, not the flags

- **GIVEN** `[clis.mycli].submit_delay_ms = 2500`
- **WHEN** the delay is resolved for the CLI command
  `"mycli --some-flag"`
- **THEN** it SHALL be 2500 (the leading token `mycli`
  matched the config entry)

#### Scenario: No CLI name resolves to a hardcoded value

- **GIVEN** an empty `[clis]` config
- **WHEN** the delay is resolved for any CLI id (including
  names that might otherwise be special-cased)
- **THEN** every CLI SHALL resolve to the same
  `DEFAULT_SUBMIT_DELAY_MS` — there is no built-in per-name
  table

### Requirement: Profile applies to supervisor and agent panes

The split-send + resolved delay SHALL apply to every launched
pane, including the supervisor pane (itself a CLI instance).
The supervisor's delay is resolved from the supervisor CLI;
the agents' delay from the agent CLI.

#### Scenario: Supervisor pane boot block is submitted

- **GIVEN** any supervisor session
- **WHEN** the supervisor pane's boot block is injected
- **THEN** it SHALL be submitted via the split-send using the
  supervisor CLI's resolved delay, so the supervisor begins
  its loop without a manual `Enter`

### Requirement: End-to-end boot registration

The system SHALL boot a fresh supervisor session such that all
coding agents register with the broker without manual
intervention, for any CLI given an adequate settle delay
(default or configured) and broker-curl seeding.

#### Scenario: All agents register unattended

- **GIVEN** a fresh supervisor session with N agents and
  broker enabled, using a CLI whose settle delay is adequate
- **WHEN** the session launches
- **THEN** within a bounded window the broker `/status` SHALL
  list all N coding agents (plus the supervisor) with no human
  `Enter` or permission approval required

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
