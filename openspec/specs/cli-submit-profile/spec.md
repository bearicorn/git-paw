# cli-submit-profile Specification

## Purpose
TBD - created by archiving change claude-oss-launch-v0-6-x. Update Purpose after archive.
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

