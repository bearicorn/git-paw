## Purpose

Parse TOML configuration from global (`~/.config/git-paw/config.toml`) and per-repo (`.git-paw/config.toml`) files. Supports custom CLI definitions, presets, and programmatic add/remove of custom CLIs with repo config overriding global config.
## Requirements
### Requirement: Parse TOML config with all fields

The system SHALL parse a TOML configuration file containing `default_cli`, `mouse`, `clis`, `presets`, and optional sections `[specs]`, `[logging]`, `[broker]`, and `[supervisor]`.

#### Scenario: Config with all fields populated
- **GIVEN** a TOML file with `default_cli`, `mouse`, custom CLIs, presets, `[broker]`, and `[supervisor]` sections
- **WHEN** the file is loaded
- **THEN** all fields SHALL be correctly parsed including supervisor fields

#### Scenario: All fields are optional
- **GIVEN** a TOML file with only `default_cli`
- **WHEN** the file is loaded
- **THEN** missing fields SHALL default to `None` or empty collections
- **AND** `supervisor` SHALL be `None`

### Requirement: Merge repo config over global config

The system SHALL merge per-repo configuration on top of global configuration, with repo values taking precedence for scalar fields and map entries.

#### Scenario: Repo overrides global scalar fields
- **GIVEN** global config has `default_cli = "claude"` and `mouse = true`, and repo has `default_cli = "gemini"`
- **WHEN** configs are merged
- **THEN** `default_cli` SHALL be `"gemini"` and `mouse` SHALL be `true` (preserved from global)

Test: `config::tests::repo_config_overrides_global_scalars`

#### Scenario: CLI maps are merged
- **GIVEN** global config has CLI `agent-a` and repo config has CLI `agent-b`
- **WHEN** configs are merged
- **THEN** both CLIs SHALL be present

Test: `config::tests::repo_config_merges_cli_maps`

#### Scenario: Repo CLI overrides global CLI with same name
- **GIVEN** both global and repo define a CLI named `my-agent`
- **WHEN** configs are merged
- **THEN** the repo definition SHALL win

Test: `config::tests::repo_cli_overrides_global_cli_with_same_name`

#### Scenario: Only global config exists
- **GIVEN** a global config file but no repo config
- **WHEN** `load_config()` is called
- **THEN** global values SHALL be used

Test: `config::tests::load_config_from_reads_global_file_when_no_repo`

#### Scenario: Only repo config exists
- **GIVEN** a repo config file but no global config
- **WHEN** `load_config()` is called
- **THEN** repo values SHALL be used

Test: `config::tests::load_config_from_reads_repo_file_when_no_global`

### Requirement: Preset lookup by name

The system SHALL provide access to named presets that define branches and a CLI.

#### Scenario: Preset accessible by name
- **GIVEN** a config with a preset named `"backend"`
- **WHEN** `get_preset("backend")` is called
- **THEN** it SHALL return the preset with its branches and CLI

Test: `config::tests::preset_accessible_by_name`

#### Scenario: Missing preset returns None
- **GIVEN** a config without the requested preset
- **WHEN** `get_preset("nonexistent")` is called
- **THEN** it SHALL return `None`

Test: `config::tests::preset_returns_none_when_not_in_config`

### Requirement: Add custom CLIs to global config

The system SHALL add custom CLI definitions to the global config, resolving non-absolute commands via PATH.

#### Scenario: Add CLI with absolute path
- **GIVEN** an absolute path to a CLI binary
- **WHEN** `add_custom_cli()` is called
- **THEN** the CLI SHALL be written to the config file

Test: `config::tests::add_cli_writes_to_config_file`

#### Scenario: Adding preserves existing entries
- **GIVEN** an existing CLI in the config
- **WHEN** a second CLI is added
- **THEN** both CLIs SHALL be present

Test: `config::tests::add_cli_preserves_existing_entries`

#### Scenario: Adding CLI with missing command fails
- **GIVEN** a command that does not exist on PATH
- **WHEN** `add_custom_cli()` is called
- **THEN** it SHALL return an error mentioning "not found on PATH"

Test: `config::tests::add_cli_errors_when_command_not_on_path`

### Requirement: Remove custom CLIs from global config

The system SHALL remove a custom CLI by name, returning an error if the CLI is not found.

#### Scenario: Remove existing CLI
- **GIVEN** a config with CLIs `keep-me` and `remove-me`
- **WHEN** `remove_custom_cli("remove-me")` is called
- **THEN** only `keep-me` SHALL remain

