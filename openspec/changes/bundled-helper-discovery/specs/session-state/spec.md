## MODIFIED Requirements

### Requirement: Find session by repository path

The system SHALL scan all session files and return the session matching a given repository path. A path that is the root of — or lies inside — a linked worktree of a repository SHALL resolve to that repository's session, by first resolving the worktree to the repository that owns it, so a process started inside an agent's worktree (such as `git paw mcp` spawned by the agent's CLI with no `--repo`) finds the session and its broker. A worktree's own root SHALL NOT be compared directly against a session's repository path.

#### Scenario: Finds correct session among multiple
- **GIVEN** two sessions for different repositories
- **WHEN** `find_session_for_repo()` is called with one repo path
- **THEN** it SHALL return the matching session

Test: `session::tests::finds_correct_session_among_multiple_by_repo_path`

#### Scenario: No matching session
- **GIVEN** saved sessions for other repositories
- **WHEN** `find_session_for_repo()` is called with a different path
- **THEN** it SHALL return `None`

Test: `session::tests::find_returns_none_when_no_repo_matches`

#### Scenario: No sessions directory
- **GIVEN** no sessions directory exists
- **WHEN** `find_session_for_repo()` is called
- **THEN** it SHALL return `None`

Test: `session::tests::find_returns_none_when_no_sessions_exist`

#### Scenario: A worktree root resolves to its repository's session

- **GIVEN** an active session for a repository and one of that session's agent worktrees
- **WHEN** `find_session_for_repo()` is called with the worktree's root
- **THEN** it SHALL return the repository's session

#### Scenario: The MCP server started in a worktree finds the broker

- **GIVEN** an active broker-enabled session and `git paw mcp` started with its working directory inside an agent worktree and no `--repo`
- **WHEN** the server resolves its repository context
- **THEN** its broker URL SHALL be the session's broker URL, not absent

### Requirement: start writes a per-repo session JSON

`git paw start` SHALL write a per-repo session JSON to
`.git-paw/sessions/paw-<project>.json` describing the launched
session. This is the discovery surface the bundled
`sweep.sh` helper reads. The file SHALL be written on launch
and removed on `purge`. Every path through which `git paw start`
creates or rebuilds the session's panes SHALL (re)write it — the
bare launch, the spec-driven launch with or without supervisor
mode, the supervisor launch, recovery of a stopped session, and
restart of a paused session — and each agent entry's `pane_index`
SHALL identify the pane that agent occupies in the resulting
session.

#### Scenario: start writes the per-repo JSON

- **WHEN** `git paw start` (any flags) succeeds
- **THEN** `.git-paw/sessions/paw-<project>.json` SHALL exist
  describing the session (name + agent list)

#### Scenario: A spec-driven launch writes the per-repo JSON

- **WHEN** `git paw start --specs <names>` succeeds without supervisor mode
- **THEN** `.git-paw/sessions/paw-<project>.json` SHALL exist with one
  `agents` entry per launched spec branch

#### Scenario: Recovery rewrites the per-repo JSON

- **GIVEN** a stopped session whose per-repo JSON is missing
- **WHEN** `git paw start` recovers it
- **THEN** `.git-paw/sessions/paw-<project>.json` SHALL exist and list
  every recovered agent with the pane index it occupies in the rebuilt
  session

#### Scenario: Restart from pause rewrites the per-repo JSON

- **GIVEN** a paused session
- **WHEN** `git paw start` resumes it
- **THEN** `.git-paw/sessions/paw-<project>.json` SHALL list every agent
  with the pane index it occupies in the resumed session

#### Scenario: purge removes the per-repo JSON

- **GIVEN** an active session with the per-repo JSON present
- **WHEN** the user runs `git paw purge --force`
- **THEN** the per-repo JSON SHALL be removed
