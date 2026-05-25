## 1. Configuration

- [x] 1.1 Add `CommonDevAllowlistConfig { enabled: bool, extra: Vec<String> }` struct to `src/config.rs`. `enabled` defaults to `true` via `#[serde(default = "CommonDevAllowlistConfig::default_enabled")]`; `extra` defaults to empty via `#[serde(default)]`. Derive `Debug, Clone, Serialize, Deserialize, PartialEq, Eq` matching `AutoApproveConfig` conventions.
- [x] 1.2 Implement `Default` for `CommonDevAllowlistConfig` returning `{ enabled: true, extra: vec![] }`. Implement the `default_enabled() -> bool { true }` helper used by `#[serde(default = ...)]`.
- [x] 1.3 Add `pub common_dev_allowlist: CommonDevAllowlistConfig` field to `SupervisorConfig` with `#[serde(default)]` so existing configs that omit the section parse to the default.
- [x] 1.4 Update the generated default config template (around `src/config.rs:586`) to include a commented `[supervisor.common_dev_allowlist]` section showing the defaults and the `extra` field with example entries.
- [x] 1.5 Update the supervisor merge logic (overlay/local-vs-repo precedence at `src/config.rs:401`) to merge the new sub-table consistent with existing supervisor merge rules — repo config wins over global, falls back to defaults.

## 2. Config tests

- [x] 2.1 `supervisor_common_dev_allowlist_defaults_when_section_absent` — parse `[supervisor]\nenabled = true\n` and assert `common_dev_allowlist.enabled == true` and `extra.is_empty()`.
- [x] 2.2 `supervisor_common_dev_allowlist_disabled_opt_out` — parse `[supervisor.common_dev_allowlist]\nenabled = false\n` and assert `enabled == false`.
- [x] 2.3 `supervisor_common_dev_allowlist_extra_parsed` — parse `[supervisor.common_dev_allowlist]\nextra = ["pnpm test", "deno fmt"]` and assert the vec equals those two entries.
- [x] 2.4 `supervisor_common_dev_allowlist_round_trips_through_save_and_load` — construct a `SupervisorConfig` with a non-default `CommonDevAllowlistConfig`, save to TOML, load back, assert equality.
- [x] 2.5 `existing_pre_v05_config_loads_with_default_common_dev_allowlist` — fixture: a `.git-paw/config.toml` that pre-dates this change (no `common_dev_allowlist` table). Assert load succeeds and the field has defaults.
- [x] 2.6 `generated_default_config_template_contains_common_dev_allowlist_section` — assert the output of the default-config generator contains the new commented section.

## 3. Dev allowlist module

- [x] 3.1 Create `src/supervisor/dev_allowlist.rs` (peer to `curl_allowlist.rs`).
- [x] 3.2 Define `pub const DEV_ALLOWLIST_PRESET: &[&str]` containing exactly the patterns enumerated in the `dev-command-allowlist` capability's "Standard preset content" requirement (Cargo subset, Git read + non-destructive write, Just, mdBook, OpenSpec, `find`, `grep`, `sed -n`). Order is informational but lock the constant so tests assert content via set equality.
- [x] 3.3 Implement `pub fn effective_patterns(extra: &[String]) -> Vec<String>` returning `DEV_ALLOWLIST_PRESET` followed by `extra` entries that are not already present in the preset.
- [x] 3.4 Implement `pub fn setup_dev_allowlist(extra: &[String], settings_path: &Path) -> Result<(), PawError>` mirroring `setup_curl_allowlist`. Merge semantics: load existing JSON or start empty; require top-level object; require `allowed_bash_prefixes` to be an array (or insert one); append missing entries from `effective_patterns(extra)`; preserve all other fields; create parent dir when missing; return `PawError::ConfigError` (matching `curl_allowlist.rs` precedent) on invalid JSON or top-level non-object.
- [x] 3.5 Register the module in `src/supervisor/mod.rs` (or whatever the supervisor mod root is).
- [x] 3.6 Public items receive `///` doc comments per project convention (`Code Style` rules).

## 4. Wiring into `cmd_supervisor`

- [x] 4.1 In `src/main.rs::cmd_supervisor()`, after `git::prune_worktrees(...)` and after the existing curl-allowlist seeding block (around line 757), invoke `setup_dev_allowlist(...)` against `<repo>/.claude/settings.json` when `supervisor_cfg.common_dev_allowlist.enabled` is `true`. Use the same non-fatal pattern: log warning to stderr on error and continue.
- [x] 4.2 When the directory `~/.claude-oss/` (resolved via `dirs::home_dir()`) exists at session start, also invoke `setup_dev_allowlist(...)` against `~/.claude-oss/settings.json`. When the directory does not exist, skip silently (do NOT create it).
- [x] 4.3 Apply the same calls in the recovery path (around `src/main.rs:1353`) so re-attached sessions re-seed the preset.
- [x] 4.4 The dev-allowlist seeding SHALL run regardless of `broker_config.enabled` (unlike the curl-allowlist call, which is broker-gated).

## 5. Unit tests on the seeder