Test: `config::tests::remove_cli_deletes_entry_from_config_file`

#### Scenario: Remove nonexistent CLI returns error
- **GIVEN** a config without the named CLI
- **WHEN** `remove_custom_cli()` is called
- **THEN** it SHALL return `PawError::CliNotFound`

Test: `config::tests::remove_nonexistent_cli_returns_cli_not_found_error`

#### Scenario: Remove CLI from empty/missing config returns error
- **GIVEN** no config file exists
- **WHEN** `remove_custom_cli()` is called
- **THEN** it SHALL return `PawError::CliNotFound`

Test: `config::tests::remove_cli_from_empty_config_returns_error`

### Requirement: Config survives round-trip serialization

A `PawConfig` SHALL be identical after save and reload.

#### Scenario: Config round-trip
- **GIVEN** a fully populated config
- **WHEN** saved and loaded back
- **THEN** it SHALL be equal to the original

Test: `config::tests::config_survives_save_and_load`

### Requirement: Config loading SHALL work with real files

The system SHALL provide a `load_config(repo_root, user_config_path)` function that loads the merged `PawConfig` from the per-repo `.git-paw/config.toml` and a user-level (global) `config.toml`. The second parameter `user_config_path: Option<&Path>` SHALL control which file is read as the user-level config:

- When `user_config_path` is `None`, the loader SHALL resolve the user-level config path via the platform-default helper (`global_config_path()` → `crate::dirs::config_dir().join("git-paw/config.toml")`), preserving the v0.4 production behaviour.
- When `user_config_path` is `Some(p)`, the loader SHALL read `p` as the user-level config and SHALL NOT consult the platform-default helper. If `p` does not exist on disk, the user-level side of the merge SHALL be the default `PawConfig`, exactly as if no file existed at the platform-default path.

The merge semantics on top of the user-level config (per-repo config overrides user-level for scalar fields and map entries) are unchanged from prior requirements in this capability.

#### Scenario: Defaults when no files exist

- **GIVEN** a temp directory with no config files
- **AND** `load_config` is called with `user_config_path = None`
- **WHEN** `load_config()` is called
- **THEN** all fields SHALL be None/empty

Test: `config_integration::load_config_returns_defaults_when_no_files_exist`

#### Scenario: Reads repo .git-paw/config.toml

- **GIVEN** a `.git-paw/config.toml` with default_cli and mouse
- **AND** `load_config` is called with `user_config_path = Some(&unused_temp_path)`
- **WHEN** `load_config()` is called
- **THEN** the values SHALL be read correctly

Test: `config_integration::load_config_reads_repo_config`

#### Scenario: Repo config with custom CLIs

- **GIVEN** a `.git-paw/config.toml` with two custom CLIs
- **AND** `load_config` is called with `user_config_path = Some(&unused_temp_path)`
- **WHEN** `load_config()` is called
- **THEN** both CLIs SHALL be parsed with correct fields

Test: `config_integration::repo_config_with_custom_clis`

#### Scenario: Repo config with presets

- **GIVEN** a `.git-paw/config.toml` with two presets
- **AND** `load_config` is called with `user_config_path = Some(&unused_temp_path)`
- **WHEN** `load_config()` is called
- **THEN** presets SHALL be accessible with correct branches and CLI

Test: `config_integration::repo_config_with_presets`

#### Scenario: Default PawConfig has no presets

- **GIVEN** a default `PawConfig`
- **WHEN** `get_preset("nonexistent")` is called
- **THEN** it SHALL return `None`

Test: `config_integration::get_preset_returns_none_for_unknown`

#### Scenario: Repo config overrides default fields

- **GIVEN** a `.git-paw/config.toml` with specific values
- **AND** `load_config` is called with `user_config_path = Some(&unused_temp_path)`
- **WHEN** `load_config()` is called
- **THEN** the repo values SHALL take precedence

Test: `config_integration::repo_config_overrides_default_fields`

#### Scenario: Repo config path is correct

- **GIVEN** a temp directory
- **WHEN** `repo_config_path()` is called
- **THEN** it SHALL return `<dir>/.git-paw/config.toml`

Test: `config_integration::repo_config_path_is_in_repo_root`

#### Scenario: Malformed TOML returns error

- **GIVEN** a `.git-paw/config.toml` with invalid TOML
- **AND** `load_config` is called with `user_config_path = Some(&unused_temp_path)`
- **WHEN** `load_config()` is called
- **THEN** it SHALL return an error

Test: `config_integration::malformed_toml_returns_error`

#### Scenario: Empty config file is valid

