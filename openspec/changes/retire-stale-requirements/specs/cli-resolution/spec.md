## MODIFIED Requirements

### Requirement: End-to-end boot registration

The system SHALL boot a fresh supervisor session such that all
coding agents register with the broker without manual
intervention, for any CLI given an adequate settle delay
(default or configured). Registration SHALL NOT depend on
permission grants seeded into any CLI settings file. The agents'
boot publish goes through the bundled `broker.sh` helper, which the
CLI's resolved permission mode and the classifier's managed
helper-script rule allow without a prompt.

#### Scenario: All agents register unattended

- **GIVEN** a fresh supervisor session with N agents and
  broker enabled, using a CLI whose settle delay is adequate
- **WHEN** the session launches
- **THEN** within a bounded window the broker `/status` SHALL
  list all N coding agents (plus the supervisor) with no human
  `Enter` or permission approval required
