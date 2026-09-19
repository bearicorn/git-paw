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
