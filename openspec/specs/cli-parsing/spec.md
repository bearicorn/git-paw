## Purpose

Define the command-line interface for git-paw using clap v4. Declares all subcommands (`start`, `stop`, `purge`, `status`, `list-clis`, `add-cli`, `remove-cli`), their flags, and argument validation. When no subcommand is given, defaults to `start`.

## Requirements

### Requirement: Default to start when no subcommand is given

The system SHALL treat no arguments as equivalent to `start` with no flags.

#### Scenario: No arguments yields None command
- **GIVEN** no arguments are passed
- **WHEN** the CLI is parsed
- **THEN** `command` SHALL be `None` (handled as `Start` in main)

Test: `cli::tests::no_args_defaults_to_none_command`

### Requirement: Start subcommand with optional flags

The `start` subcommand SHALL accept `--cli`, `--branches` (comma-separated), `--dry-run`, and `--preset` flags, all optional.

#### Scenario: Start with no flags
- **GIVEN** `start` is passed with no flags
- **WHEN** the CLI is parsed
- **THEN** all optional fields SHALL be `None` / `false`

Test: `cli::tests::start_with_no_flags`

#### Scenario: Start with --cli flag
- **GIVEN** `start --cli claude`
- **WHEN** the CLI is parsed
- **THEN** `cli` SHALL be `Some("claude")`

Test: `cli::tests::start_with_cli_flag`

#### Scenario: Start with comma-separated --branches flag
- **GIVEN** `start --branches feat/a,feat/b,fix/c`
- **WHEN** the CLI is parsed
- **THEN** `branches` SHALL be `["feat/a", "feat/b", "fix/c"]`

Test: `cli::tests::start_with_branches_flag_comma_separated`

#### Scenario: Start with --dry-run flag
- **GIVEN** `start --dry-run`
- **WHEN** the CLI is parsed
- **THEN** `dry_run` SHALL be `true`

Test: `cli::tests::start_with_dry_run`

#### Scenario: Start with --preset flag
- **GIVEN** `start --preset backend`
- **WHEN** the CLI is parsed
- **THEN** `preset` SHALL be `Some("backend")`

Test: `cli::tests::start_with_preset`

#### Scenario: Start with all flags combined
- **GIVEN** `start --cli gemini --branches a,b --dry-run --preset dev`
- **WHEN** the CLI is parsed
- **THEN** all fields SHALL be populated correctly

Test: `cli::tests::start_with_all_flags`

### Requirement: Stop subcommand

The `stop` subcommand SHALL parse with no additional arguments.

#### Scenario: Stop parses
- **GIVEN** `stop` is passed
- **WHEN** the CLI is parsed
- **THEN** the command SHALL be `Command::Stop`

Test: `cli::tests::stop_parses`

### Requirement: Purge subcommand with optional --force flag

The `purge` subcommand SHALL accept an optional `--force` flag (defaults to `false`).

#### Scenario: Purge without --force
- **GIVEN** `purge` is passed without flags
- **WHEN** the CLI is parsed
- **THEN** `force` SHALL be `false`

Test: `cli::tests::purge_without_force`

#### Scenario: Purge with --force
- **GIVEN** `purge --force` is passed
- **WHEN** the CLI is parsed
- **THEN** `force` SHALL be `true`

Test: `cli::tests::purge_with_force`

### Requirement: Status subcommand

The `status` subcommand SHALL parse with no additional arguments.

#### Scenario: Status parses
- **GIVEN** `status` is passed
- **WHEN** the CLI is parsed
- **THEN** the command SHALL be `Command::Status`

Test: `cli::tests::status_parses`

### Requirement: List-CLIs subcommand

The `list-clis` subcommand SHALL parse with no additional arguments.

#### Scenario: List-CLIs parses
- **GIVEN** `list-clis` is passed
- **WHEN** the CLI is parsed
- **THEN** the command SHALL be `Command::ListClis`

Test: `cli::tests::list_clis_parses`

### Requirement: Add-CLI subcommand with required and optional arguments

