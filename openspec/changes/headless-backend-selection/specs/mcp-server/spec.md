## ADDED Requirements

### Requirement: start_session accepts a terminal backend

The root-session `start_session` tool SHALL accept an optional `backend` parameter whose accepted values are exactly `"tmux"` and `"headless"`, resolved as `git paw start --backend` resolves it, and its advertised parameter schema SHALL list both values. A headless start through the tool SHALL apply the same eligibility gate as the command line, SHALL create nothing when any worker is refused, and SHALL return an MCP-level error listing every refused worker with its failed conditions. Omitting the parameter SHALL behave exactly as before this requirement.

#### Scenario: A headless start through MCP

- **GIVEN** a root MCP session in a repository whose configured worker CLI is eligible
- **WHEN** the client calls `start_session` with `branches` `["feat/a"]` and `backend` `"headless"`
- **THEN** a headless session SHALL exist with one worker on `feat/a`
- **AND** no tmux session SHALL have been created

#### Scenario: The gate applies through MCP

- **GIVEN** a root MCP session whose configured worker CLI has no headless form
- **WHEN** the client calls `start_session` with `backend` `"headless"`
- **THEN** the tool SHALL return an MCP-level error naming the CLI and the missing headless form
- **AND** no worktree, session file or run state SHALL exist afterwards

#### Scenario: The schema lists the values

- **WHEN** the `start_session` input schema is inspected
- **THEN** it SHALL contain an optional `backend` whose values are `tmux` and `headless`
