## ADDED Requirements

### Requirement: Configured CLIs are checked on this machine

`git paw doctor` SHALL report, in the CLIs group, one row per CLI id the configuration names (`default_cli`, `default_spec_cli`, the orchestrator CLI, each preset's and worker profile's CLI, and each `[clis.*]` entry): ✓ with the resolved path when it resolves on this machine, else ⚠ naming the id, the key and file that set it, with a remedy to install it, set `[clis.<id>] command`, or pass `--cli`. Doctor SHALL NOT print a detection warning outside the report.

#### Scenario: A committed default CLI is absent on this machine

- **GIVEN** a repo config with `default_cli = "claude-oss"` and `[clis.claude-oss] command = "claude-oss"`, and no `claude-oss` on PATH
- **WHEN** `git paw doctor` is run
- **THEN** the CLIs group SHALL report ⚠ for `claude-oss` naming `default_cli` and the repo config path, with a remedy
- **AND** stderr SHALL NOT carry a `custom CLI 'claude-oss' not found` line

#### Scenario: A configured CLI that resolves is reported with its path

- **GIVEN** a repo config with `default_cli = "claude"` and `claude` on PATH
- **WHEN** `git paw doctor` is run
- **THEN** the CLIs group SHALL report ✓ for `claude` with the resolved path

### Requirement: Broker check reports how the port was chosen

When the broker is enabled, the Broker check SHALL state the port it probed and its source: `pinned` when a config file sets `[broker] port`, `session` when a live session recorded the port it bound, else `derived` (the per-repository default). The probe and its ✓/⚠ outcomes SHALL otherwise be unchanged.

#### Scenario: Derived port is named as derived

- **GIVEN** `[broker] enabled = true`, no `[broker] port` in either config file, and no session
- **WHEN** `git paw doctor` is run
- **THEN** the Broker check SHALL name the per-repository default port and say it is derived

#### Scenario: Pinned port is named as pinned

- **GIVEN** `[broker] enabled = true` and `port = 9200` in the repo config
- **WHEN** `git paw doctor` is run
- **THEN** the Broker check SHALL probe port 9200 and say it is pinned
