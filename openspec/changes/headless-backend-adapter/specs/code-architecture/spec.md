## ADDED Requirements

### Requirement: The terminal port reports interactivity and surface activity

The `TerminalBackend` trait SHALL report whether the backend's surfaces present an interactive screen, and SHALL report a surface's activity as one of a closed set of domain values — busy, awaiting input, failed (with its exit code or signal), interrupted, or unknown. The tmux backend SHALL report its surfaces as interactive and every surface's activity as unknown, so every decision that reads a tmux surface's screen is unchanged. The headless backend SHALL report its surfaces as non-interactive and SHALL report the activity its turn record determines. The new identifiers SHALL satisfy the port's existing vocabulary audit.

#### Scenario: tmux answers interactive and unknown

- **GIVEN** the tmux backend and a live session with one coding surface
- **WHEN** interactivity and that surface's activity are queried
- **THEN** the backend SHALL report interactive surfaces
- **AND** the surface's activity SHALL be unknown
- **AND** no tmux command SHALL have been issued for the activity query

#### Scenario: Headless answers non-interactive and a known activity

- **GIVEN** the headless backend and a surface that has run one successful turn
- **WHEN** interactivity and that surface's activity are queried
- **THEN** the backend SHALL report non-interactive surfaces
- **AND** the surface's activity SHALL be awaiting input

#### Scenario: The fake scripts activity

- **GIVEN** the fake backend configured as non-interactive with one surface scripted busy
- **WHEN** interactivity and that surface's activity are queried
- **THEN** it SHALL report non-interactive surfaces and the surface busy

### Requirement: Surface specs may carry a non-interactive turn plan

A surface spec SHALL be able to carry, in addition to its launch command, an optional non-interactive turn plan of two commands: one for the surface's first turn and one for every later turn. A backend whose surfaces are non-interactive SHALL require the plan and SHALL fail to spawn a surface without one; the tmux backend SHALL ignore the plan, and its emitted argv for a spec with a plan SHALL equal its argv for the same spec without one.

#### Scenario: tmux ignores the plan

- **GIVEN** the same coding-surface spec with and without a turn plan
- **WHEN** each is spawned through the tmux backend and the emitted command strings are compared
- **THEN** the two sets of command strings SHALL be identical

#### Scenario: A non-interactive backend requires the plan

- **WHEN** a spec without a turn plan is spawned through the headless backend
- **THEN** the spawn SHALL fail with an error naming the surface

### Requirement: Operations a backend cannot perform fail with a typed error

A backend SHALL fail an operation it cannot perform with a typed unsupported-operation error that names the backend and the operation, and SHALL NOT silently do nothing, except for detaching clients and applying a layout, which a backend without clients or tiles SHALL treat as successful no-ops. Callers SHALL be able to distinguish this error from an I/O failure.

#### Scenario: Unsupported key on headless

- **WHEN** an approval digit is sent to a headless surface
- **THEN** the call SHALL return the unsupported-operation error naming the headless backend
- **AND** the error SHALL be distinguishable from an I/O error by its type

#### Scenario: Layout is a no-op on headless

- **WHEN** a layout is applied to a headless session
- **THEN** the call SHALL succeed and change nothing

### Requirement: Headless process control is confined to the headless adapter

Detaching a process into a new session, signalling a process group, and spawning the turn runner SHALL be confined to the headless adapter's modules and the runner's command handler. No other module under `src/` SHALL declare or call `setsid` or `kill`, or construct the runner's command line. A standing source-audit test SHALL enforce this alongside the existing tmux confinement audit, and SHALL name the offending file on failure. Each `unsafe` foreign declaration the adapter uses SHALL carry a safety comment stating why the call is sound.

#### Scenario: A stray process-group signal fails the audit

- **GIVEN** a call that signals a process group added to `src/supervisor/poll.rs`
- **WHEN** the source-audit test runs
- **THEN** it SHALL fail and name `src/supervisor/poll.rs`

#### Scenario: The adapter's unsafe declarations are documented

- **WHEN** the headless adapter's source is inspected by the audit
- **THEN** every `unsafe` block or foreign declaration in it SHALL be preceded by a `SAFETY:` comment
