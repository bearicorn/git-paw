## Purpose

Define the central error type `PawError` used across all git-paw modules. Every variant carries an actionable, user-facing message and maps to a process exit code.
## Requirements
### Requirement: Actionable error messages for each variant

Each `PawError` variant SHALL produce a user-facing message that explains the problem and suggests a remedy where appropriate.

#### Scenario: NotAGitRepo is actionable
- **GIVEN** `PawError::NotAGitRepo`
- **WHEN** formatted with `Display`
- **THEN** the message SHALL mention "git repository" and name the tool

Test: `error::tests::test_not_a_git_repo_is_actionable`

#### Scenario: TmuxNotInstalled includes install instructions
- **GIVEN** `PawError::TmuxNotInstalled`
- **WHEN** formatted with `Display`
- **THEN** the message SHALL include both `brew install` and `apt install` hints

Test: `error::tests::test_tmux_not_installed_includes_install_instructions`

#### Scenario: NoCLIsFound suggests add-cli
- **GIVEN** `PawError::NoCLIsFound`
- **WHEN** formatted with `Display`
- **THEN** the message SHALL suggest the `add-cli` command

Test: `error::tests::test_no_clis_found_suggests_add_cli`

#### Scenario: WorktreeError includes detail
- **GIVEN** `PawError::WorktreeError("failed to create")`
- **WHEN** formatted with `Display`
- **THEN** the message SHALL include the inner detail string

Test: `error::tests::test_worktree_error_includes_detail`

#### Scenario: SessionError includes detail
- **GIVEN** `PawError::SessionError("file corrupt")`
- **WHEN** formatted with `Display`
- **THEN** the message SHALL include the inner detail string

Test: `error::tests::test_session_error_includes_detail`

#### Scenario: ConfigError includes detail
- **GIVEN** `PawError::ConfigError("invalid toml")`
- **WHEN** formatted with `Display`
- **THEN** the message SHALL include the inner detail string

Test: `error::tests::test_config_error_includes_detail`

#### Scenario: BranchError includes detail
- **GIVEN** `PawError::BranchError("not found")`
- **WHEN** formatted with `Display`
- **THEN** the message SHALL include the inner detail string

Test: `error::tests::test_branch_error_includes_detail`

#### Scenario: UserCancelled has a message
- **GIVEN** `PawError::UserCancelled`
- **WHEN** formatted with `Display`
- **THEN** the message SHALL not be empty

Test: `error::tests::test_user_cancelled_is_not_empty`

#### Scenario: TmuxError includes detail
- **GIVEN** `PawError::TmuxError("session failed")`
- **WHEN** formatted with `Display`
- **THEN** the message SHALL include the inner detail string

Test: `error::tests::test_tmux_error_includes_detail`

#### Scenario: CliNotFound includes CLI name
- **GIVEN** `PawError::CliNotFound("my-agent")`
- **WHEN** formatted with `Display`
- **THEN** the message SHALL include the missing CLI name

Test: `error::tests::test_cli_not_found_includes_cli_name`

### Requirement: Exit codes distinguish cancellation from errors

`UserCancelled` SHALL exit with code 2; all other errors SHALL exit with code 1.

#### Scenario: UserCancelled exit code
- **GIVEN** `PawError::UserCancelled`
- **WHEN** `exit_code()` is called
- **THEN** it SHALL return `2`

Test: `error::tests::test_user_cancelled_exit_code`

#### Scenario: General errors exit code
- **GIVEN** any non-cancellation error variant
- **WHEN** `exit_code()` is called
- **THEN** it SHALL return `1`

Test: `error::tests::test_general_errors_exit_code`

### Requirement: Exit method prints to stderr and exits with correct code

`PawError::exit()` SHALL print the error message to stderr and terminate with the appropriate exit code.

#### Scenario: NotAGitRepo exits with code 1
- **GIVEN** the binary is run outside a git repository
- **WHEN** the error propagates to `exit()`
- **THEN** the process SHALL exit with code 1 and stderr SHALL contain the error message

Test: `e2e_tests::error_exit_code_is_1_for_not_a_git_repo`

#### Scenario: ConfigError exits with code 1
- **GIVEN** a nonexistent preset is requested
- **WHEN** the error propagates to `exit()`
- **THEN** the process SHALL exit with code 1 and stderr SHALL mention "not found"

Test: `e2e_tests::error_exit_code_is_1_for_preset_not_found`

### Requirement: Debug representation is derivable

All `PawError` variants SHALL support `Debug` formatting.

#### Scenario: Debug format includes variant name
- **GIVEN** `PawError::NotAGitRepo`
- **WHEN** formatted with `Debug`
- **THEN** the output SHALL contain `"NotAGitRepo"`

Test: `error::tests::test_debug_derived`

### Requirement: SkillError variants with actionable messages

The system SHALL define a `SkillError` type with variants for skill loading failures. Each variant SHALL produce a user-facing message that explains the problem and suggests a remedy. `SkillError` SHALL be wrappable inside `PawError` as a variant.

The following variants SHALL exist:

- `UnknownSkill { name: String }` — no embedded or user override found for the requested skill name
- `UserOverrideRead { path: PathBuf, source: std::io::Error }` — a user override file exists but cannot be read

#### Scenario: UnknownSkill is actionable
- **GIVEN** `SkillError::UnknownSkill { name: "nonexistent" }`
- **WHEN** formatted with `Display`
- **THEN** the message SHALL mention the skill name `"nonexistent"` and indicate no embedded default exists

#### Scenario: UserOverrideRead is actionable
- **GIVEN** `SkillError::UserOverrideRead { path: "/home/user/.config/git-paw/agent-skills/coordination.md", .. }`
- **WHEN** formatted with `Display`
- **THEN** the message SHALL include the file path and suggest checking permissions

#### Scenario: SkillError exit code
- **GIVEN** any `SkillError` variant wrapped in `PawError`
- **WHEN** `exit_code()` is called
- **THEN** it SHALL return `1`

### Requirement: BrokerError variants with actionable messages

The system SHALL define a `BrokerError` type with variants for broker-specific failures. Each variant SHALL produce a user-facing message that explains the problem and suggests a remedy. `BrokerError` SHALL be wrappable inside `PawError` as a variant.

The following variants SHALL exist:

- `PortInUse { port: u16 }` — the configured port is already occupied
- `ProbeTimeout { port: u16 }` — the stale-broker probe timed out
- `BindFailed(std::io::Error)` — socket bind failed for a reason other than port-in-use
- `RuntimeFailed(std::io::Error)` — tokio runtime construction failed

#### Scenario: PortInUse is actionable
- **GIVEN** `BrokerError::PortInUse { port: 9119 }`
- **WHEN** formatted with `Display`
- **THEN** the message SHALL mention port `9119` and suggest changing `[broker] port` in config

#### Scenario: ProbeTimeout is actionable
- **GIVEN** `BrokerError::ProbeTimeout { port: 9119 }`
- **WHEN** formatted with `Display`
- **THEN** the message SHALL mention the port and suggest checking for stuck processes

#### Scenario: BrokerError exit code
- **GIVEN** any `BrokerError` variant wrapped in `PawError`
- **WHEN** `exit_code()` is called
- **THEN** it SHALL return `1`

