# auto-approve-file-edits Specification

## Purpose
TBD - created by archiving change auto-approve-scope-v0-6-x. Update Purpose after archive.
## Requirements
### Requirement: Worktree-confined file operations classify as safe

The auto-approve classifier SHALL recognise Claude's
filesystem-prompt patterns (write / edit / create / delete)
and SHALL classify them as safe-by-pattern when the resolved
target path is inside the agent's worktree root. The
classifier SHALL canonicalize paths before the
`starts_with(worktree_root)` check to prevent symlink-escape.

#### Scenario: In-worktree file create is auto-approved

- **GIVEN** an agent on `feat/cold-start-ci-parity` whose
  worktree root is `/path/to/git-paw-feat-cold-start-ci-parity/`
- **WHEN** Claude prompts "Do you want to allow this write
  to Containerfile?" (resolving to the worktree root)
- **THEN** the classifier SHALL return safe-by-pattern, and
  the auto-approve sweep SHALL dispatch the approval
  keystrokes

#### Scenario: Out-of-worktree file create requires manual approval

- **GIVEN** the same agent
- **WHEN** Claude prompts a write to `/etc/hosts` (or any
  path resolving outside the worktree)
- **THEN** the classifier SHALL NOT return safe-by-pattern,
  and the existing manual-prompt flow SHALL run

#### Scenario: Symlink-escape attempt does not bypass the boundary

- **GIVEN** a malicious prompt whose path contains `..`
  resolving outside the worktree
- **WHEN** the classifier canonicalizes the path
- **THEN** the resolved path SHALL be checked against the
  worktree root via `starts_with`, and the symlink-escape
  attempt SHALL fail the safety check

### Requirement: approve_worktree_writes config field

The system SHALL accept
`[supervisor.auto_approve].approve_worktree_writes` as a
boolean config field defaulting to `true`. When `false`, the
classifier SHALL NOT recognise file-operation prompts as
safe-by-pattern even when the target is inside the worktree;
manual prompts SHALL run as before this change.

#### Scenario: Default true auto-approves

- **GIVEN** no `[supervisor.auto_approve]` section in
  config (or `approve_worktree_writes` unset)
- **WHEN** a worktree-confined file prompt fires
- **THEN** the classifier SHALL auto-approve

#### Scenario: Explicit false reverts to manual

- **GIVEN** `[supervisor.auto_approve].approve_worktree_writes
  = false`
- **WHEN** a worktree-confined file prompt fires
- **THEN** the classifier SHALL NOT auto-approve; the user
  SHALL see the manual prompt

### Requirement: Backwards compatibility with shell auto-approve

The file-operation category SHALL be additive — existing
shell-command auto-approval (cargo / npm / pytest / pip /
mvn / make / gradle / docker / git / curl) SHALL continue to
work exactly as in v0.5.0.

#### Scenario: Shell auto-approve still fires

- **GIVEN** a Claude prompt for `cargo build`
- **WHEN** the classifier runs
- **THEN** the shell auto-approve SHALL fire (unchanged
  from v0.5.0); the new file-operation category SHALL NOT
  interfere

