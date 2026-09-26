## ADDED Requirements

### Requirement: Bare-session add and remove refusal states the limitation plainly

When `git paw add` or `git paw remove` is invoked against a session started in bare (no-supervisor) mode, the command SHALL refuse with an error that names the session and states the limitation plainly: hot-attach and hot-detach are supported only for sessions started in supervisor mode, and a bare session must be stopped and restarted with the full branch set. The error SHALL NOT claim that supervisor mode is the default and SHALL NOT cite a release version.

#### Scenario: add against a bare session explains the limitation without stale claims

- **GIVEN** an active session started with `--no-supervisor`
- **WHEN** `git paw add feat/x` is run
- **THEN** it SHALL exit non-zero with an error naming the session and stating that add/remove require a supervisor-mode session
- **AND** the error SHALL NOT contain "the default"
- **AND** the error SHALL NOT contain "v0.6.0" or any other release version

#### Scenario: remove against a bare session gives the same plain refusal

- **GIVEN** an active session started with `--no-supervisor`
- **WHEN** `git paw remove feat/x` is run
- **THEN** it SHALL exit non-zero with the same plain explanation and no stale claims
