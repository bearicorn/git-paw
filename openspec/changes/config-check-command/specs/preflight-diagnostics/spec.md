## MODIFIED Requirements

### Requirement: Config check

The command SHALL report whether `.git-paw/config.toml` is present and parses, report the
resolved `worktree_placement`, and report the findings of the shared configuration validator (see
"`git paw config check` validates every config file") for both the global and the repository
config file, each attributed to its file: validator errors SHALL be ✗ and validator warnings — which
include unknown and deprecated fields — SHALL be ⚠. An unparseable config SHALL be ✗. When a file has
more than one finding, the remedy SHALL point at `git paw config check` for the full list.

#### Scenario: Config parses

- **GIVEN** a valid `.git-paw/config.toml`
- **WHEN** `git paw doctor` is run
- **THEN** the Config group SHALL report ✓ and state the resolved `worktree_placement`

#### Scenario: Config does not parse

- **GIVEN** a `.git-paw/config.toml` with invalid TOML
- **WHEN** `git paw doctor` is run
- **THEN** the Config group SHALL report ✗ with a remedy

#### Scenario: Deprecated field is flagged

- **GIVEN** a repo config containing `[supervisor.auto_approve] enabled = true`
- **WHEN** `git paw doctor` is run
- **THEN** the Config group SHALL report ⚠ naming `supervisor.auto_approve` as deprecated

#### Scenario: Broken global config is attributed to the global file

- **GIVEN** a valid repo config and a global config with invalid TOML
- **WHEN** `git paw doctor` is run
- **THEN** the Config group SHALL report ✗ naming the global config path, not the repo path

## ADDED Requirements

### Requirement: `git paw config check` validates every config file

The system SHALL provide `git paw config check`, which reads the global config file and the repository's `.git-paw/config.toml` (each only if present) and validates each file separately, without merging them and without modifying any file. For every finding it SHALL report the file path, the key path (when the finding concerns a key), a severity of error or warning, a message, and a remedy. It SHALL report as errors: TOML syntax errors and type mismatches (with line and column when available), and every value git-paw rejects when loading (such as an invalid approval level or a session template lacking its required placeholder). It SHALL report as warnings: keys git-paw does not recognise; deprecated keys, including the `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]` tables (deprecated in 0.17.0, no effect; see *Deprecated auto-approval tables are parsed but have no effect*), each reported once as a whole table naming the deprecating release and stating that it has no effect and can be removed, with no further finding for the table's fields or values; a `default_cli` or `default_spec_cli` that is neither a built-in CLI name nor defined under `[clis]`; and `[worktree.ports]` settings that produce a stride warning. Explicitly empty tables and lists SHALL NOT be reported as unknown. A missing file SHALL be reported as informational, not as a finding. The command SHALL also report, for each file, the schema-version findings produced by the configuration migration API (`config::migrate`, from config schema versioning) — such as a declared version newer than the binary supports, as an error — and SHALL NOT re-implement version detection or migration.

#### Scenario: Unknown key in the global file is reported with its path

- **GIVEN** a global config containing `[broker] prot = 9119`
- **WHEN** the user runs `git paw config check`
- **THEN** the output SHALL report a warning for the global config path with key `broker.prot`

#### Scenario: Deprecated auto-approval tables are reported once each

- **GIVEN** a repo config with `[supervisor.auto_approve] enabled = true` and `[supervisor.common_dev_allowlist] stacks = ["rust", "elixir"]`
- **WHEN** the user runs `git paw config check`
- **THEN** the output SHALL report exactly one warning with key `supervisor.auto_approve` and exactly one with key `supervisor.common_dev_allowlist`, each naming release 0.17.0 and stating that the table has no effect
- **AND** no finding SHALL name `elixir` or a field of either table

#### Scenario: Unknown default CLI is reported

- **GIVEN** a repo config with `default_cli = "clade"` and no `[clis.clade]`
- **WHEN** the user runs `git paw config check`
- **THEN** the output SHALL report a warning for key `default_cli`

#### Scenario: Explicit empty values are not unknown

- **GIVEN** a repo config with `[worktree.env] copy = []`
- **WHEN** the user runs `git paw config check`
- **THEN** no finding SHALL be reported for `worktree.env.copy`

#### Scenario: Syntax error is an error with a position

- **GIVEN** a repo config with an unterminated string on line 4
- **WHEN** the user runs `git paw config check`
- **THEN** the output SHALL report an error for the repo config path that names line 4

#### Scenario: Files are never modified

- **GIVEN** any global and repo config
- **WHEN** the user runs `git paw config check`
- **THEN** neither file SHALL be created, modified or deleted

### Requirement: `git paw config check` exit status and output modes

`git paw config check` SHALL exit 0 when no file has an error finding and 1 when any file has an error finding. With `--strict` it SHALL also exit 1 when any warning finding exists. With `--json` it SHALL follow the `cli-machine-output` contract and print one document containing `schema_version`, `files` (one entry per config file considered, with `path`, `kind` (`"global"` or `"repo"`), `present`, and `config_version` — the file's declared or implied schema version as reported by the configuration migration API, or `null` when the file is absent or unparseable), and `findings` (one entry per finding with `file`, `key_path` (or `null`), `severity` (`"error"` or `"warning"`), `message`, and `remedy`), with the same exit status as the human mode. Error findings are results, not a command failure: the document SHALL be printed before exiting 1. Only a failure of the command itself (for example an unreadable file system) SHALL leave stdout empty. The subcommand SHALL carry `about`, `long_about` with examples, and a `help` string on every flag, and its help SHALL state that it validates configuration files while `git paw doctor` diagnoses the environment.

#### Scenario: Warnings alone exit 0

- **GIVEN** a repo config whose only finding is an unknown key
- **WHEN** the user runs `git paw config check`
- **THEN** the process SHALL exit 0

#### Scenario: Strict mode fails on warnings

- **GIVEN** the same config
- **WHEN** the user runs `git paw config check --strict`
- **THEN** the process SHALL exit 1

#### Scenario: An error exits 1

- **GIVEN** a repo config with invalid TOML
- **WHEN** the user runs `git paw config check`
- **THEN** the process SHALL exit 1

#### Scenario: JSON output carries every finding

- **GIVEN** a repo config with one unknown key and one deprecated key
- **WHEN** the user runs `git paw config check --json`
- **THEN** stdout SHALL be one JSON object with `schema_version` 1, a `files` entry for the repo config, and `findings` listing both findings with `file`, `key_path`, `severity`, `message` and `remedy`

#### Scenario: JSON output is printed when an error is found

- **GIVEN** a repo config with invalid TOML
- **WHEN** the user runs `git paw config check --json`
- **THEN** stdout SHALL be the document with an `error` finding for the repo config and its `config_version` null
- **AND** the process SHALL exit 1
