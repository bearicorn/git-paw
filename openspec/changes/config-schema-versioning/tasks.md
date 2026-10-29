## 1. Seam — document-first load pipeline (behavior-preserving)

- [ ] 1.1 Capture golden fixtures before any code change: one config per era (v0.2.0, v0.4 broker, v0.5 supervisor, v0.7 mcp, v0.15 worktree, current template) under `tests/fixtures/config/`, each with its expected loaded `PawConfig` (Debug snapshot) and expected `save_config_to` output
- [ ] 1.2 Add `src/config/migrate.rs`: `CURRENT_CONFIG_VERSION = 1`, `MigrationNote`, `declared_version(&toml::Value) -> Result<u32, …>`, and `to_current(value, from, steps) -> (Value, Vec<MigrationNote>)` over an injectable step list (empty in production)
- [ ] 1.3 Route `load_config_file` through `toml::Value` → `migrate::to_current` → typed deserialize; notes go to stderr as `note: <path>: …`; no file write on load
- [ ] 1.4 Gate: golden fixtures load to identical `PawConfig`; `just check` green

## 2. Wire type and model split (behavior-preserving)

- [ ] 2.1 Add `src/config/wire.rs` with `ConfigFile` and per-section wire structs copying today's serde attributes exactly (including `SpecsConfig` `rename = "type"` and every `default` / `skip_serializing_if`)
- [ ] 2.2 Add total mappers `From<wire::ConfigFile> for PawConfig` and `From<&PawConfig> for wire::ConfigFile`; keep `Option` section semantics so `merged_with` is untouched
- [ ] 2.3 Switch `load_config_file` and `save_config_to` to the wire types; remove `Serialize`/`Deserialize` from `PawConfig` and its section types; keep `crate::config::*` re-exports so callers compile unchanged
- [ ] 2.4 Gate: golden `save_config_to` output byte-identical; `config_survives_save_and_load` and every `config_integration` test unchanged and green

## 3. Version semantics

- [ ] 3.1 Add `version: Option<u32>` as the first field of `wire::ConfigFile` (`default`, `skip_serializing_if = "Option::is_none"`); the model→wire mapper always sets `Some(CURRENT_CONFIG_VERSION)`
- [ ] 3.2 Refuse a version newer than current, or one that is zero, negative or not an integer, with `PawError::ConfigError` naming the path, the declared value, the supported version, and the "upgrade git-paw" hint
- [ ] 3.3 Add the active `version = 1` line to `generate_default_config()` above the first section; confirm `init::migrate_existing_config` backfills it and leaves a declared one alone
- [ ] 3.4 `add_custom_cli` / `remove_custom_cli` write through the wire type (they get the version line automatically)

## 4. Doctor

- [ ] 4.1 `ConfigProbe` carries the declared version and an unsupported-version error; `check_config` emits the schema-version check (✓ declared, ✓ implicit 1, ✗ newer with upgrade remedy, ✗ invalid)
- [ ] 4.2 `unknown_config_keys` treats `version` as a known key

## 5. Tests (one or more per scenario)

- [ ] 5.1 Unversioned fixtures load unchanged; `version = 1` is equal to no version; global `version = 1` with an unversioned repo merges unchanged (`tests/config_integration.rs`)
- [ ] 5.2 Load leaves the file's bytes and mtime unchanged
- [ ] 5.3 Migration chain unit table: a test-only v1→v2 rename step runs once and declares v2 with notes; a current document passes through with no notes (`config/migrate.rs` tests)
- [ ] 5.4 `version = 2` refused with path, both versions and upgrade hint; `version = "one"` and `version = 0` refused; an unknown key in v1 is tolerated
- [ ] 5.5 Wire↔model round-trip over a fully populated model; `[specs] type` spelling kept; global-over-repo merge result equal to the pre-change golden result
- [ ] 5.6 `git paw add-cli` on an unversioned global config (HOME overridden to a `TempDir`) writes `version = 1` and keeps both CLIs; serialized output equals the golden output plus a leading version line
- [ ] 5.7 Template contains active `version = 1` before the first section; `git paw init` backfills it into a customized unversioned config and leaves a declared version untouched
- [ ] 5.8 Standing guard `tests/config_script_keys_documented.rs`: every `[section].key` the bundled scripts read appears in the configuration reference's script-read list
- [ ] 5.9 Script parity: `discover_broker_url` resolves port 9200 from a `version = 1` config, both through `tomllib` and through the regex fallback (force the fallback with a `PY` shim)
- [ ] 5.10 Doctor: ✓ declared version (and `version` absent from unknown fields), ✓ implicit 1, ✗ newer with non-zero exit and the file unchanged (`src/doctor_tests.rs`, `tests/doctor_e2e.rs`)

## 6. Docs

- [ ] 6.1 `docs/src/configuration/README.md`: document `version`, add a "Compatibility and versioning" section (absent means 1, a newer version is refused, git-paw never rewrites to migrate, additive keys do not bump the version), and list the script-read keys
- [ ] 6.2 Comment the `version` line in the `init` template; update `git paw init --help` / `doctor --help` only if their text names config fields
- [ ] 6.3 `docs/src/architecture.md`: config module list gains `wire.rs` / `migrate.rs`; `architecture-direction.md` "Where the code is today" row for serialized formats notes config is split

## 7. Gates

- [ ] 7.1 `cargo fmt`, then `just lint`
- [ ] 7.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [ ] 7.3 `just deny`
- [ ] 7.4 `mdbook build docs/`
- [ ] 7.5 `openspec validate config-schema-versioning --strict`
