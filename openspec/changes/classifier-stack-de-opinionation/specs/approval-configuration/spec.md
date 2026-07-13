## MODIFIED Requirements

### Requirement: Configurable safe-command list

The `safe_commands` field SHALL be a list of strings that are appended to the composed whitelist defaults (the stack-neutral built-ins plus the resolved dev-allowlist patterns, per `safe-command-classification`).

#### Scenario: Custom command added

- **GIVEN** `safe_commands = ["just smoke"]` in config
- **WHEN** classification runs against the command `just smoke -v`
- **THEN** `is_safe_command(...)` SHALL return `true`

#### Scenario: Empty list keeps defaults

- **GIVEN** `safe_commands = []`
- **WHEN** classification runs against `grep -rn "foo" src/`
- **THEN** `is_safe_command(...)` SHALL still return `true` (the composed defaults apply unchanged)
