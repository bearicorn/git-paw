## MODIFIED Requirements

### Requirement: `git paw doctor` runs read-only preflight checks

The system SHALL provide a `git paw doctor` command that inspects the environment,
configuration, repository, and session state and prints a grouped diagnostic report. Each check
SHALL be reported with one of four statuses — ✓ (pass), ⚠ (warning), ✗ (failure), or ○ (skipped:
the check was not evaluated) — and every non-✓ check SHALL print an actionable remedy. Without
`--fix`, the command SHALL NOT mutate any file, config, session, worktree registration, or other
persistent state. With `--fix` it SHALL mutate only what the "`git paw doctor --fix` repairs only
git-paw's own session records" requirement allows.

#### Scenario: Doctor prints a grouped report

- **GIVEN** a git repository
- **WHEN** `git paw doctor` is run
- **THEN** it SHALL print checks grouped under Environment, CLIs, Config, Spec system, Bundled scripts, Broker, Supervisor, Session, and Hygiene
- **AND** each check SHALL carry a ✓, ⚠, ✗, or ○ status
- **AND** every ⚠, ✗, or ○ check SHALL include a remedy line

#### Scenario: Doctor does not mutate state

- **GIVEN** a repository with a `.git-paw/` directory and a drifted session record
- **WHEN** `git paw doctor` is run without `--fix`
- **THEN** no file under `.git-paw/` (config, scripts, session state), no session file, and no git worktree registration SHALL be created, modified, or deleted

#### Scenario: A skipped check is shown with its own glyph and count

- **GIVEN** a report with one ○ check
- **WHEN** `git paw doctor` is run
- **THEN** that check SHALL be printed with the glyph `○` and a detail starting with "not evaluated"
- **AND** the summary line SHALL count it under `○`, not under `⚠`

### Requirement: Exit code reflects the worst check

The command SHALL exit with a non-zero status when any check is ✗, and with zero when all
checks are ✓, ⚠, or ○. A ⚠ alone SHALL NOT cause a non-zero exit. A ○ (skipped) check SHALL NOT
count toward the worst check and SHALL NOT change the exit code.

#### Scenario: All checks pass

- **GIVEN** an environment where every check resolves to ✓
- **WHEN** `git paw doctor` is run
- **THEN** the process SHALL exit 0

#### Scenario: A hard failure is present

- **GIVEN** an environment with at least one ✗ check
- **WHEN** `git paw doctor` is run
- **THEN** the process SHALL exit non-zero

#### Scenario: Only warnings are present

- **GIVEN** an environment with ⚠ checks but no ✗ checks
- **WHEN** `git paw doctor` is run
- **THEN** the process SHALL exit 0

#### Scenario: Skipped checks do not change the exit code

- **GIVEN** a report whose evaluated checks are all ✓ or ⚠, plus one ○ check
- **WHEN** the exit code is computed
- **THEN** it SHALL be 0, the code the evaluated checks alone produce

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

The system SHALL provide `git paw config check`, which reads the global config file and the repository's `.git-paw/config.toml` (each only if present) and validates each file separately, without merging them and without modifying any file. For every finding it SHALL report the file path, the key path (when the finding concerns a key), a severity of error or warning, a message, and a remedy. A missing file SHALL be reported as informational, not as a finding.

#### Scenario: Files are never modified

- **GIVEN** any global and repo config
- **WHEN** the user runs `git paw config check`
- **THEN** neither file SHALL be created, modified or deleted

### Requirement: `git paw config check` error findings

`git paw config check` SHALL report as errors: TOML syntax errors and type mismatches, with line and column when available; every value git-paw rejects when loading; and the schema-version findings of the configuration migration API (`config::migrate`, from config schema versioning). It SHALL NOT re-implement version detection or migration.

#### Scenario: Syntax error is an error with a position

- **GIVEN** a repo config with an unterminated string on line 4
- **WHEN** the user runs `git paw config check`
- **THEN** the output SHALL report an error for the repo config path that names line 4

#### Scenario: A value rejected at load is an error

- **GIVEN** a repo config with `[clis.x] approval_args = { always = "--yolo" }`
- **WHEN** the user runs `git paw config check`
- **THEN** the output SHALL report an error for key `clis.x.approval_args`

#### Scenario: A newer config version is an error

- **GIVEN** a repo config declaring a config version newer than the binary supports
- **WHEN** the user runs `git paw config check`
- **THEN** the output SHALL report the migration API's version finding for the repo config as an error

### Requirement: `git paw config check` warning findings

`git paw config check` SHALL report as warnings: keys git-paw does not recognise, except explicitly empty tables and lists; a `default_cli` or `default_spec_cli` that is neither a built-in CLI name nor defined under `[clis]`; `[worktree.ports]` settings that produce a stride warning; and deprecated keys (see *Deprecated config tables are reported once each*).

#### Scenario: Unknown key in the global file is reported with its path

- **GIVEN** a global config containing `[broker] prot = 9119`
- **WHEN** the user runs `git paw config check`
- **THEN** the output SHALL report a warning for the global config path with key `broker.prot`

#### Scenario: Unknown default CLI is reported

- **GIVEN** a repo config with `default_cli = "clade"` and no `[clis.clade]`
- **WHEN** the user runs `git paw config check`
- **THEN** the output SHALL report a warning for key `default_cli`

#### Scenario: Explicit empty values are not unknown

