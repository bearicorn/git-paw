## MODIFIED Requirements

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

## ADDED Requirements

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
