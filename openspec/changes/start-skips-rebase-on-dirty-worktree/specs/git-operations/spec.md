## ADDED Requirements

### Requirement: Creation-time rebase never runs over uncommitted work

When `create_worktree` would rebase an existing branch, it SHALL first check the tree the rebase would run in: the worktree holding the branch, or the main checkout when no worktree holds it. When that tree has uncommitted changes, it SHALL NOT rebase, SHALL NOT stash, commit, reset, check out over or discard anything, and SHALL proceed on the branch's current HEAD as if `rebase_onto_main` were `false`.

#### Scenario: A surviving dirty worktree is reused without a rebase

- **GIVEN** no saved session, and a worktree for `feat/x` left on disk with a modified tracked file and an untracked file
- **AND** the default branch has gained commits since `feat/x` was based on it
- **WHEN** the user runs `git paw start` for `feat/x` without `--no-rebase`
- **THEN** the command SHALL succeed and the agent SHALL be launched in that worktree
- **AND** `feat/x`'s HEAD SHALL equal its HEAD before the command
- **AND** both files SHALL be byte-identical to their state before the command and `git stash list` SHALL be unchanged

#### Scenario: A branch without a worktree is not rebased in a dirty main checkout

- **GIVEN** a local branch `feat/y` that no worktree holds, and a main checkout with a modified tracked file
- **AND** the default branch has gained commits since `feat/y` was based on it
- **WHEN** the user runs `git paw start --branches feat/y`
- **THEN** the command SHALL succeed and create the worktree for `feat/y` on its current HEAD
- **AND** the main checkout's branch, HEAD and modified file SHALL be unchanged

#### Scenario: A clean worktree on a moved default branch is still rebased

- **GIVEN** a surviving worktree for `feat/x` with no uncommitted changes
- **AND** the default branch has gained commits since `feat/x` was based on it
- **WHEN** the user runs `git paw start` for `feat/x`
- **THEN** `feat/x` SHALL be rebased onto the default branch and no skip notice SHALL be printed

#### Scenario: A conflicting rebase on a clean tree still fails

- **GIVEN** a clean surviving worktree for `feat/x` whose branch conflicts with the advanced default branch
- **WHEN** the user runs `git paw start` for `feat/x`
- **THEN** the command SHALL exit non-zero with an error containing `rebase onto main failed`
- **AND** `feat/x`'s HEAD SHALL equal its HEAD before the command

### Requirement: Creation-time rebase judges uncommitted work as remove does

The creation-time rebase check SHALL use the uncommitted-work detection `remove` and `add` use: a tree is dirty when NUL-delimited `git status --porcelain -z` reports at least one entry that is not git-paw-managed. A tree whose only entries are git-paw-managed SHALL be clean. A tree whose status check fails SHALL be treated as dirty, not as an error.

#### Scenario: git-paw-managed files alone do not block a start rebase

- **GIVEN** a surviving worktree for `feat/x` whose only uncommitted entry is the injected `.git-paw/AGENTS.local.md` sidecar
- **AND** the default branch has gained commits
- **WHEN** the user runs `git paw start` for `feat/x`
- **THEN** `feat/x` SHALL be rebased onto the default branch and no skip notice SHALL be printed

#### Scenario: An unreadable worktree is not rebased

- **GIVEN** a worktree for `feat/x` still registered with git whose status check fails
- **WHEN** `create_worktree` is called for `feat/x` with `rebase_onto_main = true`
- **THEN** no `git rebase` SHALL run and `feat/x`'s tip SHALL be unchanged
- **AND** the returned skip reason SHALL say the check failed

### Requirement: Every launch path reports a skipped rebase

`create_worktree` SHALL return the reason a rebase was skipped. `git paw start` (plain, `--supervisor` and spec-driven) and `git paw add` SHALL print it as one stderr notice naming the branch and `--no-rebase`, never on stdout. The MCP `start_session` tool SHALL return it in `warnings`. With `--no-rebase` no check SHALL run and no notice SHALL be printed.

#### Scenario: start prints the skip notice on stderr

- **GIVEN** a surviving worktree for `feat/x` with uncommitted changes and an advanced default branch
- **WHEN** the user runs `git paw start` for `feat/x`
- **THEN** stderr SHALL contain one notice naming `feat/x` and `--no-rebase`
- **AND** stdout SHALL contain nothing about the skip

#### Scenario: A supervised launch completes over a dirty worktree

- **GIVEN** a surviving worktree for `feat/x` with uncommitted changes and an advanced default branch
- **WHEN** the user runs `git paw start --supervisor` for `feat/x`
- **THEN** the session SHALL launch with `feat/x` on its current HEAD
- **AND** stderr SHALL contain the skip notice naming `feat/x` and `--no-rebase`

#### Scenario: MCP start_session reports the skip as a warning

- **GIVEN** a root MCP session and a surviving worktree for `feat/x` with uncommitted changes
- **WHEN** the client calls `start_session({ "branches": ["feat/x"] })`
- **THEN** the response SHALL be successful and its `warnings` SHALL contain the skip notice naming `feat/x`
- **AND** the MCP server's stdout SHALL carry only protocol messages

#### Scenario: --no-rebase on a dirty worktree prints no notice

- **GIVEN** a surviving worktree for `feat/x` with uncommitted changes
- **WHEN** the user runs `git paw start --no-rebase` for `feat/x`
- **THEN** the branch SHALL NOT be rebased, the changes SHALL be unchanged and no skip notice SHALL be printed

#### Scenario: add keeps its notice text

- **GIVEN** a kept worktree for `feat/x` with uncommitted changes and an advanced default branch
- **WHEN** the user runs `git paw add feat/x`
- **THEN** stderr SHALL contain the same notice `add` printed before the check moved into `create_worktree`
