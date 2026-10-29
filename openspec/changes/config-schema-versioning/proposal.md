## Why

v1.0.0 freezes `config.toml` as a contract, but the file has no version and is deserialized
straight into the in-memory model (`toml::from_str` into `PawConfig` in
`config::load_config_file`). After the freeze the only way to evolve it would be additive
`#[serde(default)]` fields: a rename, a move or a changed meaning would either break old files or
be silently misread, because unknown keys are ignored. The bundled scripts also read config keys
directly (`[broker].port`/`bind`, `docs_base_url`, `[supervisor].test_command` and three
`[supervisor]` integers, via Python `tomllib`), so those names are contracts too. A version field
and a migration step have to exist before the freeze, or the format can never change after it.

## What Changes

- `config.toml` (repo and global) gains an optional top-level integer `version`. A file without
  it is read as version 1, which is today's format, so every existing config loads unchanged.
- Loading becomes read → parse to a generic TOML document → migrate to the current version →
  map into the internal model. The migration chain is empty at version 1. It exists so that
  later versions can rename or move keys without breaking older files. Migration runs in memory.
  git-paw never rewrites a user's file to migrate it.
- A file whose `version` is newer than the binary supports, or is not a positive integer, fails
  to load with a `ConfigError` that names the file and both versions and says to upgrade
  git-paw. Unknown keys are still ignored for known versions.
- A separate wire type (`config::wire`) is the serde surface of the file. `PawConfig` becomes the
  internal model, filled by a mapper. The TOML layout and the frozen serde surfaces
  (`SpecsConfig` `rename = "type"`, `merged_with`'s default-as-unset semantics) stay
  byte-identical.
- The `git paw init` default config template declares `version = 1` as an active key, so `init`
  adds it to existing configs through the existing additive top-level-key backfill.
  `add-cli`/`remove-cli`, which rewrite the global config through serde, write `version = 1`.
- `git paw doctor`'s Config group reports the schema version, and fails (✗) on a config that is
  too new.
- The configuration reference documents `version`, the compatibility rule, and the script-read
  keys as part of the contract.

No breaking change: every config that loads today loads the same way, with the same result.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `core-configuration`: ADDED requirements for the schema version field, the in-memory migration
  chain, refusing newer schemas, the wire-type/model split, versioned writes, the default
  template declaring the version, and the script-read keys as a documented contract.
- `preflight-diagnostics`: ADDED requirement that the Config group reports the schema version
  and fails on an unsupported one.

## Impact

- **Code:** `src/config/mod.rs` (`load_config_file`, `save_config_to`, `generate_default_config`),
  a new `src/config/wire.rs` (the file's serde types plus a mapper to and from `PawConfig`), a new
  `src/config/migrate.rs` (`CURRENT_CONFIG_VERSION`, the migration chain), `src/config/*.rs`
  section structs (their serde derives move to the wire types), `src/config/cli.rs`
  (`add_custom_cli`/`remove_custom_cli` write through the wire type), `src/doctor.rs`
  (`check_config`, and `unknown_config_keys` treats `version` as a known key), and
  `src/init.rs` (no code change: `migrate_existing_config` already backfills a new template
  key).
- **Scripts:** none change. `assets/scripts/_paw_common.sh` and `sweep.sh` ignore the new
  top-level key, and their regex fallbacks key on section headers.
- **Tests:** `src/config/tests.rs` and `tests/config_integration.rs` (golden v1 fixtures load
  unchanged, a version-2 file is refused, mapper round-trip), `src/doctor_tests.rs`, and the
  init backfill test.
- **Docs:** `docs/src/configuration/README.md` (a `version` entry and a "Compatibility and
  versioning" section that lists the script-read keys), and the `git paw init` template comment.
- **Enum-variant ripple:** none. `BrokerMessage` and `SpecBackendKind` are untouched.
- **Dependencies:** none. `toml::Value` is already available.
- **Overlap:** the other v0.21.0 track's `git paw config check` (validate/normalise) should
  report the version through the same `config::migrate` API instead of re-implementing it, and
  `--json` for `doctor` belongs to `uniform-json-output`.
- **Cross-version dependencies:**
  - Builds on the archived v0.17.0 `test-session-state-isolation`. Its HOME/XDG override rule keeps the
    new global-config tests away from the developer's real files.
  - Must rebase over v0.18 `cmd-library-surface`, which moves `commands/` into the library and
    passes `&PawConfig` explicitly. The mapper output is what those functions receive.
  - Its `core-configuration` and `preflight-diagnostics` deltas only add requirements, so they do
    not depend on another change's text.
  - `wire-format-dtos` (this release) follows the same DTO + version pattern for the other four
    formats.
