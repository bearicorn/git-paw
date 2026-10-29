## MODIFIED Requirements

### Requirement: Hook installation by git paw start

`git paw start` SHALL install both the pre-commit (new) and
post-commit (existing) hooks per worktree at session-create
time when, and only when, the session's resolved layers
include the Broker layer (`core-layer-isolation`). This SHALL
hold on every path that creates an agent worktree — the bare
launch, the spec-driven launch, the coordinated launch,
`git paw add`, and the recovery of a stopped session. Without
the Broker layer, no git-paw hook block and no `paw-agent-id`
marker SHALL be written. Hook installation SHALL be
idempotent — re-running `git paw start` against an existing
session SHALL NOT duplicate hook entries.

#### Scenario: Hooks installed in every agent worktree

- **GIVEN** a session whose layers include Broker
- **WHEN** `git paw start` completes
- **THEN** every agent worktree's `.git/hooks/` (or the
  shared worktree hook dir via `core.hooksPath`) SHALL
  contain both `pre-commit` and `post-commit` hooks with
  the branch-guard logic

#### Scenario: Idempotent re-install

- **GIVEN** an existing session whose hooks are already
  installed
- **WHEN** `git paw start` re-runs against the same session
- **THEN** the hooks SHALL remain present and SHALL NOT be
  duplicated

#### Scenario: No hooks without the broker layer

- **GIVEN** a session with only the Manual layer
- **WHEN** `git paw start` completes
- **THEN** the common git directory's `post-commit`,
  `pre-commit` and `pre-push` hooks SHALL contain no
  git-paw managed block
- **AND** no worktree SHALL have a `paw-agent-id` marker

#### Scenario: A coordinated session without a broker section gets hooks

- **GIVEN** `default_mode = "coordinated"` and no `[broker]`
  section
- **WHEN** a coordinated session starts
- **THEN** each agent worktree SHALL have a `paw-agent-id`
  marker and the common hooks SHALL carry the git-paw block

### Requirement: Hooks and markers are cleaned up on purge

When `git paw purge` removes a worktree, the system SHALL delete that worktree's `paw-agent-id` marker file. When a purge leaves no linked worktree carrying a `paw-agent-id` marker in the repository, the system SHALL strip the git-paw block between `HOOK_START_MARKER` and `HOOK_END_MARKER` from each of the common `post-commit`, `pre-commit` and `pre-push` hooks, preserving the user's content outside the block; a hook file whose only content was git-paw's block and its shebang SHALL be deleted. Stripping SHALL NOT alter any line outside git-paw's markers.

#### Scenario: Purge removes the per-worktree marker

- **GIVEN** a worktree with an installed `$GIT_DIR/paw-agent-id` marker
- **WHEN** `git paw purge --force` runs
- **THEN** the worktree directory (and its linked gitdir under `<main>/.git/worktrees/<name>/`, including the marker) is removed

#### Scenario: Purge strips the managed block and keeps the user's hook

- **GIVEN** a common `post-commit` hook with user content followed by git-paw's managed block
- **WHEN** `git paw purge --force` removes the last git-paw worktree
- **THEN** the hook SHALL contain the user's content and no git-paw marker line
- **AND** a commit afterwards SHALL run only the user's hook logic

#### Scenario: A hook file git-paw alone wrote is deleted

- **GIVEN** a common `pre-push` hook containing only a shebang and git-paw's managed block
- **WHEN** `git paw purge --force` removes the last git-paw worktree
- **THEN** the `pre-push` hook file SHALL no longer exist

#### Scenario: Hooks stay while another git-paw worktree remains

- **GIVEN** an agent detached earlier with `git paw remove --keep-worktree`, whose worktree and `paw-agent-id` marker remain on disk
- **WHEN** `git paw purge --force` removes the session's other worktrees
- **THEN** the common hooks SHALL still carry the git-paw block
