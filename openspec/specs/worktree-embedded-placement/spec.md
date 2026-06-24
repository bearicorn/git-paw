# worktree-embedded-placement Specification

## Purpose
TBD - created by archiving change worktree-embedded-placement. Update Purpose after archive.
## Requirements
### Requirement: Worktree placement is configurable as child or sibling

The system SHALL support a `worktree_placement` setting with two values:
`"child"` and `"sibling"`. When `worktree_placement` is `"child"`, agent
worktrees SHALL be created inside the repository at
`<repo_root>/.git-paw/worktrees/<branch-slug>/`. When `worktree_placement`
is `"sibling"`, agent worktrees SHALL be created in the repository's
parent directory at `<repo_parent>/<project>-<branch-slug>/`, matching the
v0.7.0 layout.

#### Scenario: Child placement creates worktree inside the repo

- **GIVEN** a repository whose effective config has `worktree_placement = "child"`
- **WHEN** a worktree is created for branch `feat/auth-flow`
- **THEN** the worktree SHALL be created at `<repo_root>/.git-paw/worktrees/feat-auth-flow/`

#### Scenario: Sibling placement creates worktree beside the repo

- **GIVEN** a repository whose effective config has `worktree_placement = "sibling"`
- **WHEN** a worktree is created for branch `feat/auth-flow`
- **THEN** the worktree SHALL be created at `<repo_parent>/<project>-feat-auth-flow/`

### Requirement: Child placement derives the branch slug from the branch name

The system SHALL derive the child-layout branch slug from the branch name
alone, replacing `/` with `-` and stripping characters outside the safe
set of letters, digits, dot, dash, and underscore. The slug SHALL NOT
include the project name, because the directory already resides under that
project's `.git-paw/worktrees/`.

#### Scenario: Branch with a slash maps to a dashed slug

- **GIVEN** `worktree_placement = "child"`
- **WHEN** a worktree is created for branch `feat/auth-flow`
- **THEN** the slug SHALL be `feat-auth-flow`
- **AND** the worktree path SHALL end with `.git-paw/worktrees/feat-auth-flow`

#### Scenario: Branch with unsafe characters has them stripped

- **GIVEN** `worktree_placement = "child"`
- **WHEN** a worktree is created for branch `fix/issue#42`
- **THEN** the slug SHALL be `fix-issue42`
- **AND** the worktree path SHALL end with `.git-paw/worktrees/fix-issue42`

### Requirement: Absent placement config defaults to sibling

The system SHALL behave as if placement were sibling when the effective
config does not specify `worktree_placement`, producing the exact v0.7.0
sibling layout. Pre-existing configs and sessions created before this
field SHALL be unaffected.

#### Scenario: Missing field uses sibling layout

- **GIVEN** a repository whose effective config has no `worktree_placement` field
- **WHEN** a worktree is created for branch `feat/auth-flow`
- **THEN** the worktree SHALL be created at `<repo_parent>/<project>-feat-auth-flow/` (sibling layout)

### Requirement: Sessions record concrete worktree paths for both layouts

The session JSON SHALL record the concrete absolute worktree path produced
at creation time for each worktree, regardless of placement. Resume,
status, and purge SHALL operate on the recorded path and SHALL NOT
re-derive the path from `worktree_placement`, so a session created under
one placement remains resumable and purgeable even if the config later
changes.

#### Scenario: Child-layout session round-trips through resume and purge

- **GIVEN** a session created with `worktree_placement = "child"` whose worktree path is `<repo_root>/.git-paw/worktrees/feat-x/`
- **WHEN** the session is saved, reloaded, and then purged
- **THEN** the reloaded session SHALL report the child path
- **AND** purge SHALL remove the worktree at that recorded child path

#### Scenario: Sibling-layout session round-trips through resume and purge

- **GIVEN** a session created with `worktree_placement = "sibling"` whose worktree path is `<repo_parent>/<project>-feat-x/`
- **WHEN** the session is saved, reloaded, and then purged
- **THEN** the reloaded session SHALL report the sibling path
- **AND** purge SHALL remove the worktree at that recorded sibling path

#### Scenario: Config flip does not orphan an existing session's worktree

- **GIVEN** a session was created under `worktree_placement = "sibling"` with a recorded sibling path
- **AND** the config is later changed to `worktree_placement = "child"`
- **WHEN** the session is purged
- **THEN** purge SHALL remove the worktree at the recorded sibling path, not at a re-derived child path

