## ADDED Requirements

### Requirement: Creation-time rebase never runs over uncommitted work

`create_worktree` SHALL rebase an existing branch only inside the branch's own worktree, never in or by switching the main checkout. A branch no worktree holds SHALL get its worktree first and be rebased there; if that rebase fails, the new worktree SHALL be removed. When the worktree already holding the branch has uncommitted changes, it SHALL NOT rebase, stash, commit, reset, check out over or discard anything, and SHALL proceed on the branch's current HEAD.

#### Scenario: A surviving dirty worktree is reused without a rebase

- **GIVEN** no saved session, and a worktree for `feat/x` left on disk with a modified tracked file and an untracked file
- **AND** the default branch has gained commits since `feat/x` was based on it
- **WHEN** the user runs `git paw start` for `feat/x` without `--no-rebase`
- **THEN** the command SHALL succeed and the agent SHALL be launched in that worktree
- **AND** `feat/x`'s HEAD SHALL equal its HEAD before the command
- **AND** both files SHALL be byte-identical to their state before the command and `git stash list` SHALL be unchanged

#### Scenario: A branch without a worktree is rebased in its own worktree

- **GIVEN** a local branch `feat/y` that no worktree holds, and a main checkout with a modified tracked file and an untracked file
- **AND** the default branch has gained commits since `feat/y` was based on it
- **WHEN** the user runs `git paw start --branches feat/y`
- **THEN** the command SHALL succeed, create the worktree for `feat/y` and rebase `feat/y` onto the default branch in it
- **AND** the main checkout's branch, HEAD and both files SHALL be unchanged and no skip notice SHALL be printed

#### Scenario: A failed rebase in a new worktree leaves no worktree behind

- **GIVEN** a local branch `feat/y` that no worktree holds and that conflicts with the advanced default branch
- **WHEN** `create_worktree` is called for `feat/y` with `rebase_onto_main = true`
- **THEN** it SHALL return an error containing `rebase onto main failed`
- **AND** no worktree SHALL hold `feat/y`, `feat/y`'s tip SHALL be unchanged and the main checkout SHALL stay on its branch

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

### Requirement: Creation-time rebase never touches the main checkout or a stopped rebase

When the main checkout holds the branch, `create_worktree` SHALL fail before any rebase with an error saying the branch is checked out in the main checkout and to switch it to another branch. It SHALL rebase only in the worktree path it holds or just created, never re-discovering it. A worktree whose git dir has a `rebase-merge` or `rebase-apply` state SHALL NOT be rebased or aborted; the skip reason SHALL say a rebase is already in progress.

#### Scenario: A branch checked out in the main checkout is refused

- **GIVEN** `feat/x` checked out in the main checkout and an advanced default branch
- **WHEN** `create_worktree` is called for `feat/x` with `rebase_onto_main = true`
- **THEN** it SHALL return an error naming the main checkout and telling the user to switch it to another branch
- **AND** `feat/x`'s tip SHALL be unchanged, the main checkout SHALL stay on `feat/x` and no worktree SHALL be created

#### Scenario: A new worktree is rebased by its own path

- **GIVEN** a branch `feat/x` no worktree holds, and a `git worktree list` that fails after the worktree is created
- **WHEN** `create_worktree` is called for `feat/x` with `rebase_onto_main = true`
- **THEN** the rebase SHALL run in the new worktree and the main checkout's branch SHALL never be switched

#### Scenario: A stopped rebase is left in place

- **GIVEN** a surviving worktree for `feat/x` with a clean status and a rebase stopped part-way in it
- **AND** the default branch has gained commits
- **WHEN** `create_worktree` is called for `feat/x` with `rebase_onto_main = true`
- **THEN** the worktree SHALL be reused, no new rebase and no `git rebase --abort` SHALL run, and the stopped rebase SHALL still be in progress
- **AND** the returned skip reason SHALL say a rebase is already in progress

### Requirement: Creation-time rebase judges uncommitted work as remove does

The creation-time rebase check SHALL use the uncommitted-work detection `remove` and `add` use: a tree is dirty when NUL-delimited `git status --porcelain -z` reports at least one entry that is not git-paw-managed. A tree whose only entries are git-paw-managed SHALL be clean. A tree whose status check fails SHALL be treated as dirty, not as an error. The skip reason SHALL name the worktree it checked.

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

`create_worktree` SHALL return the reason a rebase was skipped. `start` (plain, `--supervisor`, spec-driven) and `add` SHALL print it as one stderr notice naming the branch and `--no-rebase`, never on stdout. `add` SHALL check before creating the worktree, so the notice prints even if creation fails, and SHALL also print a skip `create_worktree` returns. MCP `start_session` SHALL return the reason in `warnings` without naming `--no-rebase`. With `--no-rebase` no check or notice SHALL occur.

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
- **WHEN** the client calls `start_session({ "branches": ["feat/x"], "cli": "<cli>" })`
- **THEN** the response SHALL be successful and its `warnings` SHALL contain one skip notice naming `feat/x` and its reason
- **AND** that notice SHALL NOT name `--no-rebase`, which the tool does not accept
- **AND** the MCP server's stdout SHALL carry only protocol messages

#### Scenario: --no-rebase on a dirty worktree prints no notice

- **GIVEN** a surviving worktree for `feat/x` with uncommitted changes
- **WHEN** the user runs `git paw start --no-rebase` for `feat/x`
- **THEN** the branch SHALL NOT be rebased, the changes SHALL be unchanged and no skip notice SHALL be printed

#### Scenario: add keeps its notice text

- **GIVEN** a kept worktree for `feat/x` with uncommitted changes and an advanced default branch
- **WHEN** the user runs `git paw add feat/x`
- **THEN** stderr SHALL contain the same notice `add` printed before the check moved into `create_worktree`
