## ADDED Requirements

### Requirement: start_session accepts per-worker backends

The root-session `start_session` tool SHALL accept an optional `worker_backends` object that maps branch names to `"tmux"`, `"headless"` or `"remote"`, resolved and validated as `git paw start --worker-backend` is, and its advertised parameter schema SHALL list the three values. A start in which any worker fails its backend's gate or the combination rule SHALL create nothing and SHALL return an MCP-level error listing every refused worker with its failed conditions. Omitting the parameter SHALL behave exactly as before this requirement.

#### Scenario: A mixed start through MCP

- **GIVEN** a root MCP session in a repository whose configured worker CLI is eligible for headless
- **WHEN** the client calls `start_session` with `branches` `["feat/a", "feat/b"]` and `worker_backends` `{"feat/b": "headless"}`
- **THEN** the session SHALL hold `feat/a` as a tmux worker and `feat/b` as a headless worker

#### Scenario: A refusal through MCP creates nothing

- **GIVEN** a root MCP session in a repository with no session
- **WHEN** the client calls `start_session` with `worker_backends` naming a tmux worker in a headless session
- **THEN** the tool SHALL return an MCP-level error naming the worker and the condition
- **AND** no worktree, session file or run state SHALL exist afterwards

#### Scenario: The schema lists the values

- **WHEN** the `start_session` input schema is inspected
- **THEN** it SHALL contain an optional `worker_backends` whose values are `tmux`, `headless` and `remote`
