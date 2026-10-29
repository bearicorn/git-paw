## ADDED Requirements

### Requirement: Configuration schema version 2 relocates the supervisor table

The current configuration schema version SHALL be 2. Version 2 SHALL NOT have a `[supervisor]` table; its keys SHALL live at the version-2 paths below, and the version-1 to version-2 migration step SHALL move every version-1 path to its version-2 path, and SHALL drop every path the table marks dropped, in memory, for any document that declares `version = 1` or no version. A dropped path SHALL NOT be moved to any version-2 table. The step SHALL be one entry of the configuration migration chain, SHALL be a pure function of the document, and SHALL NOT write, rename or touch the file. Every requirement in any capability that names a version-1 path with a version-2 path SHALL be read as naming its version-2 path, with unchanged meaning, type and default.

| Version-1 path | Version-2 path |
|---|---|
| `[supervisor] enabled = true` / `false` | top-level `default_mode = "coordinated"` / `"bare"` |
| a `[supervisor]` table without `enabled` | top-level `default_mode = "bare"` |
| `[supervisor] cli`, `approval`, `verify_on_commit_nudge`, `auto_revert` | `[orchestrator]` same key |
| `[supervisor.escalation]` | `[orchestrator.escalation]` |
| `[supervisor] test_command`, `lint_command`, `build_command`, `doc_build_command`, `doc_tool_command`, `spec_validate_command`, `fmt_check_command`, `security_audit_command` | `[orchestrator.gates]` same key |
| `[supervisor] agent_approval` | `[workers] approval` |
| `[supervisor] strict_branch_guard` | `[workers] strict_branch_guard` |
| `[supervisor.tell]` | `[workers.tell]` |
| `[supervisor.auto_approve]` (every key) | dropped — deprecated 0.17.0, no effect |
| `[supervisor.common_dev_allowlist]` (every key) | dropped — deprecated 0.17.0, no effect |
| `[supervisor.correction]`, `[supervisor.conflict]` | `[drive.correction]`, `[drive.conflict]` |
| `[supervisor] learnings` | `[drive.learnings] enabled` |
| `[supervisor.learnings_config]` `flush_interval_seconds`, `broker_publish` | `[drive.learnings]` same keys |
| `[supervisor] manual_approvals_log`, `branch_refresh`, `no_progress_window_seconds`, `context_bloat_threshold_k`, `budget_max_tokens`, `budget_max_cost` | `[drive]` same key |
| `[supervisor] blocked_on_supervisor_window_seconds` | `[drive] blocked_on_orchestrator_window_seconds` |

`default_mode` SHALL accept exactly `"coordinated"` and `"bare"`; any other value SHALL fail loading with a configuration error naming the key and both accepted values.

#### Scenario: Every version-1 path loads as its version-2 path

- **GIVEN** for each row of the table that has a version-2 path, one config written with the version-1 path and one with the version-2 path and the same value
- **WHEN** both are loaded
- **THEN** they SHALL produce equal configurations

#### Scenario: The deprecated auto-approval tables are dropped, not moved

- **GIVEN** a version-1 config with `[supervisor] test_command = "just check"`, `[supervisor.auto_approve] safe_commands = ["ls"]` and `[supervisor.common_dev_allowlist] stacks = ["rust"]`
- **WHEN** it is loaded
- **THEN** the configuration SHALL equal the one loaded from the same file without the two auto-approval tables
- **AND** it SHALL carry no `[workers.auto_approve]` or `[workers.common_dev_allowlist]` table

#### Scenario: A version-1 file is never rewritten

- **GIVEN** a `.git-paw/config.toml` containing `[supervisor] enabled = true` and `test_command = "just check"`
- **WHEN** any command loads the config
- **THEN** the file's bytes and modification time SHALL be unchanged
- **AND** the loaded configuration SHALL resolve coordinated mode by default and the test command `just check`

#### Scenario: A section without enabled means bare

- **GIVEN** a version-1 config whose `[supervisor]` table sets only `cli = "claude"`
- **WHEN** it is loaded
- **THEN** `default_mode` SHALL resolve to `"bare"`

#### Scenario: An invalid default_mode is refused

