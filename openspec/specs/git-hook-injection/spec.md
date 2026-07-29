# git-hook-injection Specification

## Purpose
Installs a shared post-commit dispatcher and pre-push block hook in the common git dir plus a per-worktree `paw-agent-id` marker, so that committing in any worktree publishes an `agent.artifact` (under the correct agent id and pre-expanded broker URL) while pushes are blocked. Hooks preserve pre-existing user content between managed markers, no-op outside a git-paw session, and are cleaned up on purge. It also guards against cross-worktree branch contamination in shared `.git/refs`: a pre-commit hook refuses a commit whose worktree branch does not match the branch being advanced (opt-out via `strict_branch_guard = false`), a post-commit hook publishes `agent.feedback` and a `permission_pattern` learning on mismatch, both hooks install idempotently per worktree, and the coordination skill teaches agents to stay inside their worktree.

## Requirements
### Requirement: Install post-commit dispatcher in the common git dir (shared hook pattern)

During worktree setup, the system SHALL install a `post-commit` git hook at the **common** git directory (`git rev-parse --git-common-dir`) — i.e. the main repository's `.git/hooks/post-commit`. This approach is necessary because git does not provide per-worktree hook directories without the experimental `extensions.worktreeConfig` feature, which is not suitable for production use.

**Implementation Note:** The system uses a dual-directory strategy:
- **Common git dir** (`--git-common-dir`) for shared hooks (identical across all worktrees)
- **Linked git dirs** (`--git-dir`) for per-worktree marker files (worktree-specific state)

This pattern allows the dispatcher hook to be shared while each worktree maintains its own agent identity and broker URL.

The hook SHALL be a POSIX-compatible shell script that:

