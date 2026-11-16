## MODIFIED Requirements

### Requirement: Validate specs are committed before launching

When `git paw start` launches from specs, on the bare or the coordinated path, the system SHALL check, before creating any worktree, session file or tmux session, whether any selected spec is untracked or has uncommitted changes. If any has and `--force` was not given, the system SHALL NOT launch: it SHALL exit with the not-attempted code (2) and an error naming each such spec, saying worktree agents would not see them, and naming `--force`.

#### Scenario: Uncommitted OpenSpec changes trigger warning

- **GIVEN** `openspec/changes/my-change/` exists but is not tracked by git
- **WHEN** `git paw start --from-specs` is run without `--force`
- **THEN** the command SHALL exit with code 2 and an error naming `my-change` and `--force`
- **AND** no worktree, session file or tmux session SHALL have been created

#### Scenario: Uncommitted Markdown specs trigger warning

- **GIVEN** a Markdown spec file in the configured `[specs] dir` has uncommitted modifications
- **WHEN** `git paw start --from-specs` is run without `--force`
- **THEN** the command SHALL exit with code 2 and an error naming that spec

#### Scenario: All specs committed launches normally

- **GIVEN** all spec files are committed and clean
- **WHEN** `git paw start --from-specs` is run
- **THEN** no uncommitted-spec message is shown and the session launches normally

#### Scenario: Force flag bypasses warning

- **GIVEN** uncommitted spec changes exist
- **WHEN** `git paw start --from-specs --force` is run
- **THEN** the session SHALL launch
- **AND** stderr SHALL carry one line saying the uncommitted specs were ignored because of `--force`

#### Scenario: The coordinated path refuses the same way

- **GIVEN** an untracked `openspec/changes/my-change/`
- **WHEN** `git paw start --coordinated --from-specs` is run without `--force`
- **THEN** the command SHALL exit with code 2 naming `my-change` and `--force`
- **AND** no broker, worktree or tmux session SHALL have been started

## ADDED Requirements

### Requirement: The uncommitted-spec check reads each spec where its format keeps it

The uncommitted-spec check SHALL inspect each selected spec at the location its spec format's backend reports for it (an OpenSpec change directory, a Spec Kit feature directory, a Markdown file or directory), not at a fixed `specs/<id>` path. A spec its backend cannot locate SHALL be skipped.

#### Scenario: An OpenSpec change outside specs/ is checked

- **GIVEN** an OpenSpec repository whose change `add-x` has an uncommitted edit to `openspec/changes/add-x/tasks.md`, and no `specs/add-x` path
- **WHEN** the uncommitted-spec check runs for `add-x`
- **THEN** it SHALL report `add-x`

#### Scenario: A Spec Kit feature is checked in its feature directory

- **GIVEN** a Spec Kit feature directory with an untracked `tasks.md`
- **WHEN** the uncommitted-spec check runs for that feature
- **THEN** it SHALL report the feature