- **GIVEN** an empty `.git-paw/config.toml`
- **AND** `load_config` is called with `user_config_path = Some(&unused_temp_path)`
- **WHEN** `load_config()` is called
- **THEN** it SHALL return a default config

Test: `config_integration::empty_config_file_is_valid`

#### Scenario: None preserves platform-default user-config resolution

- **GIVEN** a repo `TempDir` with no `.git-paw/config.toml`
- **AND** the platform-default user config path (`crate::dirs::config_dir().join("git-paw/config.toml")`) is a readable file containing a custom CLI named `globally-registered`
- **WHEN** `load_config(&repo, None)` is called
- **THEN** the returned `PawConfig.clis` SHALL contain `globally-registered`
- **AND** the loader SHALL have resolved the user-level path via `global_config_path()`, exactly matching v0.4 behaviour

Test: `config::tests::load_config_with_none_reads_platform_default_global`

#### Scenario: Some(path) pins the user-level read to that path

- **GIVEN** a `TempDir` containing two distinct files:
  - `tmp/global-A.toml` defining custom CLI `cli-A`
  - `tmp/global-B.toml` defining custom CLI `cli-B`
- **AND** an unrelated CLI `cli-C` is registered at the platform-default user-config path
- **WHEN** `load_config(&repo, Some(&tmp.join("global-A.toml")))` is called
- **THEN** the returned `PawConfig.clis` SHALL contain `cli-A`
- **AND** it SHALL NOT contain `cli-B` or `cli-C`

Test: `config::tests::load_config_with_some_pins_global_to_override_path`

#### Scenario: Some(nonexistent path) returns defaults for the user-level side

- **GIVEN** a `TempDir` and a path `tmp/does-not-exist.toml` that has never been written
- **AND** an unrelated CLI `cli-leak` is registered at the platform-default user-config path
- **WHEN** `load_config(&repo, Some(&tmp.join("does-not-exist.toml")))` is called
- **THEN** the user-level side of the merge SHALL be the default `PawConfig`
- **AND** the returned `PawConfig.clis` SHALL NOT contain `cli-leak`
- **AND** no error SHALL be returned (a missing user-config file is not an error)

Test: `config::tests::load_config_with_some_nonexistent_returns_defaults`

#### Scenario: Override path does not affect repo-config resolution

- **GIVEN** a `TempDir` with `.git-paw/config.toml` defining `default_cli = "claude"`
- **AND** a separate path `tmp/global.toml` defining `default_cli = "gemini"`
- **WHEN** `load_config(&tmp, Some(&tmp.join("global.toml")))` is called
- **THEN** the repo-level `default_cli = "claude"` SHALL override the user-level `default_cli = "gemini"` per the existing repo-overrides-user merge semantics
- **AND** the override parameter SHALL only control which user-level file is read, never the repo-level resolution

Test: `config::tests::load_config_override_does_not_affect_repo_resolution`

### Requirement: Custom CLI management SHALL persist through file I/O

#### Scenario: Add CLI with absolute path
- **GIVEN** no config file
- **WHEN** `add_custom_cli_to()` is called with an absolute path
- **THEN** the CLI SHALL be persisted and reloadable

Test: `config_integration::add_custom_cli_with_absolute_path`

#### Scenario: Add CLI with display name
- **GIVEN** no config file
- **WHEN** `add_custom_cli_to()` is called with a display name
- **THEN** the display name SHALL be persisted

Test: `config_integration::add_custom_cli_with_display_name`

#### Scenario: Multiple CLIs preserved across adds
- **GIVEN** 4 CLIs added sequentially
- **WHEN** the config is loaded
- **THEN** all 4 SHALL be present with correct fields

Test: `config_integration::add_multiple_custom_clis_preserves_all`

#### Scenario: Adding overwrites existing entry
- **GIVEN** a CLI with name `my-agent` already exists
- **WHEN** `add_custom_cli_to()` is called with the same name but different values
- **THEN** the new values SHALL replace the old

Test: `config_integration::add_cli_overwrites_existing_entry`

#### Scenario: Add CLI with nonexistent command fails
- **GIVEN** a non-absolute command that is not on PATH
- **WHEN** `add_custom_cli_to()` is called
- **THEN** it SHALL return an error

Test: `config_integration::add_cli_with_nonexistent_path_command_fails`

#### Scenario: Remove custom CLI
- **GIVEN** two CLIs in the config
- **WHEN** one is removed
- **THEN** only the other SHALL remain

Test: `config_integration::remove_custom_cli`