1. Reads the per-worktree `$GIT_DIR/paw-agent-id` marker file (see requirement below). `$GIT_DIR` is set by git to the correct per-worktree linked gitdir when the hook runs, so the dispatcher picks up the right agent id regardless of which worktree the commit came from.
2. Sources the marker file to recover `PAW_AGENT_ID` and `PAW_BROKER_URL`.
3. Publishes `agent.artifact` to the broker with `modified_files` built from `git diff HEAD~1 --name-only` and `status: "committed"`.
4. Uses the pre-expanded broker URL from the marker (no shell variable expansion from the user's environment).
5. Does not block the commit on broker failure (`|| true`).
6. Is no-op when `$GIT_DIR/paw-agent-id` is not present, so repos without a git-paw session continue to work.

#### Scenario: Commit triggers artifact publish

- **GIVEN** a worktree with the post-commit dispatcher installed, a `$GIT_DIR/paw-agent-id` marker, and a running broker
- **WHEN** the agent runs `git commit`
- **THEN** the broker receives an `agent.artifact` message with the committed files in `modified_files` and the correct `agent_id` from the marker

#### Scenario: Broker failure does not block commit

- **GIVEN** a worktree with the post-commit dispatcher installed and NO running broker
- **WHEN** the agent runs `git commit`
- **THEN** the commit succeeds (hook exits 0 despite curl failure)

#### Scenario: Existing post-commit hook is preserved

- **GIVEN** a common git dir that already has a `<common>/hooks/post-commit` file
- **WHEN** git-paw installs its dispatcher
- **THEN** the existing hook content is preserved and the git-paw dispatcher block is appended between `# >>> git-paw managed hook >>>` and `# <<< git-paw managed hook <<<` marker lines
- **AND** re-installing the hook replaces only the git-paw block between the markers, never the user's content

#### Scenario: Dispatcher is a no-op outside a git-paw session

- **GIVEN** a repository where the dispatcher was installed by a previous session but the marker file has been removed (`git-paw purge`)
- **WHEN** the user runs `git commit`
- **THEN** the hook exits 0 with no broker side effect

### Requirement: Install per-worktree agent marker file

During worktree setup, the system SHALL write a shell-sourceable marker file at `$GIT_DIR/paw-agent-id` — where `$GIT_DIR` is the linked worktree's private gitdir (`git rev-parse --git-dir` inside the worktree, equivalent to `<main>/.git/worktrees/<name>/` for linked worktrees, or `<main>/.git/` for the main worktree).

The marker file SHALL contain exactly two lines:

```
PAW_AGENT_ID=<slugified branch name>
PAW_BROKER_URL=<fully-qualified broker URL>
```

Both values SHALL be pre-expanded at install time so the dispatcher hook performs no shell variable substitution of user-controlled values at commit time.

#### Scenario: Marker encodes the agent id and broker URL

- **GIVEN** a worktree set up for agent `feat-x` with broker at `http://127.0.0.1:9119`
- **WHEN** git-paw installs the marker
- **THEN** `$GIT_DIR/paw-agent-id` contains `PAW_AGENT_ID=feat-x` and `PAW_BROKER_URL=http://127.0.0.1:9119`

#### Scenario: Two linked worktrees have independent markers

- **GIVEN** a repository with linked worktrees `feat-a` and `feat-b`
- **WHEN** git-paw installs markers for both
- **THEN** `feat-a`'s `$GIT_DIR/paw-agent-id` contains `PAW_AGENT_ID=feat-a` and `feat-b`'s marker contains `PAW_AGENT_ID=feat-b`
- **AND** a commit in either worktree publishes under the correct agent id via the shared dispatcher

### Requirement: Install pre-push block hook in the common git dir

During worktree setup, the system SHALL install a `pre-push` git hook at `<common>/hooks/pre-push` that unconditionally blocks all push attempts with exit code 1 and an error message on stderr.

Because the pre-push hook is identical for every worktree (it reads no per-worktree state), a single common hook suffices.

#### Scenario: Push is blocked

- **GIVEN** a worktree with the pre-push hook installed
- **WHEN** the agent runs `git push`
- **THEN** the push is blocked with exit code 1
- **AND** stderr contains "agents must not push"

### Requirement: Hooks and markers are cleaned up on purge

When `git paw purge` removes a worktree, the system SHALL delete that worktree's `paw-agent-id` marker file. The shared dispatcher and pre-push hooks in the common git dir MAY be left installed because they are idempotent and no-op when no marker is present; however, removing the last worktree SHOULD strip the git-paw block between `HOOK_START_MARKER` and `HOOK_END_MARKER` from the common post-commit hook so the user's pre-existing hook content remains intact.

#### Scenario: Purge removes the per-worktree marker

- **GIVEN** a worktree with an installed `$GIT_DIR/paw-agent-id` marker
- **WHEN** `git paw purge --force` runs
- **THEN** the worktree directory (and its linked gitdir under `<main>/.git/worktrees/<name>/`, including the marker) is removed

#### Scenario: Dispatcher stays idempotent after purge

- **GIVEN** a common post-commit dispatcher installed by a prior session
- **WHEN** `git paw purge --force` runs and removes the last worktree marker
- **THEN** subsequent commits from non-git-paw branches execute the dispatcher, find no marker, and exit 0 with no broker side effect

### Requirement: Post-commit hook detects branch mismatch

The post-commit hook installed by `git paw start` SHALL
detect when a commit landed on a branch that does not match
the worktree's expected branch and SHALL publish an
`agent.feedback` to the offending agent identifying the
mismatch. Because git worktrees share `.git/refs`, a bash
session inside one worktree that `cd`s (or uses absolute
paths) into another worktree's checkout can advance the
wrong branch's ref when it commits.

#### Scenario: Commit on expected branch is silent

- **GIVEN** an agent working in worktree
  `feat/foo` whose worktree HEAD matches `feat/foo`
- **WHEN** the agent commits
- **THEN** the post-commit hook SHALL NOT publish a branch-
  mismatch warning (this is the normal case)

#### Scenario: Commit on integration branch from agent worktree triggers warning

- **GIVEN** an agent working in worktree `feat/foo` whose
  bash session has been hijacked into the supervisor's
  checkout
- **WHEN** the agent's `git commit` advances
  `feat/v0.6.0-specs` (or any branch other than `feat/foo`)
  instead of its own branch
- **THEN** the hook SHALL publish an `agent.feedback`
  message identifying:
  - The expected branch (`feat/foo`)
  - The branch that was actually advanced
  - The commit SHA
  - A recommended remediation (cherry-pick + reset)

### Requirement: Hook publishes scope-violation learning

The post-commit hook SHALL emit an `agent.learning` record
with category `permission_pattern` (per
[[quality-learnings]]) whenever a branch mismatch is
detected. The body SHALL include the expected branch, the
actually-advanced branch, the commit SHA, and a one-line
description. This pre-figures the qualitative-learning
category for cross-worktree contamination so dogfood
patterns surface in the learnings file.

#### Scenario: Branch mismatch produces a learning record

- **GIVEN** a detected branch-mismatch commit
- **WHEN** the hook publishes
- **THEN** the broker SHALL receive both the
  `agent.feedback` AND an `agent.learning` with
  `category: "permission_pattern"` and a body identifying
  the contamination

### Requirement: pre-commit guard refuses cross-worktree commit

The system SHALL install a pre-commit hook (alongside the
v0.4.0 post-commit hook) that verifies the worktree's
checked-out branch matches the branch the commit would
advance. When they mismatch, the hook SHALL exit non-zero
with a clear error and SHALL NOT permit the commit. Users
who deliberately need to commit cross-worktree (rare) SHALL
opt out via `[supervisor] strict_branch_guard = false`.

#### Scenario: Pre-commit guard blocks cross-worktree commit

- **GIVEN** a bash session in worktree `feat/foo` whose
  HEAD is `feat/foo` but the current `HEAD` ref (via
  `git symbolic-ref`) points elsewhere because of a stale
  `cd`
- **WHEN** `git commit` runs
- **THEN** the pre-commit hook SHALL exit non-zero with a
  message identifying the mismatch and SHALL NOT create the
  commit

#### Scenario: Same-worktree commit passes

- **GIVEN** a normal in-worktree commit
- **WHEN** `git commit` runs
- **THEN** the pre-commit guard SHALL pass and the commit
  SHALL proceed

#### Scenario: Opt-out config disables the guard

- **GIVEN** `[supervisor].strict_branch_guard = false`
- **WHEN** any commit runs
- **THEN** the pre-commit guard SHALL NOT block the commit;
  the post-commit `agent.feedback` warning still fires
  (detection without enforcement)

### Requirement: Hook installation by git paw start

`git paw start` SHALL install both the pre-commit (new) and
post-commit (existing) hooks per worktree at session-create
time. Hook installation SHALL be idempotent — re-running
`git paw start` against an existing session SHALL NOT
duplicate hook entries.

#### Scenario: Hooks installed in every agent worktree

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

### Requirement: Agent skill teaches the discipline

The bundled `assets/agent-skills/coordination.md` SHALL
include a "Stay inside your worktree" subsection teaching
agents to use only relative paths from their worktree root
when running bash, and explicitly forbidding `cd` to
absolute paths outside the worktree. The prose SHALL
reference the branch-guard hook as the enforcement
mechanism.

#### Scenario: Skill prose names the discipline

- **WHEN** the coordination skill is read
- **THEN** the "Stay inside your worktree" subsection SHALL
  appear with explicit "use relative paths only" guidance
  and a reference to the pre-commit guard
