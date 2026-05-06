# safe-command-classification Specification

## Purpose
TBD - created by archiving change auto-approve-patterns. Update Purpose after archive.
## Requirements
### Requirement: Whitelist of safe command classes

The system SHALL maintain an explicit whitelist of command prefixes that are eligible for auto-approval, and SHALL NOT auto-approve anything outside the whitelist.

#### Scenario: Default whitelist

- **GIVEN** the default supervisor configuration
- **WHEN** `default_safe_commands()` is queried
- **THEN** the result SHALL contain at minimum:
  - `cargo fmt`
  - `cargo clippy`
  - `cargo test`
  - `cargo build`
  - `git commit`
  - `git push`
  - `curl http://127.0.0.1:` (broker localhost)

#### Scenario: Unknown command not in whitelist

- **GIVEN** a captured permission prompt for `rm -rf /tmp/foo`
- **WHEN** the classifier runs
- **THEN** `is_safe_command("rm -rf /tmp/foo", &whitelist)` SHALL return `false`
- **AND** the auto-approver SHALL NOT send approval keystrokes

### Requirement: Configurable whitelist extension

The whitelist SHALL be extendable by user configuration so projects can add their own safe patterns without modifying the binary.

#### Scenario: Config adds project-specific patterns

- **GIVEN** `[supervisor.auto_approve] safe_commands = ["just lint", "just test"]` in `.git-paw/config.toml`
- **WHEN** the supervisor loads its configuration
- **THEN** the effective whitelist SHALL be the union of the defaults and the configured entries

#### Scenario: Config does not weaken defaults

- **GIVEN** a config that omits `safe_commands` or sets it to `[]`
- **WHEN** the supervisor loads its configuration
- **THEN** the default whitelist SHALL still apply

### Requirement: Prefix matching semantics

The classifier SHALL use prefix matching against the captured command text so that flag variations are accepted without per-flag whitelist entries.

#### Scenario: Flag variation matches prefix

- **GIVEN** whitelist entry `cargo test`
- **WHEN** the captured command is `cargo test --no-run --workspace`
- **THEN** `is_safe_command(...)` SHALL return `true`

#### Scenario: Different program does not match

- **GIVEN** whitelist entry `cargo test`
- **WHEN** the captured command is `cargotest --foo` (no space)
- **THEN** `is_safe_command(...)` SHALL return `false`

