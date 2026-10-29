## ADDED Requirements

### Requirement: Start prints a resume summary when it resumes a session

When `git paw start` resumes an existing session for the repository — restarting a paused session, recovering a stopped session, or reattaching to an active one — it SHALL print, before attaching (or before the detached-mode attach hint when not attached to a terminal), a summary with one line per coding agent containing: the branch; the CLI; how the agent resumes (`resumes session <id>` when recovery relaunches it with its recorded CLI session id, `starts fresh` when it is relaunched without one, `running` when reattaching to a live agent); and a worktree diff summary — the number of uncommitted files, the number of commits on the branch not in the default branch, and the files changed, insertions and deletions of the branch against its merge base with the default branch. When an agent's worktree directory does not exist, its line SHALL say the worktree is missing and point at `git paw doctor`. A git probe that fails SHALL render that field as unknown and SHALL NOT fail the start. The resume mode shown SHALL be derived from the same decision recovery uses to build the launch command. Starting a fresh session (no existing session) SHALL print no resume summary.

#### Scenario: Recovered session shows resume mode and work per agent

- **GIVEN** a stopped session with `feat/a` (claude, recorded session id `abc`) having 2 uncommitted files and 3 commits ahead, and `feat/b` (no session id) clean and 0 ahead
- **WHEN** the user runs `git paw start`
- **THEN** the output SHALL contain a line for `feat/a` naming `claude`, `resumes session abc`, 2 uncommitted, 3 ahead and its shortstat
- **AND** a line for `feat/b` naming `starts fresh`, 0 uncommitted and 0 ahead

#### Scenario: Paused session restart prints the summary

- **GIVEN** a paused session
- **WHEN** the user runs `git paw start`
- **THEN** the resume summary SHALL be printed before the session is reattached

#### Scenario: Reattach shows running agents

- **GIVEN** an active session with live agents, started from an interactive terminal
- **WHEN** the user runs `git paw start`
- **THEN** the summary SHALL mark each agent `running` before reattaching

#### Scenario: Missing worktree is flagged

- **GIVEN** a stopped session whose `feat/c` worktree directory was deleted by hand
- **WHEN** the user runs `git paw start`
- **THEN** the `feat/c` line SHALL say the worktree is missing and name `git paw doctor`

#### Scenario: Detached start prints the summary before the attach hint

- **GIVEN** a stopped session and a non-interactive invocation
- **WHEN** the user runs `git paw start`
- **THEN** the resume summary SHALL appear on stdout before the "Attach with" hint

#### Scenario: Fresh launch prints no summary

- **GIVEN** no session exists for the repository
- **WHEN** the user runs `git paw start --cli claude --branches feat/x`
- **THEN** no resume summary SHALL be printed