The `add-cli` subcommand SHALL require `name` and `command` positional arguments and accept an optional `--display-name` flag.

#### Scenario: Add-CLI with required arguments only
- **GIVEN** `add-cli my-agent /usr/local/bin/my-agent`
- **WHEN** the CLI is parsed
- **THEN** `name` SHALL be `"my-agent"`, `command` SHALL be the path, and `display_name` SHALL be `None`

Test: `cli::tests::add_cli_with_required_args`

#### Scenario: Add-CLI with --display-name
- **GIVEN** `add-cli my-agent my-agent --display-name "My Agent"`
- **WHEN** the CLI is parsed
- **THEN** `display_name` SHALL be `Some("My Agent")`

Test: `cli::tests::add_cli_with_display_name`

#### Scenario: Add-CLI missing required arguments is rejected
- **GIVEN** `add-cli` with no positional arguments
- **WHEN** the CLI is parsed
- **THEN** parsing SHALL fail

Test: `cli::tests::add_cli_missing_required_args_is_rejected`

### Requirement: Remove-CLI subcommand with required argument

The `remove-cli` subcommand SHALL require a `name` positional argument.

#### Scenario: Remove-CLI parses
- **GIVEN** `remove-cli my-agent`
- **WHEN** the CLI is parsed
- **THEN** `name` SHALL be `"my-agent"`

Test: `cli::tests::remove_cli_parses`

### Requirement: Standard flags --version and --help

The CLI SHALL accept `--version` and `--help` flags.

#### Scenario: --version flag is accepted
- **GIVEN** `--version` is passed
- **WHEN** the CLI is parsed
- **THEN** clap SHALL emit a `DisplayVersion` response

Test: `cli::tests::version_flag_is_accepted`

#### Scenario: --help flag is accepted
- **GIVEN** `--help` is passed
- **WHEN** the CLI is parsed
- **THEN** clap SHALL emit a `DisplayHelp` response

Test: `cli::tests::help_flag_is_accepted`

### Requirement: Unknown subcommands are rejected

The CLI SHALL reject unrecognized subcommands with a parse error.

#### Scenario: Unknown subcommand fails
- **GIVEN** an unrecognized subcommand is passed
- **WHEN** the CLI is parsed
- **THEN** parsing SHALL fail

Test: `cli::tests::unknown_subcommand_is_rejected`

### Requirement: Help output contains all subcommands and quick start

The `--help` output SHALL list all subcommands and include a Quick Start section.

#### Scenario: Help lists all subcommands
- **GIVEN** `--help` is passed
- **WHEN** the binary runs
- **THEN** stdout SHALL contain start, stop, purge, status, list-clis, add-cli, remove-cli, init, and replay

Test: `cli_tests::help_shows_all_subcommands`

#### Scenario: Help contains Quick Start
- **GIVEN** `--help` is passed
- **WHEN** the binary runs
- **THEN** stdout SHALL contain "Quick Start"

Test: `cli_tests::help_contains_quick_start`

#### Scenario: Start help shows all flags
- **GIVEN** `start --help` is passed
- **WHEN** the binary runs
- **THEN** stdout SHALL contain --cli, --branches, --dry-run, and --preset

Test: `cli_tests::start_help_shows_flags`

#### Scenario: Purge help shows --force flag
- **GIVEN** `purge --help` is passed
- **WHEN** the binary runs
- **THEN** stdout SHALL contain --force

Test: `cli_tests::purge_help_shows_force_flag`

#### Scenario: Add-CLI help shows arguments
- **GIVEN** `add-cli --help` is passed
- **WHEN** the binary runs
- **THEN** stdout SHALL contain --display-name, name, and command arguments

Test: `cli_tests::add_cli_help_shows_arguments`

### Requirement: Version output includes binary name

The `--version` output SHALL include the binary name.

#### Scenario: Version output
- **GIVEN** `--version` is passed
- **WHEN** the binary runs
- **THEN** stdout SHALL contain "git-paw"

Test: `cli_tests::version_output`

