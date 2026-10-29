## ADDED Requirements

### Requirement: Surface specs may carry a remote task plan

A surface spec SHALL be able to carry an optional remote task plan: the repository as the vendor names it, the base branch and base commit the session starts from, the session title, the task prompt, and, when re-attaching, the recorded vendor session. The remote backend SHALL require the plan and SHALL fail to spawn a surface without one; the tmux and headless backends SHALL ignore it, and the tmux backend's emitted argv for a spec with a plan SHALL equal its argv for the same spec without one.

#### Scenario: tmux ignores the remote plan

- **GIVEN** the same coding-surface spec with and without a remote task plan
- **WHEN** each is spawned through the tmux backend and the emitted command strings are compared
- **THEN** the two sets of command strings SHALL be identical

#### Scenario: The remote backend requires the plan

- **GIVEN** the remote backend
- **WHEN** a spec without a remote task plan is spawned through it
- **THEN** the spawn SHALL fail with an error naming the surface

### Requirement: Remote follow decisions are pure functions

The follow's decisions — which questions and plans to escalate, which answers to send, which events to render, when to list events, and when to stop following — SHALL be one pure function of the follow state and the observed inputs (the session's state and last-update time, its events, the read outcome, the broker messages observed since the follow's cursor, the configured time cap and the current time) that returns the next state and an ordered list of effects drawn from a closed enum. It SHALL perform no I/O. The broker host SHALL gather the observations through the remote agent service port and the broker, apply the returned effects in order, and log each applied effect with its outcome.

#### Scenario: The follow decision is tested with data alone

- **GIVEN** a follow state with no escalated question and an observation of a session awaiting feedback with one agent-message event
- **WHEN** the follow decision is called
- **THEN** it SHALL return one effect publishing an `agent.question` for the worker
- **AND** the test SHALL need no fake service, no broker and no clock

#### Scenario: The follow decision module performs no I/O

- **WHEN** a source audit inspects the module holding the follow decision
- **THEN** it SHALL contain no use of the remote agent service port, the terminal port, the broker client, `std::process`, `std::fs`, `Instant::now` or `SystemTime::now`

### Requirement: Vendor network access is confined to the vendor adapters

Every network request to a remote agent vendor SHALL be made only by that vendor's adapter module under `src/remote/`, and no other module SHALL construct a vendor URL, add a vendor credential to a request, or start the vendor transport. A standing source-audit test SHALL enforce this and SHALL name the offending file on failure.

#### Scenario: A stray vendor call fails the audit

- **GIVEN** a request to a vendor host added to `src/supervisor/drive.rs`
- **WHEN** the source-audit test runs
- **THEN** it SHALL fail and name `src/supervisor/drive.rs`
