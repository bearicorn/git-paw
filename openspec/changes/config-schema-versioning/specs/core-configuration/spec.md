## ADDED Requirements

### Requirement: Config files declare a schema version

Every `config.toml` git-paw reads (the global `config.toml` and the repo `.git-paw/config.toml`) SHALL accept an optional top-level integer key `version` naming the schema version of the file's layout. A file without `version` SHALL be treated as version 1, and version 1 SHALL be exactly the layout git-paw accepted before this key existed. The binary SHALL define one current schema version (`CURRENT_CONFIG_VERSION`, 1 at introduction), and that value SHALL change only for an incompatible layout change — a renamed, moved, or re-interpreted key. Adding an optional key with a default SHALL NOT change the version.

#### Scenario: An unversioned config loads exactly as before

- **GIVEN** a `.git-paw/config.toml` written for any release from v0.2.0 onward, with no `version` key
- **WHEN** the config is loaded
- **THEN** loading SHALL succeed
- **AND** the resulting configuration SHALL equal the configuration the same file produced before this change

#### Scenario: An explicit version 1 is equivalent to no version

- **GIVEN** two config files identical except that one declares `version = 1`
- **WHEN** both are loaded
- **THEN** they SHALL produce equal configurations

#### Scenario: The version applies to the global config too

- **GIVEN** a global `config.toml` that declares `version = 1` and a repo config that declares none
- **WHEN** the merged configuration is loaded
- **THEN** loading SHALL succeed and the merge result SHALL be unchanged from before this change

### Requirement: Older config versions migrate in memory on load

The system SHALL load a config file by parsing it into a generic TOML document, reading its declared version, applying each registered migration step from that version up to `CURRENT_CONFIG_VERSION` in order, and only then deserializing the result into the configuration types. Each migration step SHALL be a pure function from a document to a document plus human-readable notes, with no I/O. Migration SHALL happen in memory only: loading a config SHALL NOT write, rename, or touch the file on disk. Any notes a migration produces SHALL be printed to stderr prefixed with the file path. At version 1 the migration chain SHALL be empty.

#### Scenario: Loading never rewrites the file

- **GIVEN** an unversioned `.git-paw/config.toml`
- **WHEN** the config is loaded
- **THEN** the file's bytes and modification time SHALL be unchanged

#### Scenario: A registered step runs for an older document

- **GIVEN** a migration chain containing a step from version 1 to version 2 that renames a key
- **WHEN** a version-1 document is migrated to version 2
- **THEN** the step SHALL run exactly once
- **AND** the migrated document SHALL carry the renamed key and declare version 2
- **AND** the step's notes SHALL be returned for printing

#### Scenario: A current-version document passes through unchanged

- **WHEN** a document that already declares `CURRENT_CONFIG_VERSION` is migrated
- **THEN** no step SHALL run and the document SHALL be returned unchanged with no notes

### Requirement: A config declaring an unsupported schema version is refused

When a config file declares a `version` greater than `CURRENT_CONFIG_VERSION`, loading SHALL fail with a configuration error that names the file, the declared version, and the highest supported version, and tells the user to upgrade git-paw. A `version` that is zero, negative, or not an integer SHALL fail loading with a configuration error naming the file and the invalid value. For a supported version, unknown keys SHALL continue to be ignored by loading.

#### Scenario: A newer config is refused with an upgrade hint

- **GIVEN** a `.git-paw/config.toml` declaring `version = 2` and a binary whose current version is 1
- **WHEN** any command loads the config
- **THEN** the command SHALL fail with a configuration error
- **AND** the message SHALL name the file, version 2, supported version 1, and say to upgrade git-paw

#### Scenario: An invalid version value is refused

- **GIVEN** a config file declaring `version = "one"` or `version = 0`
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with a configuration error naming the file and the invalid value

#### Scenario: Unknown keys in a supported version are still tolerated

- **GIVEN** a `version = 1` config that also contains a key git-paw does not recognise
- **WHEN** the config is loaded
- **THEN** loading SHALL succeed and the unknown key SHALL be ignored

### Requirement: The config file format is a wire type separate from the internal model