- **GIVEN** a repo config with `[worktree.env] copy = []`
- **WHEN** the user runs `git paw config check`
- **THEN** no finding SHALL be reported for `worktree.env.copy`

### Requirement: Deprecated config tables are reported once each

`git paw config check` SHALL report each deprecated table, including `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]` (deprecated in 0.17.0, no effect; see *Deprecated auto-approval tables are parsed but have no effect*), as one warning for the whole table that names the deprecating release and states that it has no effect and can be removed, with no further finding for the table's fields or values.

#### Scenario: Deprecated auto-approval tables are reported once each

- **GIVEN** a repo config with `[supervisor.auto_approve] enabled = true` and `[supervisor.common_dev_allowlist] stacks = ["rust", "elixir"]`
- **WHEN** the user runs `git paw config check`
- **THEN** the output SHALL report exactly one warning with key `supervisor.auto_approve` and exactly one with key `supervisor.common_dev_allowlist`, each naming release 0.17.0 and stating that the table has no effect
- **AND** no finding SHALL name `elixir` or a field of either table

### Requirement: `git paw config check` exit status and help

`git paw config check` SHALL exit 0 when no file has an error finding and 1 when any file has an error finding. With `--strict` it SHALL also exit 1 when any warning finding exists. The subcommand SHALL carry `about`, `long_about` with examples, and a `help` string on every flag, and its help SHALL state that it validates configuration files while `git paw doctor` diagnoses the environment.

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

### Requirement: `git paw config check --json` document

With `--json`, `git paw config check` SHALL follow the `cli-machine-output` contract and print one document with `schema_version`; `files`, one entry per config file considered with `path`, `kind` (`"global"` or `"repo"`), `present` and `config_version` (as reported by the migration API, or `null` when absent or unparseable); and `findings`, one entry per finding with `file`, `key_path` (or `null`), `severity` (`"error"` or `"warning"`), `message` and `remedy`.

#### Scenario: JSON output carries every finding

- **GIVEN** a repo config with one unknown key and one deprecated key
- **WHEN** the user runs `git paw config check --json`
- **THEN** stdout SHALL be one JSON object with `schema_version` 1, a `files` entry for the repo config, and `findings` listing both findings with `file`, `key_path`, `severity`, `message` and `remedy`

### Requirement: `git paw config check --json` prints the document on error findings

With `--json`, `git paw config check` SHALL exit with the same status as the human mode. Error findings are results, not a command failure: the document SHALL be printed before exiting 1. Only a failure of the command itself (for example an unreadable file system) SHALL leave stdout empty.

#### Scenario: JSON output is printed when an error is found

- **GIVEN** a repo config with invalid TOML
- **WHEN** the user runs `git paw config check --json`
- **THEN** stdout SHALL be the document with an `error` finding for the repo config and its `config_version` null
- **AND** the process SHALL exit 1

### Requirement: Doctor does not report fallback defaults after a config load error

When the configuration fails to load, `git paw doctor` SHALL report the load error in the Config group and SHALL report each check whose result depends on configuration values — the Spec-system, Broker and Supervisor groups, the configured-CLI rows, and root-skill staleness — as one ○ (skipped) check per group, not ⚠, whose detail starts with "not evaluated" and whose remedy points at the config error. It SHALL NOT report values taken from built-in defaults as the configuration.

#### Scenario: A TOML error does not read as a disabled broker

- **GIVEN** a repo config with `[broker] enabled = true`, `[supervisor] enabled = true`, `[specs] type = "openspec"`, and a syntax error on line 2
- **WHEN** `git paw doctor` is run
- **THEN** the Config group SHALL report ✗ naming the error
- **AND** the Broker, Supervisor and Spec-system groups SHALL each report ○ "not evaluated"
- **AND** the report SHALL NOT say the broker or supervisor is disabled or that no spec system is configured

#### Scenario: JSON carries the not-evaluated checks

- **GIVEN** the same config
- **WHEN** `git paw doctor --json` is run
- **THEN** the Broker, Supervisor and Spec-system entries SHALL have status `skipped` and a detail starting with "not evaluated"
- **AND** the overall `status` SHALL be `fail`, from the Config ✗, and not `skipped`
- **AND** the process SHALL exit non-zero

### Requirement: Configuration-independent checks run despite a config load error

When the configuration fails to load, `git paw doctor` SHALL still run and report every check that does not depend on configuration values: the Environment group, the known-CLI roster on PATH, the Python 3 interpreter, the presence, mode and content of the bundled helpers, and the Hygiene group.

#### Scenario: Environment and hygiene are still reported

- **GIVEN** a repo config with invalid TOML and `.gitignore` missing `.git-paw/tmp/`
- **WHEN** `git paw doctor` is run
- **THEN** the Environment group SHALL report git and tmux as usual
- **AND** the Hygiene group SHALL report ⚠ for the missing `.gitignore` entry

### Requirement: An unparseable config file yields only its syntax finding

When a config file is not valid TOML, `git paw config check` SHALL report that file's syntax error and no other finding for that file, and SHALL still validate the other file on its own.

#### Scenario: Broken repo file, valid global file

- **GIVEN** a repo config with invalid TOML and a global config containing an unknown key `[broker] prot = 1`
- **WHEN** the user runs `git paw config check`
- **THEN** the repo config SHALL have exactly one finding, the syntax error
- **AND** the global config SHALL have the warning for `broker.prot`
