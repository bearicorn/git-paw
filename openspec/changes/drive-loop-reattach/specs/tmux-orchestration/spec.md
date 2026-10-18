## MODIFIED Requirements

### Requirement: Start reattaches to an existing live session instead of forking

`git paw start` SHALL reattach to the repository's existing live session rather than creating a numerically-suffixed parallel session (`paw-<project>-2`) when that repository already has a live session (resolved by repository path, per `session-state` find-by-repository). The `-N` collision suffix SHALL be reserved for genuinely distinct sessions (e.g. two different repositories that share a project name), not a re-invocation of `start` against the same repository's running session. In a non-interactive context where reattaching is not possible, `start` SHALL refuse with an actionable error (or require an explicit opt-in flag) rather than silently forking a parallel wave.

With `--unattended`, `start` SHALL instead bind a drive loop to the live session, whether or not the invocation is interactive (per `supervisor-unattended-operation`, *A drive loop can be bound to a live supervised session*). It SHALL NOT attach the terminal, SHALL NOT create a suffixed parallel session, and SHALL refuse only for the reasons that requirement and *At most one drive loop drives a session instance* state. Without `--unattended` the behaviour above is unchanged.

#### Scenario: Re-start of a repo with a live session reattaches

- **GIVEN** a repository with a live session `paw-x`
- **WHEN** the user runs `git paw start` again in that repository (interactive)
- **THEN** the system SHALL reattach to `paw-x` and SHALL NOT create `paw-x-2`

#### Scenario: A distinct repository sharing a name still gets its own session

- **GIVEN** a live session `paw-x` belonging to a different repository
- **WHEN** `git paw start` runs in a second repository whose project name is also `x`
- **THEN** the system SHALL resolve a distinct session name (`paw-x-2`) for the second repository

#### Scenario: Non-interactive re-start refuses rather than forking

- **GIVEN** a repository with a live session and a non-interactive (non-TTY) invocation
- **WHEN** `git paw start` runs again in that repository without `--unattended`
- **THEN** the system SHALL refuse with an actionable error (or require an explicit opt-in flag) rather than silently creating a suffixed parallel session

Test: `tests/supervisor_reattach_integration.rs::supervisor_restart_refuses_rather_than_forking_a_parallel_session` (unchanged).

#### Scenario: Non-interactive re-start with --unattended binds a drive loop

- **GIVEN** a repository with a live supervised session `paw-x`, no drive loop running, and a non-interactive (non-TTY) invocation
- **WHEN** `git paw start --unattended` runs again in that repository
- **THEN** the system SHALL bind a drive loop to `paw-x` and run it to an exit condition
- **AND** SHALL NOT create `paw-x-2`
- **AND** SHALL NOT print the non-interactive reattach refusal

Test: `tests/unattended_drive_e2e.rs::unattended_rebinds_a_loop_to_a_live_session_after_heartbeat`.
