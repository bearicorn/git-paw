# worktree-branch-guard Specification

## Purpose
TBD - created by archiving change session-bugfixes-v0-6-x. Update Purpose after archive.
## Requirements
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
[[agent-learning-variant]]) whenever a branch mismatch is
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