#### Scenario: Remove nonexistent CLI returns error
- **GIVEN** no CLIs in the config
- **WHEN** `remove_custom_cli_from()` is called
- **THEN** it SHALL return an error

Test: `config_integration::remove_nonexistent_cli_returns_error`

#### Scenario: Remove all CLIs leaves empty config
- **GIVEN** one CLI in the config
- **WHEN** it is removed
- **THEN** the CLI map SHALL be empty

Test: `config_integration::remove_all_custom_clis_leaves_empty_config`

### Requirement: Global and repo config SHALL merge custom CLIs correctly

#### Scenario: Repo custom CLIs merge with global
- **GIVEN** global config with 2 CLIs and repo config with 2 CLIs (one overlapping)
- **WHEN** `load_config_from()` is called
- **THEN** the result SHALL have 3 CLIs, with repo winning on collision

Test: `config_integration::repo_custom_clis_merge_with_global_custom_clis`

### Requirement: Config SHALL handle many custom CLIs

#### Scenario: Config with 10 custom CLIs
- **GIVEN** a config file with 10 custom CLI definitions
- **WHEN** `load_config()` is called
- **THEN** all 10 SHALL be parsed correctly

Test: `config_integration::config_with_many_custom_clis`

### Requirement: The system SHALL support a default_spec_cli config field

The system SHALL support a `default_spec_cli` field in `PawConfig` that specifies the CLI to use for `--from-specs` branches that don't have a `paw_cli` override, bypassing the interactive picker.

#### Scenario: default_spec_cli set
- **WHEN** a config has `default_spec_cli = "claude"`
- **THEN** `PawConfig.default_spec_cli` SHALL be `Some("claude")`

#### Scenario: default_spec_cli absent
- **WHEN** a config has no `default_spec_cli` field
- **THEN** `PawConfig.default_spec_cli` SHALL be `None`

#### Scenario: Merge preserves repo override
- **WHEN** global config has `default_spec_cli = "claude"` and repo config has `default_spec_cli = "gemini"`
- **THEN** the merged config SHALL have `default_spec_cli = Some("gemini")`

### Requirement: Repo SHALL override new v0.2.0 scalar fields

#### Scenario: Repo overrides new v0.2.0 scalar fields
- **GIVEN** global config has `default_spec_cli = "claude"` and repo has `default_spec_cli = "gemini"`
- **WHEN** configs are merged
- **THEN** `default_spec_cli` SHALL be `"gemini"`

### Requirement: Specs configuration section

The system SHALL support an optional `[specs]` section with a `dir` field and a `type` field. Field names SHALL match the `spec-scanning` capability and the implementation in `src/config.rs::SpecsConfig`.

- `dir: String` — path (relative to the repo root) to the directory containing spec files
- `type: String` — backend identifier (e.g. `"openspec"`, `"markdown"`); the field is exposed as `spec_type` in Rust to avoid clashing with the `type` keyword and is serialised as `type` in TOML/JSON via `#[serde(rename = "type")]`

When the `[specs]` section is absent, the optional `specs` field on `PawConfig` SHALL be `None`.

#### Scenario: Specs section with all fields

- **GIVEN** a TOML file with `[specs]` containing `dir = "openspec/specs"` and `type = "openspec"`
- **WHEN** the file is loaded
- **THEN** `specs.dir` SHALL be `"openspec/specs"`
- **AND** `specs.spec_type` SHALL be `"openspec"`

#### Scenario: Specs section defaults

- **GIVEN** a TOML file without a `[specs]` section
- **WHEN** the file is loaded
- **THEN** `specs` SHALL be `None`

#### Scenario: Round-trip preserves rename

- **GIVEN** a `SpecsConfig { dir: "openspec/specs".into(), spec_type: "openspec".into() }`
- **WHEN** the value is serialised to TOML and parsed back
- **THEN** the resulting TOML SHALL contain `type = "openspec"` (not `spec_type`)
- **AND** parsing SHALL succeed and reproduce the original struct

### Requirement: Logging configuration section

The system SHALL support an optional `[logging]` section with `enabled` and `log_dir` fields.

#### Scenario: Logging section with all fields
- **GIVEN** a TOML file with `[logging]` containing `enabled = true` and `log_dir = ".git-paw/logs"`
- **WHEN** the file is loaded
- **THEN** `logging.enabled` SHALL be `true` and `logging.log_dir` SHALL be `".git-paw/logs"`