- **GIVEN** a config with `default_mode = "supervised"`
- **WHEN** it is loaded
- **THEN** loading SHALL fail with an error naming `default_mode`, `coordinated` and `bare`

### Requirement: Version-1 coordination keys warn once per key family

When a loaded file contains version-1 paths, git-paw SHALL print, through the shared deprecation helper, one warning per key family and file per process — the families being `default_mode`, `[orchestrator]`, `[orchestrator.gates]`, `[workers]` and `[drive]` — naming the file, each version-1 path found in that family, its version-2 path, and the deprecating release. The warnings SHALL go to stderr only and SHALL NOT change the exit code. The dropped `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]` tables belong to no family: they warn once per file through the same helper as *Deprecated auto-approval tables are parsed but have no effect* states, and SHALL NOT also produce a family warning.

#### Scenario: Two families, two warnings

- **GIVEN** a config with `[supervisor] cli = "claude"`, `approval = "auto"` and `test_command = "just check"`
- **WHEN** `git paw status` runs
- **THEN** stderr SHALL carry exactly one warning naming `[supervisor] cli` and `[supervisor] approval` with `[orchestrator]`
- **AND** exactly one warning naming `[supervisor] test_command` with `[orchestrator.gates]`

#### Scenario: Loading twice in one process warns once

- **GIVEN** the same config
- **WHEN** one command loads it twice
- **THEN** each family's warning SHALL appear once

### Requirement: A version-1 key and its version-2 counterpart conflict

A file that sets both a version-1 path and the version-2 path it migrates to — for a sub-table, both the old and the new table — SHALL fail loading with a configuration error naming the file and both paths. Version-1 and version-2 paths that do not map to each other MAY appear in the same file.

#### Scenario: Old and new test command together are refused

- **GIVEN** a config with `[supervisor] test_command = "a"` and `[orchestrator.gates] test_command = "b"`
- **WHEN** it is loaded
- **THEN** loading SHALL fail with an error naming the file, `[supervisor] test_command` and `[orchestrator.gates] test_command`

#### Scenario: Unrelated old and new keys coexist

- **GIVEN** a config with no `version`, `[supervisor] test_command = "a"` and `[workers] approval = "manual"`
- **WHEN** it is loaded
- **THEN** loading SHALL succeed with both values and one deprecation warning for `[orchestrator.gates]`

### Requirement: A version-2 file does not accept the supervisor table

A file that declares `version = 2` and contains a `[supervisor]` table SHALL fail loading with a configuration error naming the file and the table and pointing at the migration table in the configuration reference.

#### Scenario: Declared version 2 with an old table is refused

- **GIVEN** a config with `version = 2` and `[supervisor] cli = "claude"`
- **WHEN** it is loaded
- **THEN** loading SHALL fail with an error naming the file and `[supervisor]`

### Requirement: The coordination tables merge across files as one unit

When the global and repository configs are merged, the `[orchestrator]`, `[workers]` and `[drive]` tables (with their sub-tables) SHALL be treated as one unit: if the repository file sets any of them after its own migration, the global file's three tables SHALL be ignored; otherwise the global file's tables SHALL apply. `default_mode` SHALL merge like any other top-level key, the repository value winning. For any pair of version-1 files this SHALL produce the same effective configuration as the `[supervisor]` section precedence did before this change.

#### Scenario: A repository table hides the global coordination tables

- **GIVEN** a global config with `[supervisor] agent_approval = "full-auto"` and a repository config with `[supervisor] test_command = "just check"`
- **WHEN** the merged configuration is loaded
- **THEN** the worker approval SHALL resolve to the default `auto`, not `full-auto`
- **AND** the test command SHALL be `just check`

#### Scenario: Without repository tables the global ones apply

- **GIVEN** a global config with `[workers] approval = "manual"` and a repository config with none of the three tables
- **WHEN** the merged configuration is loaded
- **THEN** the worker approval SHALL resolve to `manual`

### Requirement: git-paw writes and stamps only the version-2 layout

