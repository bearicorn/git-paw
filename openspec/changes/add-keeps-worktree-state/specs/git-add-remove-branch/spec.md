## MODIFIED Requirements

### Requirement: git paw add subcommand

The system SHALL provide a `git paw add <branch-name>` subcommand
that attaches a new worktree and tmux pane to an already-running
session. The subcommand SHALL accept `--cli <id>` to choose the
agent CLI (defaulting to the session's default CLI),
`--from-spec <change>` to resolve the branch name and CLI from a
spec, and `--no-rebase` (boolean, default `false`) to skip rebasing
an existing branch onto the default branch, with the meaning given
in "Worktree creation reuses start conventions". `--no-rebase` MAY be
combined with `--cli` and `--from-spec`, and SHALL carry a `help`
string. The subcommand SHALL fail with an actionable error when no
session is active for the repository.

#### Scenario: Add a branch to a running session

- **GIVEN** an active session with N agent panes
- **WHEN** the user runs `git paw add feat/new-thing`
- **THEN** the system SHALL create a worktree for `feat/new-thing`,
  spawn a new agent pane running the default CLI, and register the
  branch in the session JSON, leaving the existing N panes intact

#### Scenario: Add with an explicit CLI

- **GIVEN** an active session
- **WHEN** the user runs `git paw add feat/x --cli codex`
- **THEN** the new pane SHALL launch the `codex` CLI in the new
  worktree

#### Scenario: Add when no session is active

- **GIVEN** no active session for the repository
- **WHEN** the user runs `git paw add feat/x`
- **THEN** the command SHALL exit non-zero with a message
  explaining there is no active session and suggesting
  `git paw start`

#### Scenario: Add an unknown --cli value

- **WHEN** the user runs `git paw add feat/x --cli nonesuch`
  where `nonesuch` is not a detected CLI
- **THEN** the command SHALL exit non-zero with a message listing
  the detected CLI ids, and SHALL NOT create a worktree or pane

#### Scenario: --no-rebase parses alone and with the other flags

- **GIVEN** `add feat/x --no-rebase --cli my-agent` is passed
- **WHEN** the CLI is parsed
- **THEN** `no_rebase` SHALL be `true` and `cli` SHALL be `Some("my-agent")`
- **AND** `add feat/x` without the flag SHALL parse with `no_rebase` `false`

#### Scenario: --no-rebase appears in add's help

- **WHEN** the user runs `git paw add --help`
- **THEN** the output SHALL list `--no-rebase` with its help string

### Requirement: Worktree creation reuses start conventions

The `add` subcommand SHALL create the worktree using the same
naming convention, base-branch resolution, and idempotent-create
behaviour as `git paw start`. Adding a branch whose worktree
already exists SHALL reuse the existing worktree rather than error.

The `add` subcommand SHALL invoke `create_worktree` with
`rebase_onto_main = !no_rebase`, matching `start`'s `--no-rebase`
semantics: without `--no-rebase`, a branch that already exists
locally SHALL be rebased onto the repository's default branch before
its worktree is opened or reused (subject to "add never rebases over
uncommitted work"); with `--no-rebase`, the branch SHALL NOT be
rebased and its HEAD SHALL be unchanged by `add`. A branch that does
not yet exist SHALL be created as `create_worktree` specifies and
SHALL NOT be rebased, regardless of the flag.

#### Scenario: Worktree naming matches start

- **WHEN** the user runs `git paw add feat/x` in project `myproj`
- **THEN** the created worktree SHALL follow the same path
  convention a `git paw start` launch of `feat/x` would produce

#### Scenario: Idempotent worktree create on re-add

- **GIVEN** a worktree for `feat/x` already exists on disk
- **WHEN** the user runs `git paw add feat/x`
- **THEN** the command SHALL reuse the existing worktree without
  error

#### Scenario: A clean existing branch is rebased by default

- **GIVEN** an active session and a branch `feat/x` whose worktree was kept on disk by `git paw remove feat/x --keep-worktree` and has no uncommitted changes
- **AND** the default branch has gained commits since `feat/x` was last based on it
- **WHEN** the user runs `git paw add feat/x`
- **THEN** `feat/x` SHALL be rebased onto the default branch, as `git paw start` would do for an existing branch
- **AND** the agent SHALL be attached in the reused worktree

#### Scenario: --no-rebase keeps the branch on its base

- **GIVEN** the same session, kept clean worktree and advanced default branch
- **WHEN** the user runs `git paw add feat/x --no-rebase`
- **THEN** `feat/x`'s HEAD after the command SHALL equal its HEAD before the command
- **AND** the agent SHALL be attached in the reused worktree

## ADDED Requirements

### Requirement: add never rebases over uncommitted work

When `git paw add` would rebase a branch that is checked out in an existing worktree, the system SHALL first check that worktree for uncommitted changes, and when it has any, `add` SHALL NOT rebase the branch and SHALL NOT stash, commit, reset, check out over, or otherwise modify or discard any of those changes.

A worktree has uncommitted changes when the uncommitted-work detection used by `remove` (NUL-delimited `git status --porcelain -z`, per "Robust uncommitted-work detection") reports at least one entry — modified, staged, renamed, deleted or untracked — that is not git-paw-managed under "Uncommitted-work safety". A worktree whose only entries are git-paw-managed SHALL be treated as clean.

On a worktree with uncommitted changes, `add` SHALL continue exactly as if `--no-rebase` had been passed: it SHALL reuse the worktree, attach the agent on the branch's current HEAD and exit zero. It SHALL print one notice on stderr (never stdout) that names the branch, says the rebase was skipped because the worktree has uncommitted changes, and names `--no-rebase`. When `--no-rebase` is passed, no check is needed and no skip notice SHALL be printed.

When a rebase that `add` does attempt fails (for example a conflict, or git refusing for a state the detection did not report), `add` SHALL exit non-zero before spawning a pane or changing the session JSON, the branch SHALL be left at its pre-rebase HEAD under `create_worktree`'s abort contract, and the error SHALL name `--no-rebase`.

#### Scenario: A CLI swap on a dirty kept worktree completes

- **GIVEN** an active session whose agent `feat/x` has a modified tracked file and an untracked file in its worktree
- **AND** the default branch has gained commits since `feat/x` was based on it
- **AND** `git paw remove feat/x --keep-worktree` has run
- **WHEN** the user runs `git paw add feat/x --cli my-agent` without `--no-rebase`
- **THEN** the command SHALL exit zero and `feat/x` SHALL be attached with `my-agent` in the same worktree
- **AND** `feat/x`'s HEAD SHALL equal its HEAD before the command
- **AND** the modified and untracked files SHALL be byte-identical to their state before the command
- **AND** `git stash list` SHALL be unchanged
- **AND** stderr SHALL contain a notice naming `feat/x` and `--no-rebase`

#### Scenario: The skip notice stays off stdout

- **GIVEN** a kept worktree with uncommitted changes
- **WHEN** `git paw add` skips its rebase
- **THEN** the notice SHALL be written to stderr and nothing about the skip SHALL be written to stdout

#### Scenario: git-paw-managed files alone do not block the rebase

- **GIVEN** a kept worktree whose only uncommitted entries are git-paw-managed (for example the injected `.git-paw/AGENTS.local.md` sidecar)
- **AND** the default branch has gained commits
- **WHEN** the user runs `git paw add feat/x`
- **THEN** `feat/x` SHALL be rebased onto the default branch
- **AND** no skip notice SHALL be printed

#### Scenario: Explicit --no-rebase on a dirty worktree prints no skip notice

- **GIVEN** a kept worktree with uncommitted changes
- **WHEN** the user runs `git paw add feat/x --no-rebase`
- **THEN** the agent SHALL be attached, the branch SHALL NOT be rebased, the uncommitted changes SHALL be unchanged, and no skip notice SHALL be printed

#### Scenario: A failed rebase names --no-rebase and attaches nothing

- **GIVEN** a kept worktree with no uncommitted changes whose branch conflicts with the advanced default branch
- **WHEN** the user runs `git paw add feat/x`
- **THEN** the command SHALL exit non-zero with an error containing `rebase onto main failed` and `--no-rebase`
- **AND** `feat/x`'s HEAD SHALL equal its HEAD before the command
- **AND** no pane SHALL be spawned and the session JSON SHALL be unchanged
