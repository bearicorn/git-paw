## ADDED Requirements

### Requirement: git paw add reports its result as JSON

`git paw add` SHALL accept `--json`. With it, on success the command SHALL print a `cli-machine-output` document containing `schema_version`, `session`, `branch`, `agent_id` (the broker slug of the branch), `cli`, `worktree_path`, and `paused` (whether the agent was added to a paused session and will start on `git paw start`). The document SHALL NOT expose a terminal pane index. `--json` SHALL NOT add any prompt and SHALL NOT change what `add` does; on failure stdout SHALL be empty.

#### Scenario: add --json reports the added agent

- **GIVEN** an active session
- **WHEN** `git paw add feat/x --cli my-agent --json` succeeds
- **THEN** stdout SHALL be one object with `branch` `"feat/x"`, `agent_id` `"feat-x"`, `cli` `"my-agent"`, the created `worktree_path`, and `paused` `false`

#### Scenario: add --json into a paused session

- **GIVEN** a paused session
- **WHEN** `git paw add feat/y --cli my-agent --json` succeeds
- **THEN** the document SHALL carry `paused` `true`

#### Scenario: A failed add prints nothing on stdout

- **GIVEN** no session for the repository
- **WHEN** `git paw add feat/x --json` is run
- **THEN** stdout SHALL be empty and the command SHALL exit non-zero with its usual error on stderr

### Requirement: git paw remove reports its result as JSON

`git paw remove` SHALL accept `--json`. With it, on success the command SHALL print a `cli-machine-output` document containing `schema_version`, `session`, `branch`, `worktree_path`, and `worktree_kept` (whether `--keep-worktree` left the worktree on disk). `--json` SHALL NOT bypass the uncommitted-work safety check and SHALL NOT add any prompt; a refused or failed removal SHALL leave stdout empty.

#### Scenario: remove --json reports the removed agent

- **GIVEN** an active session containing `feat/x` with a clean worktree
- **WHEN** `git paw remove feat/x --json` succeeds
- **THEN** stdout SHALL be one object with `branch` `"feat/x"`, its `worktree_path`, and `worktree_kept` `false`

#### Scenario: --keep-worktree is reflected

- **WHEN** `git paw remove feat/x --keep-worktree --json` succeeds
- **THEN** the document SHALL carry `worktree_kept` `true`

#### Scenario: The safety check still refuses

- **GIVEN** `feat/x` has uncommitted work and `--force` is not passed
- **WHEN** `git paw remove feat/x --json` is run
- **THEN** the command SHALL refuse exactly as in human mode, exit non-zero, and leave stdout empty
