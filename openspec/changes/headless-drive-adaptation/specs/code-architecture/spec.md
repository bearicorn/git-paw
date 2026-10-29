## ADDED Requirements

### Requirement: Headless drive and wake decisions are pure functions

The decisions this capability's headless support adds — skipping prompt scanning and escalation on non-interactive surfaces, handling turn outcomes and no-progress, and choosing which idle workers to wake — SHALL each be a pure function of their decision state and observed inputs (surface interactivity and activity, captured output, the resolved CLI's unavailable markers, inbox sequence numbers, logged escalations and the current time) that returns the next state and an ordered list of effects drawn from the existing closed drive-effect enum. They SHALL perform no I/O. The broker host and the drive loop SHALL gather those observations and apply the returned effects through the terminal port and the broker client.

#### Scenario: The wake decision is tested with data alone

- **GIVEN** an observation with one idle worker holding a new addressed message and one busy worker holding one
- **WHEN** the wake decision is called
- **THEN** it SHALL return one submit-text effect, for the idle worker
- **AND** the test SHALL need no fake terminal, no broker and no clock

#### Scenario: The decision module performs no I/O

- **WHEN** a source audit inspects the module holding the headless decisions
- **THEN** it SHALL contain no use of the terminal port, the broker client, `std::process`, `std::fs`, `Instant::now` or `SystemTime::now`
