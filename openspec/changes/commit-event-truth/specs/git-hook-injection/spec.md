## ADDED Requirements

### Requirement: Commits replayed by a rebase are not published

The post-commit dispatcher SHALL publish nothing — no `agent.artifact`, no branch-mismatch feedback, no learning — for a commit made while a `git rebase` or `git am` is in progress in the committing worktree. A single cherry-pick or an amend outside a rebase SHALL still publish as a commit.

#### Scenario: A rebase of many commits publishes no committed artifacts

- **GIVEN** an agent worktree with the dispatcher installed, a running broker, and a branch of 27 commits behind its base
- **WHEN** `git rebase <base>` replays all 27 commits without conflict
- **THEN** the broker SHALL receive no `agent.artifact` with `status: "committed"` for that agent during the rebase

#### Scenario: An amend still publishes

- **GIVEN** an agent worktree with no rebase in progress
- **WHEN** the agent runs `git commit --amend`
- **THEN** the broker SHALL receive one `agent.artifact` with `status: "committed"` for that agent

### Requirement: A finished rebase publishes one rewrite event

git-paw SHALL install a `post-rewrite` hook beside the dispatcher, with the same managed-block preservation and marker gate. When git invokes it for a rebase it SHALL publish exactly one `agent.artifact` with `status: "rewritten"`, the new tip in `sha`, and the union of the rewritten commits' files in `modified_files`. It SHALL publish nothing for an amend.

#### Scenario: One event per rebase

- **GIVEN** an agent worktree whose branch has 5 commits
- **WHEN** the branch is rebased onto a new base without conflict
- **THEN** the broker SHALL receive exactly one `agent.artifact` with `status: "rewritten"` and `sha` equal to the new `HEAD`

#### Scenario: An existing post-rewrite hook is preserved

- **GIVEN** a common git dir with a user-written `hooks/post-rewrite`
- **WHEN** git-paw installs its hooks
- **THEN** the user's content SHALL be preserved and git-paw's block SHALL sit between the managed markers

### Requirement: The commit event names exactly the commit's files and SHA without locking the index

The dispatcher SHALL build `modified_files` from the files changed by the new commit alone, including a root commit, and SHALL carry the commit's SHA in the artifact's `sha`. Uncommitted files SHALL NOT appear. Neither the dispatcher nor the rewrite hook SHALL take the worktree's `index.lock`.

#### Scenario: Uncommitted files are not reported as committed

- **GIVEN** an agent worktree with `a.rs` staged and `b.rs` modified but unstaged
- **WHEN** the agent commits
- **THEN** the artifact's `modified_files` SHALL be `["a.rs"]` and its `sha` SHALL equal the new `HEAD`

#### Scenario: A root commit lists its files

- **GIVEN** an agent worktree on an orphan branch with no commits
- **WHEN** the agent makes its first commit adding `README.md`
- **THEN** the artifact's `modified_files` SHALL contain `README.md`

### Requirement: An orchestrator's commit is not published as the worker's

git-paw SHALL set `GIT_PAW_ACTOR=orchestrator` in the orchestrator CLI's environment and SHALL NOT set it for any coding agent. When it is set, the post-commit and post-rewrite hooks SHALL publish nothing, whichever worktree the commit lands in.

#### Scenario: The orchestrator commits in a worker's worktree

- **GIVEN** a running session and agent `feat-x`
- **WHEN** a process with `GIT_PAW_ACTOR=orchestrator` runs `git -C <feat-x worktree> commit`
- **THEN** the broker SHALL receive no `agent.artifact` for `feat-x` and no verify nudge

#### Scenario: Worker panes do not inherit the marker

- **GIVEN** a freshly started coordinated session
- **WHEN** a coding agent's pane environment is inspected
- **THEN** `GIT_PAW_ACTOR` SHALL be unset