The serialized form of `config.toml` SHALL be defined by dedicated wire types, separate from the in-memory configuration model (`PawConfig` and its section types). The system SHALL map a wire value to the model after migration, and the model back to a wire value when writing. The in-memory model SHALL NOT derive `Serialize` or `Deserialize`. The wire types SHALL reproduce the existing TOML layout byte for byte, including every key name, every default and omission rule, the `[specs]` `type` key, and the global-over-repo merge semantics in which an absent section or field means "unset". Merging SHALL operate on the mapped model after each file has been migrated on its own.

#### Scenario: Wire and model round-trip

- **GIVEN** a fully populated configuration model
- **WHEN** it is mapped to the wire type, serialized, parsed, and mapped back
- **THEN** the result SHALL equal the original model

#### Scenario: The specs type key keeps its spelling

- **GIVEN** a config containing `[specs]` with `type = "openspec"`
- **WHEN** it is loaded and written back by git-paw
- **THEN** the written file SHALL still spell the key `type`

#### Scenario: Merge semantics are unchanged

- **GIVEN** a global config and a repo config that both set overlapping scalars, sections, and `[clis.*]` entries
- **WHEN** the merged configuration is loaded
- **THEN** the result SHALL equal the result of the pre-change merge for the same two files

### Requirement: Configs written by git-paw declare the current schema version

Whenever git-paw serializes a configuration to disk (for example `git paw add-cli` and `git paw remove-cli` rewriting the global config), the written file SHALL declare `version` equal to `CURRENT_CONFIG_VERSION` as its first top-level key. All other content SHALL be the same as the pre-change serialization of the same configuration.

#### Scenario: add-cli stamps an unversioned global config

- **GIVEN** a global `config.toml` with one custom CLI and no `version` key
- **WHEN** `git paw add-cli my-agent /usr/local/bin/my-agent` is run
- **THEN** the rewritten file SHALL declare `version = 1`
- **AND** both the existing CLI and `my-agent` SHALL be present

#### Scenario: Serialized output differs only by the version line

- **GIVEN** a configuration that serialized to a known TOML document before this change
- **WHEN** it is serialized after this change
- **THEN** the output SHALL equal the previous document with a leading `version = 1` line added

### Requirement: The default config template declares the current schema version

The default config produced by `generate_default_config()` (used by `git paw init`) SHALL contain an active, uncommented top-level `version` line equal to `CURRENT_CONFIG_VERSION`, placed before the first section header. Through `init`'s additive top-level-key backfill, running `git paw init` on an existing config without `version` SHALL add the line and SHALL leave every other key and section unchanged. A config that already declares `version` SHALL NOT have it modified by `init`.

#### Scenario: The template declares the version

- **WHEN** the default config string is generated
- **THEN** it SHALL contain the active line `version = 1` before the first `[section]` header

#### Scenario: init backfills the version into an existing config

- **GIVEN** an existing `.git-paw/config.toml` with customized sections and no `version` key
- **WHEN** `git paw init` is run
- **THEN** the file SHALL gain a top-level `version = 1`
- **AND** every pre-existing section and key SHALL be unchanged

#### Scenario: init leaves a declared version alone

- **GIVEN** an existing config that already declares `version = 1`
- **WHEN** `git paw init` is run
- **THEN** the `version` line SHALL be unchanged

### Requirement: Script-read configuration keys are a documented contract

The configuration reference SHALL list every config key that the bundled helper scripts read directly from `config.toml` — at introduction `[broker] port`, `[broker] bind`, top-level `docs_base_url`, `[supervisor] test_command`, `no_progress_window_seconds`, `context_bloat_threshold_k`, and `blocked_on_supervisor_window_seconds` — and SHALL state that renaming or moving any of them requires a schema version bump and an updated script in the same release. A standing test SHALL fail when a bundled script reads a config key that the reference does not list.

#### Scenario: Every script-read key is documented

- **GIVEN** the bundled scripts under `assets/scripts/`
- **WHEN** the config keys they read are collected
- **THEN** each key SHALL appear in the configuration reference's script-read key list

#### Scenario: Scripts keep resolving their keys from a versioned config

- **GIVEN** a `.git-paw/config.toml` declaring `version = 1` and `[broker] port = 9200`
- **WHEN** the bundled broker-URL discovery runs, both with a TOML parser available and through its regex fallback
- **THEN** it SHALL resolve port 9200
