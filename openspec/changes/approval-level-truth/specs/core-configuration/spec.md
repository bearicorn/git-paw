## ADDED Requirements

### Requirement: Version-1 `auto` approval values load as `accept-edits`

In a version-1 file `"auto"` means what is now called `accept-edits`. For any document that declares `version = 1` or no version, the version-1 to version-2 migration step SHALL therefore also:

- rewrite the approval value `"auto"` to `"accept-edits"` at `[supervisor] agent_approval`, `[supervisor] approval` and every `[profiles.<name>] approval`. The rewrite applies before or together with those keys' moves to `[workers] approval` and `[orchestrator] approval`.
- rename every `[clis.<name>].approval_args` key `auto` to `accept-edits`, keeping its value.

Every other approval value and key SHALL be left as written. Like the rest of the step, this SHALL be a pure, in-memory function of the document, and SHALL NOT write, rename or touch the file. A version-2 document SHALL NOT be rewritten: in it, `"auto"` means the `Auto` level of *Permission flag mapping*.

A requirement or scenario in any capability that names a version-1 path set to `"auto"` SHALL be read as naming that path's version-2 name set to `"accept-edits"`.

#### Scenario: A version-1 auto worker launches exactly as before

- **GIVEN** a version-1 config with `[supervisor] agent_approval = "auto"` and a `claude` worker
- **WHEN** the worker's pane command is built
- **THEN** it SHALL contain `--permission-mode acceptEdits`
- **AND** it SHALL be byte-identical to the command v0.20.0 built for the same config

#### Scenario: Every version-1 auto path migrates

- **GIVEN** a version-1 config with `[supervisor] agent_approval = "auto"`, `[supervisor] approval = "auto"`, `[profiles.routine] kind = "worker"` with `approval = "auto"`, and `[clis.claude] approval_args = { auto = "--permission-mode auto" }`
- **WHEN** it is loaded
- **THEN** the worker approval, the orchestrator approval and `routine`'s approval SHALL be `accept-edits`
- **AND** `[clis.claude].approval_args` SHALL map `accept-edits` to `--permission-mode auto` and SHALL have no `auto` key

#### Scenario: A hand-written auto-mode override keeps working

- **GIVEN** a version-1 config with `[supervisor] agent_approval = "auto"` and `[clis.claude] approval_args = { auto = "--permission-mode auto" }`
- **WHEN** a `claude` worker's pane command is built
- **THEN** it SHALL contain `--permission-mode auto` and SHALL NOT contain `acceptEdits`

#### Scenario: A version-2 auto is the CLI's own auto mode

- **GIVEN** a config with `version = 2` and `[workers] approval = "auto"` and a `claude` worker
- **WHEN** its pane command is built
- **THEN** it SHALL contain `--permission-mode auto`

#### Scenario: Migration leaves the file untouched

- **GIVEN** a version-1 `.git-paw/config.toml` containing `[profiles.routine] approval = "auto"`
- **WHEN** any command loads the config
- **THEN** the file's bytes and modification time SHALL be unchanged

### Requirement: A version-1 `auto` warns once per file

When a loaded version-1 file writes `"auto"` at an approval path, or an `approval_args` key `auto`, git-paw SHALL print exactly one warning line for that file per process, through the shared deprecation helper (`core-deprecation`). The line SHALL name the file, each such path, and the replacement `accept-edits`, and SHALL state that in version 2 `auto` launches the CLI's own auto mode. The registry SHALL hold one entry for this item with:

- the item `"auto"` as a version-1 approval value;
- the replacement `"accept-edits"`;
- the deprecating release 0.21.0;
- no removal release, because the item is read through migration.

`git paw config check` SHALL report it from that entry as a warning. The warning SHALL go to stderr only and SHALL NOT change stdout or the exit code. A file that leaves its approvals unset, or uses only other values, SHALL NOT produce it.

#### Scenario: One warning names every path

- **GIVEN** a version-1 config with `[supervisor] agent_approval = "auto"` and `[profiles.routine] approval = "auto"`
- **WHEN** `git paw status` runs
- **THEN** stderr SHALL contain exactly one line naming the file, `supervisor.agent_approval`, `profiles.routine.approval`, `accept-edits` and release 0.21.0
- **AND** stdout and the exit code SHALL equal those of a run with both values written as `accept-edits` in a version-2 file

#### Scenario: Unset approvals do not warn

- **GIVEN** a version-1 config with `[supervisor] enabled = true` and no approval keys
- **WHEN** `git paw status` runs
- **THEN** stderr SHALL contain no line about `auto`

#### Scenario: Config check reports the version-1 auto

- **GIVEN** a version-1 config with `[clis.claude] approval_args = { auto = "--x" }`
- **WHEN** `git paw config check` runs
- **THEN** it SHALL report a warning for `clis.claude.approval_args.auto` naming `accept-edits` and release 0.21.0

### Requirement: Rewriting or stamping a file never changes what its `auto` means

`git paw init` SHALL NOT backfill `version = 2` into a file that writes `"auto"` at an approval path or uses an `approval_args` key `auto`. For such a file it SHALL leave `version` absent and print its one-line pointer to `git paw config check`, exactly as it does for a file with a `[supervisor]` table. When `git paw add-cli` or `git paw remove-cli` rewrites a version-1 global file, the written file SHALL carry `accept-edits` wherever the version-1 file meant it, and the rewrite notice SHALL name each approval value or key it renamed.

#### Scenario: Init does not widen a profile by stamping the version

- **GIVEN** a `.git-paw/config.toml` with no `version`, no `[supervisor]` table and `[profiles.routine] kind = "worker"`, `approval = "auto"`
- **WHEN** `git paw init` runs
- **THEN** the file SHALL still have no `version` key
- **AND** stdout SHALL carry one line pointing at `git paw config check`

#### Scenario: Add-cli writes the migrated value

- **GIVEN** a version-1 global config with `[clis.claude] approval_args = { auto = "--permission-mode auto" }`
- **WHEN** `git paw add-cli my-agent my-agent` rewrites it
- **THEN** the written file SHALL declare `version = 2` and map `accept-edits` to `--permission-mode auto` under `[clis.claude].approval_args`
- **AND** the rewrite notice SHALL name `clis.claude.approval_args.auto`
