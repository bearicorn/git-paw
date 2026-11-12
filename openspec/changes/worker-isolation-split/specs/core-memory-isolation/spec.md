## ADDED Requirements

### Requirement: An opted-out worker still never writes the shared auto memory

A worker whose CLI sets `inherit_operator_config = true` SHALL still launch with its family's auto memory turned off, so no worker writes the memory every worktree of the repository shares, whatever config folder its CLI uses.

#### Scenario: A worker-config-folder CLI keeps memory off

- **GIVEN** `[clis.claude-worker]` with `family = "claude"` and `inherit_operator_config = true`
- **WHEN** a worker using it is launched by `start`, `add` or recovery
- **THEN** its environment SHALL carry `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1`

Test: `tests/worker_operator_isolation_e2e.rs::an_opted_out_worker_keeps_auto_memory_off`
