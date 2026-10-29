## Context

Config is loaded by `config::load_config(repo_root, user_config_path)` →
`load_config_from` → `load_config_file` on the global file
(`dirs::config_dir()/git-paw/config.toml`) and the repo file (`<repo>/.git-paw/config.toml`),
then `global.merged_with(&repo)` and `auto_wire_governance`. `load_config_file` runs
`toml::from_str::<PawConfig>` and then the `validate_approval_args` /
`validate_session_templates` checks. No struct uses `deny_unknown_fields`, so unknown keys are
dropped without a message. `doctor::unknown_config_keys` finds them after the fact by
round-tripping through `PawConfig`.

Config is written in three places:
- `save_config_to`, a full `toml::to_string_pretty` rewrite. It is used by
  `add_custom_cli` / `remove_custom_cli` on the **global** file, and it already drops comments
  and unknown keys.
- `init::write_config_if_missing`, for fresh files only.
- `init::migrate_existing_config`, a text-level additive backfill of missing sections and
  top-level template keys. It is the only migration that exists today.

The bundled scripts read `[broker].port`/`bind`, `docs_base_url`, `[supervisor].test_command`
and `[supervisor].no_progress_window_seconds` / `context_bloat_threshold_k` /
`blocked_on_supervisor_window_seconds`, using `tomllib` with a regex fallback
(`assets/scripts/_paw_common.sh`, `sweep.sh`).

`code-architecture` freezes two config serde surfaces: `SpecsConfig` `rename = "type"` and
`PawConfig::merged_with`'s default-as-unset merge.

## Goals / Non-Goals

**Goals:**
- A version marker plus a migration hook, so the format can change after v1.0.0 without breaking
  older files.
- The internal model (`PawConfig`) can change without changing the file format.
- Every config that loads today loads the same way. The only observable additions are the
  `version` key in files git-paw writes and the new doctor line.

**Non-Goals:**
- Any actual schema change. The chain is empty at version 1.
- Rewriting user files during migration.
- `git paw config check` (owned by the other v0.21.0 track).
- A deprecated-key warning helper (v1.0.0 deprecation policy).
- JSON Schema publication for config.

## Decisions

### D1 — Top-level `version` integer; absent means 1
The key is `version`, which is the name MILESTONE gives, and its value is an integer. A missing
key means version 1, defined as exactly today's layout, so no existing file needs a migration.
The constant is `config::migrate::CURRENT_CONFIG_VERSION = 1`. The version is bumped **only**
for an incompatible layout change (rename, move, or change of meaning). Additive optional fields
keep the version unchanged, as they do today.

*Alternatives:* a key named `schema_version` or `config_version` is less likely to be read as
"git-paw's version" (open question 1). A semver string adds parsing and gains nothing; a single
monotonic integer is enough for a chain of migrations.

### D2 — Migrate a `toml::Value`, then deserialize the wire type
The load pipeline is:
`read_to_string → toml::from_str::<toml::Value> → read version → migrate::to_current(value, from)`
`→ value.try_into::<wire::ConfigFile>() → PawConfig::from(wire) → existing validators`.

Each step `migrate_vN_to_vN1(&mut toml::Value) -> Vec<MigrationNote>` is a pure function on the
document tree. It is tested with fixtures and does no I/O. `MigrationNote`s go to stderr once per
load as `note: <path>: …`. Migrating the generic tree, rather than deserializing older typed
structs, means each version needs only the step that changes it, with no `ConfigFileV1`,
`ConfigFileV2`, … hierarchy.

*Alternative:* versioned typed structs per version with `From` chains. That copies the whole
section tree once per version, and is rejected as ceremony.

### D3 — A newer or invalid version fails the load
A `version` above `CURRENT_CONFIG_VERSION` returns a `ConfigError` like:
`<path>: config version 2 is newer than this git-paw supports (1) — upgrade git-paw`.
A zero, a negative value or a non-integer value is also a `ConfigError`. The other choice,
loading best-effort with a warning, is wrong for this case: a version bump means the layout
changed incompatibly, and the old binary would misread it, with unknown keys silently ignored.
Unknown keys inside a known version stay accepted and are reported only by `doctor`, as today.
(Open question 2.)

