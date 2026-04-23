## ADDED Requirements

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
