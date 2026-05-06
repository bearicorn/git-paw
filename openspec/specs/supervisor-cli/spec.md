# supervisor-cli Specification

## Purpose
TBD - created by archiving change supervisor-mode. Update Purpose after archive.
## Requirements
### Requirement: Supervisor mode resolution chain

The system SHALL determine whether to enter supervisor mode using the following resolution chain, evaluated in order:

1. If `--supervisor` flag is present → enable supervisor mode (no prompt)
2. If `[supervisor] enabled = true` in config → enable supervisor mode (no prompt)
3. If `[supervisor] enabled = false` in config → disable supervisor mode (no prompt)
4. If `[supervisor]` section is absent (`None`) → prompt "Start in supervisor mode? (y/n)"
5. If `--dry-run` is present and step 4 would apply → assume no supervisor (skip prompt)

When supervisor mode is enabled (steps 1 or 2), the system SHALL call `cmd_supervisor()`. When disabled (step 3 or 5), the system SHALL proceed with normal `cmd_start()`.

#### Scenario: --supervisor flag enables regardless of config

- **GIVEN** a config with `[supervisor] enabled = false`
- **WHEN** `git paw start --supervisor` is run
- **THEN** supervisor mode SHALL be enabled
- **AND** `cmd_supervisor()` SHALL be called

#### Scenario: Config enabled = true enables without prompt

- **GIVEN** a config with `[supervisor] enabled = true`
- **WHEN** `git paw start` is run with no flags
- **THEN** supervisor mode SHALL be enabled without any interactive prompt

#### Scenario: Config enabled = false disables without prompt

- **GIVEN** a config with `[supervisor] enabled = false`
- **WHEN** `git paw start` is run with no flags
- **THEN** supervisor mode SHALL NOT be entered
- **AND** no interactive prompt SHALL be shown

#### Scenario: No supervisor section prompts the user

- **GIVEN** a config with no `[supervisor]` section
- **WHEN** `git paw start` is run with no flags
- **THEN** the system SHALL prompt "Start in supervisor mode?"

#### Scenario: dry-run skips supervisor prompt

- **GIVEN** a config with no `[supervisor]` section
- **WHEN** `git paw start --dry-run` is run
- **THEN** no interactive prompt SHALL be shown
- **AND** supervisor mode SHALL NOT be entered

### Requirement: Merge ordering from dependency signals

The supervisor SHALL determine the safe merge order for worktree branches using the dependency graph built from `agent.blocked` messages in the broker's message log.

The system SHALL:
1. Build a directed graph where edge `A → B` means "agent A was blocked on agent B"
2. Compute a topological sort of this graph (agents with no dependents first, agents others depend on first)
3. Merge branches in the computed order
4. Run the configured test command after each merge and verify it passes before proceeding

When the dependency graph contains a cycle, the system SHALL log a warning and fall back to an arbitrary merge order rather than failing.

#### Scenario: No dependencies yields arbitrary order

- **GIVEN** three agents that never published `agent.blocked`
- **WHEN** the supervisor computes merge order
- **THEN** all three branches are in the merge order list
- **AND** no error occurs

#### Scenario: Dependency chain determines merge order

- **GIVEN** agent A published `agent.blocked` with `from = "feat-b"` (A depends on B)
- **WHEN** the supervisor computes merge order
- **THEN** `feat-b` appears before `feat-a` in the merge order

#### Scenario: Cycle in dependencies logs warning and falls back

- **GIVEN** agent A is blocked on B and agent B is blocked on A
- **WHEN** the supervisor computes merge order
- **THEN** a warning SHALL be logged identifying the cycle
- **AND** both branches SHALL still be included in the merge order

#### Scenario: Test command runs after each merge

- **GIVEN** a supervisor config with `test_command = "just check"`
- **WHEN** the supervisor merges the first branch
- **THEN** `just check` SHALL be run before merging the next branch

### Requirement: Validate specs are committed before launching

When `git paw start --from-specs` is used, the system SHALL verify that spec files discovered in the working directory are also present in the git index. This applies to both OpenSpec format (`openspec/changes/`) and Markdown format (the configured `[specs] dir`).

If any spec change directory or file exists in the working tree but is untracked or has uncommitted changes, the system SHALL warn: "N spec(s) have uncommitted changes. Worktree agents will not see uncommitted specs. Commit first or use --force to proceed."

The system SHALL NOT launch unless the user confirms or `--force` is passed.

#### Scenario: Uncommitted OpenSpec changes trigger warning

- **GIVEN** `openspec/changes/my-change/` exists but is not tracked by git
- **WHEN** `git paw start --from-specs` is run
- **THEN** the system SHALL warn about uncommitted specs
- **AND** SHALL NOT launch without user confirmation

#### Scenario: Uncommitted Markdown specs trigger warning

- **GIVEN** a Markdown spec file in the configured `[specs] dir` has uncommitted modifications
- **WHEN** `git paw start --from-specs` is run
- **THEN** the system SHALL warn about uncommitted specs

#### Scenario: All specs committed launches normally

- **GIVEN** all spec files are committed and clean
- **WHEN** `git paw start --from-specs` is run
- **THEN** no warning is shown and the session launches normally

#### Scenario: Force flag bypasses warning

- **GIVEN** uncommitted spec changes exist
- **WHEN** `git paw start --from-specs --force` is run
- **THEN** the session launches without warning
- **AND** if `just check` fails, the supervisor SHALL stop and report the failure

### Requirement: Purge warns about unmerged commits

Before destroying worktrees, `git paw purge` SHALL check each worktree branch for commits not yet merged to the default branch. The system SHALL:

1. For each worktree branch, run `git log <branch> --not <default-branch> --oneline`
2. If any branch has unmerged commits, display a warning listing each branch and its commit count
3. Require either `--force` flag or interactive confirmation ("Y" response) to proceed
4. If the user declines, exit without destroying any worktrees

The default branch SHALL be resolved from `git symbolic-ref refs/remotes/origin/HEAD`, falling back to `main` if unavailable.

#### Scenario: Purge with no unmerged commits proceeds without warning

- **GIVEN** all worktree branches have no commits beyond the default branch
- **WHEN** `git paw purge` is run
- **THEN** no unmerged commit warning SHALL be shown
- **AND** purge proceeds normally

#### Scenario: Purge with unmerged commits warns before destroying

- **GIVEN** one worktree branch has 3 commits not merged to main
- **WHEN** `git paw purge` is run without `--force`
- **THEN** a warning SHALL be displayed identifying the branch and the number of unmerged commits
- **AND** the system SHALL prompt for confirmation before proceeding

#### Scenario: Purge --force skips confirmation but still warns

- **GIVEN** one worktree branch has unmerged commits
- **WHEN** `git paw purge --force` is run
- **THEN** the warning SHALL still be displayed
- **AND** purge SHALL proceed without waiting for interactive confirmation

#### Scenario: Purge cancelled by user preserves worktrees

- **GIVEN** one worktree branch has unmerged commits
- **WHEN** `git paw purge` is run and the user answers "N" to the confirmation
- **THEN** no worktrees SHALL be removed
- **AND** the system SHALL exit with a non-error message indicating purge was cancelled