- [x] 5.1 `writes_preset_when_file_absent` — fresh settings.json. Empty `extra`. Assert every preset pattern is present in `allowed_bash_prefixes` after the call.
- [x] 5.2 `merges_with_existing_user_entries` — pre-populate `allowed_bash_prefixes` with `["my-tool", "some-other"]` and another top-level field. After seeding: those entries plus that field still present; preset appended.
- [x] 5.3 `does_not_duplicate_existing_preset_entries` — pre-populate `allowed_bash_prefixes` with `["cargo build", "git push"]`. After seeding: each appears exactly once.
- [x] 5.4 `appends_extra_patterns_after_preset` — pass `extra = ["pnpm test", "deno fmt"]`. Assert both appear in the resulting array and follow the preset.
- [x] 5.5 `extra_entries_not_validated` — pass `extra = ["this is nonsense $$"]`. Seeder succeeds; the entry is present.
- [x] 5.6 `extra_duplicates_preset_entry_not_added_twice` — pass `extra = ["cargo build"]` on a fresh file. Assert `cargo build` appears exactly once.
- [x] 5.7 `invalid_json_returns_error_not_panic` — write `not json {{{` to settings.json. Assert `setup_dev_allowlist` returns `PawError::ConfigError` containing `"invalid JSON"`. The file SHALL be left unchanged.
- [x] 5.8 `creates_parent_directory_when_missing` — point at `<tmp>/.claude/settings.json` where `.claude/` does not exist. Assert call succeeds and the file exists afterwards.
- [x] 5.9 `preset_constant_contains_all_required_patterns_and_no_excluded_ones` — a behavioural test on `DEV_ALLOWLIST_PRESET` asserting: every required pattern from the capability's "Standard preset content" requirement is present; no excluded pattern (`cargo install`, `cargo run`, `git rebase`, `git reset`, `git checkout`, `git push --force`, `sed` without `-n`, `npm`, `pnpm`, etc.) is present.
- [x] 5.10 `effective_patterns_orders_preset_before_extra` — preset entries appear before extra entries in the returned `Vec`.
- [x] 5.11 `effective_patterns_deduplicates_extra_against_preset` — `effective_patterns(&["cargo build".into()])` does not produce two `cargo build` entries.

## 6. Integration tests

- [x] 6.1 `tests/dev_allowlist_integration.rs` (or extend `tests/supervisor_*` tests). Launch `cmd_supervisor` against a tempdir repo with default config (no `common_dev_allowlist` table). After the call, read `<repo>/.claude/settings.json` and assert `allowed_bash_prefixes` contains every preset pattern.
- [x] 6.2 Same test with `[supervisor.common_dev_allowlist] enabled = false` — assert `<repo>/.claude/settings.json` is **not** modified by the dev-allowlist code path. (If a curl-allowlist call also runs because broker is enabled, the file may still exist; assert no preset pattern is in it.)
- [x] 6.3 Test with `extra = ["pnpm test"]` — assert the entry appears in `allowed_bash_prefixes` alongside the preset.
- [x] 6.4 Test with `~/.claude-oss/` simulated via a tempdir-rooted `HOME` env (or equivalent test hook): when the directory pre-exists, assert `~/.claude-oss/settings.json` was also written; when absent, assert the directory is **not** created and only `<repo>/.claude/settings.json` is written.
- [x] 6.5 Recovery path test: simulate a session re-attach (the code path around `src/main.rs:1353`) and assert the preset is re-seeded into `<repo>/.claude/settings.json`.
- [x] 6.6 Non-broker supervisor test: `[broker] enabled = false`, `[supervisor] enabled = true`. Assert the preset is still seeded (the seeding does not depend on broker status).
- [x] 6.7 Invalid-JSON existing file test: seed a malformed `<repo>/.claude/settings.json`, run `cmd_supervisor`. Assert the function returns success at the session-start level (non-fatal) and a warning was emitted to stderr identifying the file.

## 7. Documentation

- [x] 7.1 Add a "Common dev-command allowlist" subsection to the supervisor user-guide chapter (`docs/src/user-guide/supervisor.md` or wherever the supervisor docs live). Cover: what the preset includes (link to `dev-command-allowlist/spec.md`); how to opt out (`[supervisor.common_dev_allowlist] enabled = false`); how to extend (`extra = [...]`); where to find / prune `.claude/settings.json` if the user wants a clean slate.
- [x] 7.2 Document the `[supervisor.common_dev_allowlist]` table in `docs/src/configuration.md` with the full default values and a usage example.
- [x] 7.3 Update `README.md` if the supervisor section lists user-visible behaviour changes for v0.5.0 — mention the new default-enabled allowlist and where to opt out.
- [x] 7.4 `mdbook build docs/` succeeds.

## 8. Release notes

- [x] 8.1 v0.5.0 release notes: announce the new default-enabled `[supervisor.common_dev_allowlist]` feature. Enumerate the preset patterns in full (so users can audit before upgrading). Note the opt-out one-liner (`enabled = false`) and the `extra` extension point. Note that this is the v0.5.0 mitigation for drift 44 / drift 27; full per-CLI placement lands in v1.0.0 hook-providers.

## 9. Quality gates

- [ ] 9.1 `just check` — fmt, clippy pedantic, all tests green.
- [ ] 9.2 `just deny` — license / advisory / duplicate-dep checks clean.
- [ ] 9.3 No new `unwrap()` / `expect()` in non-test code. All public items have `///` doc comments. All module-level entries have `//!` doc comments.
- [ ] 9.4 `mdbook build docs/` succeeds.
- [ ] 9.5 `openspec validate common-dev-allowlist-preset --strict` passes.
- [ ] 9.6 Coverage on `src/supervisor/dev_allowlist.rs` is >= 80% (the module is pure logic and the integration tests exercise the wiring).