### D4 — Wire type and internal model split mechanically
`src/config/wire.rs` holds `ConfigFile` and the per-section wire structs. They are byte-for-byte
the current serde attributes, including `rename = "type"`, every `default` /
`skip_serializing_if`, and `Option` sections. They also carry
`#[serde(default, skip_serializing_if = "Option::is_none")] version: Option<u32>`. `PawConfig`
and its section types drop `Serialize`/`Deserialize` and gain
`From<wire::ConfigFile> for PawConfig` and `From<&PawConfig> for wire::ConfigFile`.

The internal sections keep their `Option` fields, so `merged_with` runs unchanged on
`PawConfig`. Merge happens after mapping and after each file's own migration. Enums that are
both config values and domain types (`ApprovalLevel`, `WorktreePlacement`, …) get wire mirrors
only where their serde spelling is the contract. The mapper is total and has no failure path, so
the mapping needs no error handling. Validators keep operating on `PawConfig`.

The work is done seam-first: introduce `wire.rs` as type aliases of today's structs (no change),
switch `load_config_file` / `save_config_to` to the pipeline, then move the derives. The suite
stays green at each step.

*Alternative:* version plus `toml::Value` migration with no DTO, keeping `PawConfig` as the serde
type. It is cheaper, but every internal rename stays a format change, and MILESTONE's
contract-locking item explicitly asks for the split. Recorded as the fallback if the mapper
proves too costly (open question 3).

### D5 — git-paw-written files carry the version
`save_config_to` always emits `version = 1`. `ConfigFile` serializes `version` first, and the
mapper sets `Some(CURRENT_CONFIG_VERSION)`. The default template gains an active top-level
`version = 1` line under its header comment. `init::migrate_existing_config` already backfills
any absent active top-level template key, inserting it above the first `[section]`, so
`git paw init` adds `version = 1` to existing files with no code change. `init` is still
additive: a file that already has `version` is left untouched.

### D6 — Doctor reports the version
`ConfigProbe` gains `version: Option<u32>` (the declared value) and an "unsupported" error.
`check_config` adds a `schema version` check:
- ✓ `version N` when declared and supported;
- ✓ `version 1 (implicit)` when absent;
- ✗ when it is newer or invalid, with the remedy "upgrade git-paw".

`unknown_config_keys` treats `version` as known. This is an ADDED requirement, so it does not
collide with any edit the other track makes to the existing "Config check" requirement.

### D7 — Script-read keys are named as contract
The configuration reference lists the keys the bundled scripts read and states that renaming
any of them is a version bump plus a script change in the same release. A standing test keeps
the list in step: it greps `assets/scripts/*.sh` for `data.get(` / `sup.get(` section keys and
asserts each one is listed in the reference. This follows the `spec-organization` §7 guard
pattern.

## Risks / Trade-offs

- [The mapper drifts from the wire type when a field is added on one side only] → the mapper
  round-trip test runs over a fully populated fixture, `config_survives_save_and_load` stays
  green, and the compiler flags struct literals.
- [`save_config_to` output gains a `version = 1` line] → this is intended. A global config
  written by `add-cli` was already a full rewrite, and only the added line differs, which is
  pinned by a golden test.
- [A user hand-writes `version = 2` by mistake] → clear `ConfigError` with both numbers.
- [A regex-fallback script misreads `version`] → the fallbacks match only `port`, `bind`,
  `docs_base_url` and named `[supervisor]` keys, not `version`. A parity test feeds a
  `version = 1` config to `discover_broker_url` and `discover_docs_base_url`.

## Migration Plan

1. Wave 1 (seam): `config/migrate.rs` and the `toml::Value` pipeline in `load_config_file` with
   an identity chain, plus golden-fixture tests. Behavior is identical.
2. Wave 2: `config/wire.rs` holds the serde derives, with the mappers. Byte-identical
   round-trip tests on the v0.2.0-onward fixtures.
3. Wave 3: the `version` field, refusal of newer versions, the template line, the doctor check,
   and docs.

Rollback is a revert of each wave. Files written with `version = 1` still load on a reverted
binary, because the key is unknown there and therefore ignored.

## Open Questions

1. Key name: `version` (MILESTONE's wording, recommended) or `schema_version`.
2. A newer version is a hard error (recommended) rather than warn-and-load.
3. Full wire/model split (recommended, MILESTONE item) rather than the cheaper version-plus-
   migration-only fallback.
