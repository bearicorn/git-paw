## ADDED Requirements

### Requirement: Per-CLI names are compared only inside the agent CLI module

Production code outside `src/agent_cli/` SHALL NOT compare, match or test a value against a known CLI name (`claude`, `codex`, `agy`, `qwen`, `cursor-agent`). Per-CLI behaviour SHALL be reached through the CLI's resolved record, such as its family. A source audit SHALL enforce this.

#### Scenario: A CLI-name comparison outside the module is caught

- **GIVEN** the line `if cli == "codex" {` in production code outside `src/agent_cli/`
- **WHEN** the CLI-name audit runs
- **THEN** it SHALL report that line

Test: `tests/source_audit.rs::cli_name_audit_catches_each_comparison_form`

#### Scenario: A CLI-name match arm outside the module is caught

- **GIVEN** a production `match` arm `"claude" => …` or `("claude", _) => …` outside `src/agent_cli/`
- **WHEN** the CLI-name audit runs
- **THEN** it SHALL report that line

Test: `tests/source_audit.rs::cli_name_audit_catches_each_comparison_form`

#### Scenario: A wrapped match or a prefix test is caught

- **GIVEN** a production line outside `src/agent_cli/` with `Some("claude") =>`, `(_, "codex") =>` or `.starts_with("claude")`
- **WHEN** the CLI-name audit runs
- **THEN** it SHALL report that line

Test: `tests/source_audit.rs::cli_name_audit_catches_each_comparison_form`

#### Scenario: Comparisons inside the module and in tests pass

- **GIVEN** CLI-name comparisons only inside `src/agent_cli/` or inside `#[cfg(test)]` items
- **WHEN** the CLI-name audit runs
- **THEN** it SHALL report nothing

Test: `tests/source_audit.rs::a_cli_name_comparison_in_a_test_item_is_not_a_finding`, `tests/source_audit.rs::cli_names_are_compared_only_inside_agent_cli`

#### Scenario: The current tree has no finding outside the module

- **WHEN** the CLI-name audit runs over the current tree
- **THEN** no production file outside `src/agent_cli/` SHALL have a finding

Test: `tests/source_audit.rs::cli_names_are_compared_only_inside_agent_cli`
