## ADDED Requirements

### Requirement: Per-CLI MCP capability override

The system SHALL decide whether a target CLI "speaks MCP" (and so receives the MCP tool form of the boot block rather than the helper-script form) from the CLI identity, consulting in order: (1) an explicit `mcp = true` or `mcp = false` on the matching `[clis.<name>]` entry; (2) the built-in MCP-capable set, which SHALL be exactly `claude`, `codex`, `cursor-agent`, and `agy`; (3) otherwise not MCP-capable, so the helper-script form is used. The lookup SHALL use the leading whitespace-delimited token of the CLI identity (the `[clis.<name>]` key or built-in / pass-through name), never the resolved launch command, so a custom variant whose `command` is a built-in binary does not inherit that binary's MCP form without opting in.

The `mcp` field SHALL be optional: when unset it SHALL NOT be serialized, and a config without it SHALL load and behave exactly as before this requirement. Every launch path that renders a boot block (bare start, spec-driven start, supervisor pane, and `git paw add`) SHALL apply the same resolution.

#### Scenario: A custom CLI opts in to the MCP form

- **GIVEN** `[clis.claude-oss] command = "claude"` with `mcp = true`
- **WHEN** the boot block is rendered for CLI `claude-oss`
- **THEN** the four events SHALL be expressed as MCP publish tool invocations

Test: `skills::tests::cli_speaks_mcp_honours_config_opt_in`

#### Scenario: A built-in MCP CLI can be opted out

- **GIVEN** `[clis.claude] command = "claude"` with `mcp = false`
- **WHEN** MCP capability is resolved for CLI `claude`
- **THEN** it SHALL resolve to not MCP-capable and the helper-script form SHALL be used

Test: `skills::tests::cli_speaks_mcp_honours_config_opt_out`

#### Scenario: A custom CLI without the field keeps the helper form

- **GIVEN** `[clis.claude-oss] command = "claude"` with no `mcp` field
- **WHEN** MCP capability is resolved for CLI `claude-oss`
- **THEN** it SHALL resolve to not MCP-capable

Test: `skills::tests::cli_speaks_mcp_unset_custom_cli_falls_back_to_helper`

#### Scenario: Built-in MCP-capable set names Cursor's agent CLI

- **GIVEN** no `[clis]` entries
- **WHEN** MCP capability is resolved for `claude`, `codex`, `cursor-agent`, `agy`, `cursor`, and `aider`
- **THEN** the first four SHALL resolve MCP-capable
- **AND** `cursor` and `aider` SHALL resolve not MCP-capable

Test: `skills::tests::cli_speaks_mcp_known_clis_true_unknown_clis_false`

#### Scenario: Unset `mcp` is omitted on serialization

- **GIVEN** a `CustomCli` with `mcp` unset
- **WHEN** the config is serialized to TOML and loaded back
- **THEN** the serialized text SHALL NOT contain an `mcp` key
- **AND** the round-tripped value SHALL equal the original

Test: `config::tests::custom_cli_mcp_field_round_trips_and_is_skipped_when_unset`

#### Scenario: Launch path renders the opted-in form

- **GIVEN** a broker-enabled config with `[clis.paw-stub]` (`mcp = true`) selected for a worker
- **WHEN** the session is launched
- **THEN** the boot prompt delivered to that worker's pane SHALL contain the MCP publish tool form, not the `broker.sh` helper invocations

Test: `tests/cli_launch_identity_e2e.rs::mcp_opt_in_reaches_worker_boot_prompt`
