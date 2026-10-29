## RENAMED Requirements

- FROM: `### Requirement: Supervisor enabled resolution chain`
- TO: `### Requirement: Start-mode resolution chain`
- FROM: `### Requirement: Init prompts for supervisor configuration`
- TO: `### Requirement: Init prompts for the default start mode`
- FROM: ### Requirement: `git paw init` SHALL write a commented-out [supervisor] block enumerating gate keys
- TO: ### Requirement: `git paw init` SHALL write commented-out coordination tables enumerating gate keys

## MODIFIED Requirements

### Requirement: Start-mode resolution chain

The system SHALL decide whether a session runs in coordinated mode or bare mode using this resolution chain, first match wins:

1. `--bare` (or its deprecated spelling `--no-supervisor`) → bare, no prompt
2. `--coordinated` (or its deprecated spelling `--supervisor`) or `--unattended` → coordinated, no prompt
3. `default_mode = "coordinated"` in the merged config → coordinated, no prompt
4. `default_mode = "bare"` in the merged config → bare, no prompt (explicit opt-out)
5. no `default_mode` and `--dry-run` → bare, no prompt
6. no `default_mode` → prompt the user "Start in coordinated mode? (y/n)"; when stdin is not a terminal, bare without prompting

`default_mode` is read after migration, so a version-1 `[supervisor]` table resolves through it (`enabled = true` → coordinated; `enabled = false` or no `enabled` → bare). The distinction between an absent `default_mode` (unconfigured, prompts) and `default_mode = "bare"` (explicitly bare, never prompts) SHALL be preserved.

#### Scenario: The coordinated flag wins over config

- **GIVEN** a config with `default_mode = "bare"`
- **WHEN** `--coordinated` is passed
- **THEN** coordinated mode SHALL be active without prompting

#### Scenario: Config selects coordinated mode without a flag

- **GIVEN** a config with `default_mode = "coordinated"`
- **WHEN** no mode flag is passed
- **THEN** coordinated mode SHALL be active without prompting

#### Scenario: The bare flag overrides config

- **GIVEN** a config with `default_mode = "coordinated"`
- **WHEN** `--bare` is passed
- **THEN** the session SHALL run in bare mode without prompting

#### Scenario: Explicit bare does not prompt

- **GIVEN** a config with `default_mode = "bare"`
- **WHEN** no mode flag is passed
- **THEN** the session SHALL run in bare mode
- **AND** the user SHALL NOT be prompted

#### Scenario: A version-1 table resolves through default_mode

- **GIVEN** a version-1 config with `[supervisor] enabled = true`
- **WHEN** no mode flag is passed
- **THEN** coordinated mode SHALL be active without prompting

#### Scenario: No default_mode prompts the user

- **GIVEN** a config with no `default_mode` and no `[supervisor]` table, and an interactive terminal
- **WHEN** no mode flag is passed
- **THEN** the user SHALL be prompted "Start in coordinated mode? (y/n)"

#### Scenario: Dry run skips the prompt

- **GIVEN** a config with no `default_mode`
- **WHEN** `--dry-run` is passed without a mode flag
- **THEN** the dry run SHALL assume bare mode (no prompt)

### Requirement: Init prompts for the default start mode

During `git paw init`, when the config has neither `default_mode` nor a version-1 `[supervisor]` table, the system SHALL prompt the user:

1. "Use coordinated mode by default? (y/n)"
2. If yes: "Test command to run after each agent completes (e.g. 'just check', leave empty to skip):"

A yes SHALL write `default_mode = "coordinated"` and, when a test command was entered, `[orchestrator.gates]` with `test_command`. A no SHALL write `default_mode = "bare"` (explicit opt-out, preventing future prompts during `start`). A non-interactive init SHALL write `default_mode = "bare"`. When the config already has `default_mode` or a `[supervisor]` table, init SHALL NOT prompt and SHALL NOT change either.

#### Scenario: Init with coordinated mode enabled

- **WHEN** the user answers "yes" and enters "just check" as test command
- **THEN** the generated config SHALL contain `default_mode = "coordinated"` and `[orchestrator.gates]` with `test_command = "just check"`
- **AND** it SHALL NOT contain `[supervisor]`

#### Scenario: Init with coordinated mode declined

- **WHEN** the user answers "no"
- **THEN** the generated config SHALL contain `default_mode = "bare"`
- **AND** future `git paw start` calls SHALL NOT prompt about the mode

#### Scenario: Init leaves a version-1 table alone

- **GIVEN** an existing config with `[supervisor] enabled = false`
- **WHEN** `git paw init` runs interactively
- **THEN** no mode prompt SHALL be shown and the `[supervisor]` table SHALL be byte-identical

### Requirement: `git paw init` SHALL write commented-out coordination tables enumerating gate keys

