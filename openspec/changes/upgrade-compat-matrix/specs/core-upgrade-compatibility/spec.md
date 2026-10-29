## Purpose

Guarantees that upgrading git-paw never strands state an earlier release wrote. Every persisted
file — repository and global config, the global session file, the per-repository discovery file,
`broker.log` and the manual-approvals log — written by any release from v0.16.0 onward loads under
the current build, and a session such a release recorded can be recovered, stopped and purged. The
guarantee is proven by a committed fixture matrix captured from each released binary, which every
release extends before it ships.

## ADDED Requirements

### Requirement: Files written by a supported release load under the current build

The current build SHALL load, without error and without losing a recorded value, every persisted file written by any git-paw release from v0.16.0 onward: the repository `.git-paw/config.toml`, the global `config.toml`, the global session file, the per-repository discovery file under `.git-paw/sessions/`, `broker.log`, and the manual-approvals JSONL. A field added after the writing release SHALL take its documented default. A file that carries no format version SHALL be read as the baseline version of its format. Loading SHALL NOT rewrite a user's configuration file. A key that the current build documents as deprecated with no effect — the `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]` tables since v0.17.0 — SHALL load without error and SHALL NOT be counted as a lost value, because no release after v0.16.0 uses it; the current build's deprecation warning for it is expected output, not a failure.

#### Scenario: A v0.16.0 repository config loads

- **GIVEN** the `.git-paw/config.toml` captured from v0.16.0
- **WHEN** the current build runs `git paw status` in that repository
- **THEN** the config SHALL load without error
- **AND** every value the fixture sets SHALL be the value the current build uses
- **AND** the file's bytes SHALL be unchanged afterwards

Test: `tests/upgrade_compat_matrix.rs::every_release_row_loads` (row `0.16.0/config.toml`).

#### Scenario: A deprecated no-effect table in an old config is not a lost value

- **GIVEN** a fixture config from a release before v0.17.0 that declares `[supervisor.auto_approve]` or `[supervisor.common_dev_allowlist]`
- **WHEN** the current build runs `git paw status` in that repository
- **THEN** the config SHALL load without error
- **AND** the row SHALL NOT fail because those tables' values have no effect
- **AND** every other value the fixture sets SHALL be the value the current build uses

Test: `tests/upgrade_compat_matrix.rs::every_release_row_loads` (row `0.16.0/config.toml`, deprecated-table exemption).

#### Scenario: A v0.16.0 session file is found and reported

- **GIVEN** the global session file and discovery file captured from v0.16.0, installed in a test-owned data directory
- **WHEN** the current build runs `git paw status --json` in that repository
- **THEN** the session SHALL be found
- **AND** its status, branches and worktree paths SHALL be reported as recorded

Test: `tests/upgrade_compat_matrix.rs::every_release_row_loads` (row `0.16.0/session`).

#### Scenario: v0.16.0 manual approvals are aggregated

- **GIVEN** the manual-approvals JSONL captured from v0.16.0
- **WHEN** the current build runs `git paw approvals` for that session
- **THEN** every recorded pattern SHALL appear with its count

Test: `tests/upgrade_compat_matrix.rs::every_release_row_loads` (row `0.16.0/approvals`).

### Requirement: A session recorded by an older release resumes and tears down under the current build

A session recorded by any release from v0.16.0 onward, whose tmux session no longer exists, SHALL be recoverable by `git paw start` under the current build: the session SHALL be rebuilt from its recorded worktree entries, each with its recorded CLI. Recovery SHALL keep the dashboard the session recorded: a session recorded with a dashboard SHALL be rebuilt with one, and a session recorded with a broker but no dashboard SHALL be rebuilt without one, whatever the current configuration selects. `git paw stop` and `git paw purge` SHALL then remove everything the older release created for it — its worktrees, the branches it recorded as created, the per-repository discovery file, the global session file and `broker.log`.

#### Scenario: A stopped v0.16.0 session is recovered

- **GIVEN** the v0.16.0 session fixture with status `active` and no tmux session running
- **WHEN** the current build runs `git paw start` in that repository
- **THEN** a session with one agent surface per recorded worktree SHALL be created
- **AND** each agent surface SHALL run the CLI recorded for its worktree

Test: `tests/upgrade_compat_matrix.rs::every_release_row_resumes` (socket- and HOME-isolated).

#### Scenario: A session recorded with a broker but no dashboard recovers without one

- **GIVEN** the v0.21.0 session fixture captured with the broker enabled and the dashboard not enabled, with status `active` and no tmux session running
- **AND** a repository configuration that enables the dashboard
- **WHEN** the current build runs `git paw start` in that repository
- **THEN** the recovered session SHALL have no dashboard pane
- **AND** its coding agents SHALL start at pane 0

Test: `tests/upgrade_compat_matrix.rs::every_release_row_resumes` (row `0.21.0/broker-no-dashboard`).

#### Scenario: A recovered v0.16.0 session is purged completely

- **GIVEN** a v0.16.0 session recovered by the current build
- **WHEN** `git paw purge --force` runs
- **THEN** the recorded worktrees, the branches the session created, the discovery file, the global session file and `broker.log` SHALL be gone

Test: `tests/upgrade_compat_matrix.rs::every_release_row_resumes`.

### Requirement: The broker appends to a log written by an older release

A broker started by the current build for a session whose `broker.log` was written by a release from v0.16.0 onward SHALL append its entries after the existing content without truncating or rewriting it. Any reader of `broker.log` the current build ships SHALL accept the older lines.

#### Scenario: A v0.16.0 log keeps its lines after a new broker writes

- **GIVEN** the `broker.log` captured from v0.16.0
- **WHEN** a broker started by the current build publishes and flushes a message
- **THEN** the file SHALL begin with the v0.16.0 lines unchanged
- **AND** the new entry SHALL follow them

Test: `tests/upgrade_compat_matrix.rs::broker_appends_to_old_logs`.

### Requirement: Every release adds its fixture row before it ships

The test suite SHALL hold, under `tests/fixtures/compat/<version>/`, the files each release from v0.16.0 onward wrote in a scripted session, captured from that release's binary with machine-specific paths, usernames and timestamps replaced by placeholders, together with a record of how the row was captured. A standing test SHALL fail when a release at or after v0.16.0 listed in `CHANGELOG.md` has no fixture row, and when a fixture file contains an unreplaced absolute home path.

#### Scenario: A release without a row fails the suite

- **GIVEN** a `CHANGELOG.md` section for a release at or after v0.16.0
- **AND** no `tests/fixtures/compat/<version>/` directory for it
- **WHEN** the suite runs
- **THEN** the completeness test SHALL fail and SHALL name the missing release

Test: `tests/upgrade_compat_matrix.rs::every_released_version_has_a_row`.

#### Scenario: A fixture leaking a real home path fails the suite

- **GIVEN** a fixture file containing an absolute path under a real user home directory rather than a placeholder
- **WHEN** the suite runs
- **THEN** the hygiene check SHALL fail and SHALL name the file

Test: `tests/upgrade_compat_matrix.rs::fixtures_carry_no_machine_paths`.
