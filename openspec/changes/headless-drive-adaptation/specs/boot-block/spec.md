## ADDED Requirements

### Requirement: Headless workers receive the non-interactive addendum

The boot prompt of a worker whose surface is non-interactive SHALL include the bundled headless addendum, which SHALL tell the worker that no one can answer an interactive prompt, that an action its permission policy does not allow will be refused rather than asked about, and that when it needs input or is blocked it reports this as its coordination instructions describe and then ends its turn, after which it will be resumed with a new message. The boot prompt of a worker on an interactive surface SHALL NOT include the addendum and SHALL be byte-identical to its prompt before this requirement.

#### Scenario: A headless worker's boot prompt carries the addendum

- **GIVEN** a worker launched on a non-interactive surface
- **WHEN** its boot prompt is rendered
- **THEN** it SHALL contain the headless addendum's content

#### Scenario: A tmux worker's boot prompt is unchanged

- **GIVEN** a worker launched on a tmux surface
- **WHEN** its boot prompt is rendered
- **THEN** it SHALL NOT contain the headless addendum's content
- **AND** it SHALL equal the boot prompt rendered before this requirement for the same inputs