### Requirement: No arguments behaves like start

When no subcommand is provided, the binary SHALL behave identically to `start`.

#### Scenario: No args produces same error as start
- **GIVEN** the binary is run with no arguments outside a git repo
- **WHEN** both `git-paw` and `git-paw start` are run
- **THEN** they SHALL produce identical stderr output

Test: `cli_tests::no_args_behaves_like_start`

### Requirement: Subcommands run without error when applicable

Subcommands that don't require a session SHALL succeed in a valid git repo.

#### Scenario: Stop runs without error
- **GIVEN** the binary is run in a git repo
- **WHEN** `stop` is passed
- **THEN** it SHALL succeed

Test: `cli_tests::stop_runs_without_error`

#### Scenario: Status runs without error
- **GIVEN** the binary is run in a git repo
- **WHEN** `status` is passed
- **THEN** it SHALL succeed

Test: `cli_tests::status_runs_without_error`

#### Scenario: List-CLIs runs without error
- **GIVEN** the binary is run in a git repo
- **WHEN** `list-clis` is passed
- **THEN** it SHALL succeed

Test: `cli_tests::list_clis_runs_without_error`

### Requirement: Binary rejects missing required arguments

Subcommands with required arguments SHALL fail when they are missing.

#### Scenario: Add-CLI requires arguments
- **GIVEN** `add-cli` is passed with no arguments
- **WHEN** the binary runs
- **THEN** it SHALL fail with stderr mentioning "required"

Test: `cli_tests::add_cli_requires_arguments`

#### Scenario: Remove-CLI requires argument
- **GIVEN** `remove-cli` is passed with no arguments
- **WHEN** the binary runs
- **THEN** it SHALL fail with stderr mentioning "required"

Test: `cli_tests::remove_cli_requires_argument`

### Requirement: Not-a-repo error from binary

Commands requiring a git repo SHALL fail with an actionable error when run outside one.

#### Scenario: Start from non-git directory
- **GIVEN** the binary is run outside a git repository
- **WHEN** `start` is passed
- **THEN** it SHALL fail with stderr containing "Not a git repository"

Test: `cli_tests::start_from_non_git_dir`

#### Scenario: Unknown subcommand from binary
- **GIVEN** the binary is passed an unrecognized subcommand
- **WHEN** it runs
- **THEN** it SHALL fail with stderr containing "error"

Test: `cli_tests::unknown_subcommand_fails`

### Requirement: Replay subcommand

The `replay` subcommand SHALL accept an optional `<branch>` positional argument, a `--list` flag, a `--color` flag, and an optional `--session` flag.

#### Scenario: Replay with branch
- **WHEN** `replay feat/add-auth` is passed
- **THEN** `branch` SHALL be `Some("feat/add-auth")`, `list` SHALL be `false`, `color` SHALL be `false`

#### Scenario: Replay with --list
- **WHEN** `replay --list` is passed
- **THEN** `list` SHALL be `true` and `branch` SHALL be `None`

#### Scenario: Replay with --color
- **WHEN** `replay feat/add-auth --color` is passed
- **THEN** `color` SHALL be `true`

#### Scenario: Replay with --session
- **WHEN** `replay feat/add-auth --session paw-myproject` is passed
- **THEN** `session` SHALL be `Some("paw-myproject")`

#### Scenario: Replay with no arguments and no --list
- **WHEN** `replay` is passed with no arguments and no `--list`
- **THEN** parsing SHALL fail with an error indicating either a branch or `--list` is required

#### Scenario: Replay help text
- **WHEN** `replay --help` is passed
- **THEN** stdout SHALL contain descriptions of `--list`, `--color`, and `--session` flags with examples

### Requirement: Init subcommand

The `init` subcommand SHALL parse with no required arguments.

#### Scenario: Init parses
- **WHEN** `init` is passed
- **THEN** the command SHALL be `Command::Init`

#### Scenario: Init help text
- **WHEN** `init --help` is passed
- **THEN** stdout SHALL contain a description of project initialization and examples