The config `git paw init` writes SHALL include commented-out `[orchestrator]`, `[orchestrator.gates]`, `[workers]` and `[drive]` blocks, each line prefixed with `#` so TOML parsing ignores them. The `[orchestrator.gates]` block SHALL list the eight gate-command keys (`test_command`, `lint_command`, `build_command`, `doc_build_command`, `doc_tool_command`, `spec_validate_command`, `fmt_check_command`, `security_audit_command`) with one example value each, illustrating common stacks; the `[orchestrator]` block SHALL list `cli` and `approval`.

#### Scenario: `git paw init` writes the commented tables

- **GIVEN** a fresh repository with no `.git-paw/config.toml`
- **WHEN** `git paw init` is invoked non-interactively
- **THEN** `.git-paw/config.toml` SHALL contain the commented headers `# [orchestrator]`, `# [orchestrator.gates]`, `# [workers]` and `# [drive]` on their own lines
- **AND** a commented line for each of the eight gate-command keys

#### Scenario: The commented tables are valid TOML when uncommented

- **GIVEN** the four `git paw init`-written commented blocks
- **WHEN** every line of those blocks prefixed with `# ` has its prefix stripped
- **THEN** the resulting TOML SHALL parse as a version-2 config without error
- **AND** the eight gate commands, `[orchestrator] cli` and `approval` SHALL carry the example values

### Requirement: Deprecated auto-approval tables are parsed but have no effect

The system SHALL continue to accept, in a version-1 configuration, the `[supervisor.auto_approve]` table (`enabled`, `safe_commands`, `stall_threshold_seconds`, `approval_level` with the values `off` / `conservative` / `safe`, and `approve_worktree_writes`) and the `[supervisor.common_dev_allowlist]` table (`enabled`, `stacks`, `extra`) with their existing field names and types, so configurations written for earlier versions load without error. Neither table nor any of its fields SHALL have any behavioural effect. No prompt is approved, no skill text is rendered and no threshold is applied because of them, and a value that previously triggered a clamp warning (such as `stall_threshold_seconds = 0`) SHALL NOT trigger one.

The version-1 to version-2 configuration migration SHALL drop both tables. Version 2 has no path for them, and they SHALL NOT be moved under `[workers]` or any other version-2 table. Because git-paw serializes only the version-2 layout, saving a configuration SHALL NOT write either table, whether or not the loaded file declared it. When `git paw add-cli` or `git paw remove-cli` rewrites a global file that declared either table, its one-line rewrite notice SHALL name each dropped table. git-paw SHALL NOT rewrite any other file to drop them.

When a configuration file declares either table, the system SHALL print exactly one warning line to stderr for that file per git-paw process, through the shared deprecation helper (`core-deprecation`). The line SHALL name every deprecated table the file declares and state that it has no effect and can be removed. A file that declares neither table SHALL produce no such warning. Each table SHALL be one deprecation registry entry of kind configuration key, deprecated in 0.17.0, with no removal release, because version-1 files are read through migration.

#### Scenario: An old configuration with both tables loads with one warning

- **GIVEN** a `.git-paw/config.toml` declaring `[supervisor.auto_approve]` with every field set and `[supervisor.common_dev_allowlist]` with `stacks = ["rust"]` and `extra = ["just lint"]`
- **WHEN** a git-paw subcommand that loads the repository configuration runs
- **THEN** loading SHALL succeed
- **AND** stderr SHALL contain exactly one warning line about the two tables, naming both `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]` and stating that they have no effect

#### Scenario: The warning names only the declared table

- **GIVEN** a configuration file that declares `[supervisor.auto_approve]` and not `[supervisor.common_dev_allowlist]`
- **WHEN** it is loaded
- **THEN** the deprecation warning SHALL name `[supervisor.auto_approve]`
- **AND** SHALL NOT name `[supervisor.common_dev_allowlist]`

#### Scenario: A configuration without the tables produces no warning about them

- **GIVEN** a configuration file with an `[orchestrator]` table and neither deprecated table
- **WHEN** it is loaded
- **THEN** loading SHALL succeed
- **AND** no warning naming `[supervisor.auto_approve]` or `[supervisor.common_dev_allowlist]` SHALL be printed

#### Scenario: A rewrite drops the tables

- **GIVEN** a global configuration declaring both tables with non-default values
- **WHEN** it is loaded and saved back by `git paw add-cli`
- **THEN** the saved file SHALL contain neither table, under `[supervisor]`, `[workers]` or any other name
- **AND** the rewrite notice SHALL name both tables as dropped

#### Scenario: A repository file is never rewritten to drop them

- **GIVEN** a `.git-paw/config.toml` declaring `[supervisor.auto_approve] enabled = true`
- **WHEN** `git paw status` and `git paw init` run
- **THEN** the file's `[supervisor.auto_approve]` table SHALL be byte-identical afterwards

#### Scenario: The tables have no effect on a run

- **GIVEN** a configuration declaring `[supervisor.auto_approve] safe_commands = ["ls"]` and `[supervisor.common_dev_allowlist] stacks = ["rust"]`
- **WHEN** an `--unattended` run's agent pane shows a live permission prompt for `ls -la`
- **THEN** the prompt SHALL be escalated
- **AND** no keystroke SHALL be sent to that pane
