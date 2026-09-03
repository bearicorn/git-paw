## ADDED Requirements

### Requirement: Gate vocabulary is configurable

The set of gate names the correction loop recognises as gate verdicts SHALL be configurable under `[supervisor.correction]`. The field SHALL default to git-paw's current gate list — `testing`, `regression`, `spec audit`, `doc audit`, `security audit`, and `scope` — so an installation that does not configure it behaves identically to before.

A consumer whose review process uses different gate names SHALL be able to name them, so their gate verdicts are recognised and start a correction cycle. A tagged message whose tag is not in the configured set SHALL continue to be treated as a non-gate producer and SHALL NOT start a correction cycle.

#### Scenario: Default vocabulary reproduces current behaviour

- **GIVEN** a configuration that does not set the gate vocabulary
- **WHEN** the correction loop evaluates a gate verdict
- **THEN** the recognised gate names SHALL be `testing`, `regression`, `spec audit`, `doc audit`, `security audit`, and `scope`

#### Scenario: Configured vocabulary is recognised

- **GIVEN** a configuration naming a custom gate
- **WHEN** a verdict tagged with that custom gate name arrives
- **THEN** it SHALL be recognised as a gate verdict
- **AND** it SHALL be eligible to start a correction cycle

#### Scenario: Unrecognised tag does not start a correction cycle

- **GIVEN** a configured gate vocabulary
- **WHEN** a tagged message arrives whose tag is not in that vocabulary
- **THEN** it SHALL NOT be treated as a gate verdict
- **AND** it SHALL NOT start a correction cycle

#### Scenario: Existing configuration loads unchanged

- **WHEN** a configuration written before this field existed is loaded
- **THEN** it SHALL load without error
- **AND** the gate vocabulary SHALL be the default list