Every configuration git-paw serializes SHALL use the version-2 layout and declare `version = 2`; when `git paw add-cli` or `git paw remove-cli` rewrites a global file that used version-1 paths, it SHALL print one line stating that the file was rewritten with the version-2 names, and naming each deprecated auto-approval table the rewrite dropped. `git paw init` SHALL backfill `version = 2` only into a file that contains no `[supervisor]` table; for a file that does, it SHALL leave `version` absent and print one line pointing at `git paw config check`. The generated default config SHALL show the version-2 tables as commented examples and SHALL contain no `[supervisor]` text.

#### Scenario: add-cli rewrites a global supervisor table

- **GIVEN** a global config with one custom CLI and `[supervisor] agent_approval = "manual"`
- **WHEN** `git paw add-cli my-agent /usr/local/bin/my-agent` runs
- **THEN** the rewritten file SHALL declare `version = 2`, contain `[workers]` with `approval = "manual"` and no `[supervisor]`
- **AND** stdout SHALL carry one line saying the file now uses the version-2 names

#### Scenario: add-cli drops the deprecated auto-approval tables and names them

- **GIVEN** a global config with one custom CLI, `[supervisor.auto_approve] enabled = true` and `[supervisor.common_dev_allowlist] stacks = ["rust"]`
- **WHEN** `git paw add-cli my-agent /usr/local/bin/my-agent` runs
- **THEN** the rewritten file SHALL contain no `auto_approve` and no `common_dev_allowlist` table under any name
- **AND** the one rewrite line on stdout SHALL name `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]` as dropped

#### Scenario: init does not stamp a version-1 file

- **GIVEN** an existing `.git-paw/config.toml` with `[supervisor] enabled = true` and no `version`
- **WHEN** `git paw init` runs
- **THEN** the file SHALL still have no `version` key and its `[supervisor]` table SHALL be byte-identical
- **AND** stdout SHALL carry one line pointing at `git paw config check`

#### Scenario: init stamps a layout-neutral file

- **GIVEN** an existing config with only `[broker] enabled = true`
- **WHEN** `git paw init` runs
- **THEN** the file SHALL gain `version = 2`

#### Scenario: The template carries no supervisor table

- **WHEN** the default config string is generated
- **THEN** it SHALL contain the active line `version = 2` and commented `[orchestrator]`, `[orchestrator.gates]`, `[workers]` and `[drive]` examples
- **AND** it SHALL NOT contain the substring `[supervisor`

### Requirement: Bundled helpers read the version-2 keys with a version-1 fallback

The bundled helper scripts SHALL read the test command from `[orchestrator.gates] test_command` and the stuck-detection windows from `[drive] no_progress_window_seconds`, `context_bloat_threshold_k` and `blocked_on_orchestrator_window_seconds`, and SHALL fall back to the corresponding version-1 `[supervisor]` key when the version-2 key is absent, both through the TOML parser and through the regex fallback. The configuration reference's list of script-read keys SHALL name the version-2 keys and SHALL list the version-1 fallbacks as migration support.

#### Scenario: The new key is read first

- **GIVEN** a config with `[orchestrator.gates] test_command = "make test"`
- **WHEN** `sweep.sh` resolves its test command, with a TOML parser available and through its regex fallback
- **THEN** it SHALL resolve `make test` both ways

#### Scenario: A version-1 file still configures the helper

- **GIVEN** a config with only `[supervisor] test_command = "just check"` and `blocked_on_supervisor_window_seconds = 600`
- **WHEN** `sweep.sh` resolves its test command and blocked window, both ways
- **THEN** it SHALL resolve `just check` and `600`

### Requirement: The configuration reference documents the version-2 tables and the migration

The configuration reference SHALL document `default_mode`, `[orchestrator]`, `[orchestrator.gates]`, `[orchestrator.escalation]`, `[workers]` and its sub-tables, and `[drive]` and its sub-tables, SHALL state the one-unit merge rule, and SHALL carry a migration table listing every version-1 path with its version-2 path, or, for `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]`, "dropped — deprecated 0.17.0, no effect". A test SHALL fail when the migration table and the migration step's mapping differ.

#### Scenario: The migration table matches the step

- **WHEN** the configuration reference's migration table is compared with the migration step's mapping
- **THEN** every mapped path SHALL appear in the table with the same version-2 path, every dropped path SHALL appear as dropped, and the table SHALL list nothing else
