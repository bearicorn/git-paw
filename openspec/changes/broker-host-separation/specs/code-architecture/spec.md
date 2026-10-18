## ADDED Requirements

### Requirement: Terminal backend hosts detached service surfaces

The `TerminalBackend` trait SHALL provide a way to run a non-interactive service command inside a session without occupying a surface in the agent grid, and to stop it: `spawn_service(session, name, command) -> ServiceHandle` and `stop_service(handle)`, plus a liveness query. The tmux backend SHALL implement a service as a detached window named after the service (created with `new-window -d -n <name>`), so that window 0's pane indices are unaffected and `kill-session` still terminates the service with `SIGHUP`. The fake backend SHALL record spawned and stopped services.

#### Scenario: tmux service is a detached window

- **GIVEN** the `TmuxBackend` and a live session
- **WHEN** `spawn_service(session, "broker", cmd)` is called
- **THEN** a window named `broker` SHALL exist in the session running `cmd`
- **AND** window 0's pane count and indices SHALL be unchanged

#### Scenario: Stopping a service closes its window

- **GIVEN** a running tmux service `broker`
- **WHEN** `stop_service` is called
- **THEN** the `broker` window SHALL be gone
- **AND** the service process SHALL have received `SIGHUP`

#### Scenario: Fake backend records services

- **GIVEN** the fake backend
- **WHEN** a launch flow spawns the broker service through it
- **THEN** the fake SHALL have recorded one `spawn_service` with name `broker` and the host command
