## MODIFIED Requirements

### Requirement: Uncommitted-work safety

`remove` SHALL refuse to delete a worktree containing uncommitted
changes unless `--force` is passed. The refusal message SHALL
identify the changed files so the user knows what would be lost.
`--keep-worktree` SHALL bypass this check (since nothing is
deleted from disk).

The uncommitted-work check SHALL ignore git-paw's own managed/injected
files when deciding whether to refuse and when listing changed files.
A path is git-paw-managed when it is the injected sidecar
`.git-paw/AGENTS.local.md`, or when it is the tracked `AGENTS.md` whose
only uncommitted change is the presence of git-paw's managed
`<!-- git-paw:start -->` block (i.e. the file is otherwise unmodified
relative to HEAD). These files are git-paw injection produced by
`start`/`setup_worktree_agents_md`, not the user's uncommitted work, so
they SHALL NOT, on their own, cause `remove` to refuse, and they SHALL
NOT appear in the refusal message. A worktree whose ONLY uncommitted
entries are git-paw-managed files SHALL be treated as clean: the pane
SHALL close, the worktree SHALL be removed, and the session entry SHALL
be dropped without requiring `--force`. Any uncommitted change to a file
that is NOT git-paw-managed — including a user edit to `AGENTS.md`
outside the managed block — SHALL still cause `remove` to refuse without
`--force`, and SHALL be listed in the refusal message.

#### Scenario: Refusal on dirty worktree

- **GIVEN** an agent `feat/x` whose worktree has uncommitted
  changes in `src/foo.rs`
- **WHEN** the user runs `git paw remove feat/x`
- **THEN** the command SHALL exit non-zero, list `src/foo.rs` as
  uncommitted, and instruct the user to commit or pass `--force`,
  leaving the pane and worktree intact

#### Scenario: --force bypasses the safety check

- **GIVEN** the same dirty worktree
- **WHEN** the user runs `git paw remove feat/x --force`
- **THEN** the worktree SHALL be removed despite the uncommitted
  changes

#### Scenario: --keep-worktree skips the safety check

- **GIVEN** the same dirty worktree
- **WHEN** the user runs `git paw remove feat/x --keep-worktree`
- **THEN** the pane SHALL be closed and the session entry SHALL be
  dropped, but the worktree (including uncommitted changes) SHALL
  remain on disk

#### Scenario: Clean just-started worktree with only git-paw-injected files is removed

- **GIVEN** an agent `feat/x` whose worktree was just provisioned by
  `start`, so its only uncommitted entry is the git-paw-injected sidecar
  `.git-paw/AGENTS.local.md` (and/or the managed `<!-- git-paw:start -->`
  block) with no user edits
- **WHEN** the user runs `git paw remove feat/x` without `--force`
- **THEN** the command SHALL succeed, the pane SHALL be closed, the
  worktree SHALL be removed, and the branch entry SHALL be dropped from
  the session JSON
- **AND** the command SHALL NOT report `.git-paw/AGENTS.local.md` or the
  managed block as uncommitted changes

#### Scenario: Genuine user edit still refuses, and managed files are not listed

- **GIVEN** an agent `feat/x` whose worktree contains BOTH a
  user-authored uncommitted change in `src/foo.rs` AND the git-paw-injected
  sidecar `.git-paw/AGENTS.local.md`
- **WHEN** the user runs `git paw remove feat/x` without `--force`
- **THEN** the command SHALL exit non-zero and refuse the removal
- **AND** the refusal message SHALL list `src/foo.rs`
- **AND** the refusal message SHALL NOT list `.git-paw/AGENTS.local.md`
