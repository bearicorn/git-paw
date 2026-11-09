## ADDED Requirements

### Requirement: Per-CLI names are compared only inside the agent CLI module

Production code outside `src/agent_cli/` SHALL NOT compare, match or test a value against a known CLI name (`claude`, `codex`, `agy`, `qwen`, `cursor-agent`). Per-CLI behaviour SHALL be reached through the CLI's resolved record. A source audit SHALL enforce this, with an explicit allowlist for known pre-existing sites.

#### Scenario: A CLI-name comparison outside the module is caught

- **GIVEN** the line `if cli == "codex" {` in production code outside `src/agent_cli/`
- **WHEN** the CLI-name audit runs
- **THEN** it SHALL report that line

#### Scenario: A CLI-name match arm outside the module is caught

- **GIVEN** a production `match` arm `"claude" => …` or `("claude", _) => …` outside `src/agent_cli/`
- **WHEN** the CLI-name audit runs
- **THEN** it SHALL report that line

#### Scenario: Comparisons inside the module and in tests pass

- **GIVEN** CLI-name comparisons only inside `src/agent_cli/` or inside `#[cfg(test)]` items
- **WHEN** the CLI-name audit runs
- **THEN** it SHALL report nothing

#### Scenario: The allowlisted doctor site passes

- **WHEN** the CLI-name audit runs over the current tree
- **THEN** the `src/doctor.rs` workflow-definition table SHALL pass only through its allowlist entry
- **AND** every other production file outside `src/agent_cli/` SHALL have no finding