#### Scenario: Logging section defaults
- **GIVEN** a TOML file without a `[logging]` section
- **WHEN** the file is loaded
- **THEN** `logging` SHALL be `None`

### Requirement: Default config generation

The system SHALL provide a function to generate a default `config.toml` string with active defaults and commented-out fields including the `[supervisor]` section.

#### Scenario: Generated config contains commented supervisor examples
- **WHEN** the default config string is generated
- **THEN** it SHALL contain commented-out examples for `[supervisor]` with `enabled`, `cli`, `test_command`, and `agent_approval` fields

#### Scenario: Generated config contains commented examples
- **WHEN** the default config string is generated
- **THEN** it SHALL contain commented-out examples for `default_spec_cli`, `branch_prefix`, `[specs]`, `[logging]`, `[broker]`, and `[supervisor]`

### Requirement: Config round-trip with new fields

A `PawConfig` with v0.2.0 fields populated SHALL be identical after save and reload.

#### Scenario: Config with specs and logging round-trips
- **GIVEN** a config with `default_spec_cli`, `branch_prefix`, `specs`, and `logging` populated
- **WHEN** saved and loaded back
- **THEN** it SHALL be equal to the original

### Requirement: Broker configuration section

The system SHALL support an optional `[broker]` section with the following fields:

- `enabled: bool` — defaults to `false` when the field or section is absent
- `port: u16` — defaults to `9119` when absent
- `bind: String` — defaults to `"127.0.0.1"` when absent

The `BrokerConfig` struct SHALL provide a `url(&self) -> String` method returning `http://<bind>:<port>`.

#### Scenario: Broker section with all fields
- **GIVEN** a TOML file with `[broker]` containing `enabled = true`, `port = 9200`, `bind = "127.0.0.1"`
- **WHEN** the file is loaded
- **THEN** `broker.enabled` SHALL be `true`, `broker.port` SHALL be `9200`, `broker.bind` SHALL be `"127.0.0.1"`

#### Scenario: Broker section defaults
- **GIVEN** a TOML file without a `[broker]` section
- **WHEN** the file is loaded
- **THEN** `broker` SHALL have `enabled = false`, `port = 9119`, `bind = "127.0.0.1"`

#### Scenario: Partial broker section
- **GIVEN** a TOML file with `[broker]` containing only `enabled = true`
- **WHEN** the file is loaded
- **THEN** `broker.enabled` SHALL be `true`, `broker.port` SHALL be `9119`, `broker.bind` SHALL be `"127.0.0.1"`

#### Scenario: BrokerConfig url method
- **GIVEN** `BrokerConfig { enabled: true, port: 9200, bind: "127.0.0.1" }`
- **WHEN** `url()` is called
- **THEN** the result SHALL be `"http://127.0.0.1:9200"`

#### Scenario: Broker config round-trips through save and load
- **GIVEN** a config with `[broker]` fully populated
- **WHEN** saved and loaded back
- **THEN** all broker fields SHALL match the original

### Requirement: Internal callers SHALL preserve v0.4 behaviour by passing None

All production call sites of `load_config` inside the git-paw binary SHALL pass `None` as the `user_config_path` argument, so production behaviour is byte-identical to the v0.4 single-argument `load_config(repo_root)` API.

The `Option<&Path>` argument SHALL exist only to give test code a discoverable way to isolate the user-level config read from whatever exists at the dev machine's platform-default path. No production code path SHALL pass `Some(_)`.

#### Scenario: All production call sites pass None

- **GIVEN** the v0.5.0 source tree
- **WHEN** every call site of `config::load_config` inside `src/` is inspected
- **THEN** every call SHALL be of the form `config::load_config(&repo_root, None)` (modulo whitespace and the exact name of the `repo_root` binding)
- **AND** no production call site SHALL pass `Some(_)`

Test: covered by `cargo build` (compile-time) plus a focused grep-style assertion in `src/main.rs::tests` or equivalent — see tasks.md task 2.

#### Scenario: Production behaviour is byte-identical to v0.4

- **GIVEN** a v0.5.0 binary built from this change
- **AND** the same `.git-paw/config.toml` and platform-default user config that a v0.4 binary would read
- **WHEN** any production command that calls `load_config` runs (e.g. `git paw start`, `git paw add-cli`, `git paw dashboard`)
- **THEN** the merged `PawConfig` the command operates on SHALL be equal to the merged `PawConfig` v0.4 would have produced

Test: behaviour preserved by construction (every production call passes `None`); verified by the v0.4 test suite continuing to pass unchanged plus the new `load_config_with_none_reads_platform_default_global` unit test.

