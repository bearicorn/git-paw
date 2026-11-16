## ADDED Requirements

### Requirement: Unattended launch announces broker readiness before provisioning

`git paw start --unattended` with the broker enabled SHALL print one line `broker ready: <url>` on stderr as soon as the broker host answers, before any worktree's `on_create` runs. An orchestrator that runs the blocking launch in the background SHALL be able to wait for that line before publishing.

#### Scenario: The ready line precedes hook output

- **GIVEN** an unattended launch whose `on_create` prints `hook running`
- **WHEN** stderr and stdout are captured in order
- **THEN** `broker ready: http://127.0.0.1:<port>` SHALL appear before the first `hook running`

#### Scenario: A publish after the ready line succeeds

- **GIVEN** an unattended launch running in the background with a slow `on_create`
- **WHEN** the orchestrator publishes through the broker helper right after `broker ready` appears
- **THEN** the publish SHALL succeed
