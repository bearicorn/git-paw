## MODIFIED Requirements

### Requirement: End-to-end boot registration

The system SHALL boot a fresh supervisor session such that all
coding agents register with the broker without a human `Enter`,
for any CLI given an adequate settle delay (default or
configured). The agents' boot publish goes through the bundled
`broker.sh` helper at its stable path. Whether that call runs
without a permission prompt is decided by the agent CLI's own
permission system: its resolved permission mode, or an allow
rule for the helper's exact path in that CLI's settings.
Registration SHALL NOT depend on a git-paw command classifier.
When the CLI prompts for the helper, git-paw SHALL NOT answer
the prompt; under `--unattended` the drive loop escalates it
(`supervisor-unattended-operation`).

#### Scenario: All agents register unattended

- **GIVEN** a fresh supervisor session with N agents and
  broker enabled, using a CLI whose settle delay is adequate
  and whose permission system lets the `broker.sh` helper run
- **WHEN** the session launches
- **THEN** within a bounded window the broker `/status` SHALL
  list all N coding agents (plus the supervisor) with no human
  `Enter` or permission approval required

#### Scenario: A prompted boot publish is escalated, not answered

- **GIVEN** an `--unattended` session whose agent CLI raises a
  permission prompt for its `broker.sh` boot publish
- **WHEN** the drive loop sweeps that pane
- **THEN** the prompt SHALL be escalated
- **AND** no keystroke SHALL be sent to that pane
