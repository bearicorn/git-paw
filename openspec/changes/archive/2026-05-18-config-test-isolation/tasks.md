## 1. Signature change in `src/config.rs`

- [x] 1.1 Change `pub fn load_config(repo_root: &Path) -> Result<PawConfig, PawError>` to `pub fn load_config(repo_root: &Path, user_config_path: Option<&Path>) -> Result<PawConfig, PawError>`.
- [x] 1.2 In the body, branch on the new parameter: `Some(p) => p.to_path_buf()` becomes the global path; `None => global_config_path()?` preserves v0.4 resolution. Pass the resulting path to the existing `load_config_from(&global_path, repo_root)` call (which is unchanged).
- [x] 1.3 Update the Rustdoc on `load_config` to document the new parameter, the `None`-preserves-v0.4 contract, and the test-isolation use case for `Some(_)`. Cross-reference `load_config_from` in the doc comment so future readers see the relationship.

## 2. Production call-site updates in `src/main.rs`

- [x] 2.1 Update line 72 from `config::load_config(&repo_root)` to `config::load_config(&repo_root, None)`.
- [x] 2.2 Update line 230 from `config::load_config(&repo_root).unwrap_or_default()` to `config::load_config(&repo_root, None).unwrap_or_default()`.
- [x] 2.3 Update line 340 to `config::load_config(&repo_root, None)?`.
- [x] 2.4 Update line 1094 to `config::load_config(&repo_root, None)?`.
- [x] 2.5 Update line 1326 to `config::load_config(repo_root, None)?`.
- [x] 2.6 Update line 1397 to `config::load_config(&repo_root, None)?`.
- [x] 2.7 Update line 1748 to `config::load_config(&repo_root, None)?`.
- [x] 2.8 Update line 1830 to `config::load_config(&repo_root, None)?`.
- [x] 2.9 Verify with `grep -rn "load_config(" src/` that no production call site passes `Some(_)` and that every call has exactly two arguments. (Line numbers above are from the v0.5.0 codebase as of the prep-commit baseline; if the file has shifted, the eight call sites are the only ones returned by `grep -n "config::load_config(" src/main.rs`.)

## 3. Test call-site updates in `tests/config_integration.rs`

- [x] 3.1 Update line 31 (`load_config_returns_defaults_when_no_files_exist`) from `load_config(tmp.path())` to `load_config(tmp.path(), Some(&tmp.path().join("global.toml")))`. The override path intentionally points at a file that doesn't exist so the user-level side returns defaults.
- [x] 3.2 Update line 55 (`load_config_reads_repo_config`) similarly.
- [x] 3.3 Update line 77 (`repo_config_with_custom_clis`) similarly.
- [x] 3.4 Update line 114 (`repo_config_with_presets`) similarly.
- [x] 3.5 Update line 149 (`repo_config_overrides_default_fields`) similarly.
- [x] 3.6 Update line 177 (`malformed_toml_returns_error`) similarly.
- [x] 3.7 Update line 191 (`empty_config_file_is_valid`) similarly.
- [x] 3.8 Update line 406 (`config_with_many_custom_clis`) similarly.
- [x] 3.9 Verify with `grep -n "load_config(" tests/config_integration.rs` that every call to the two-argument `load_config` passes `Some(_)`. Calls to `load_config_from` and `load_repo_config` SHALL be left as-is (they are already isolated by construction).

## 4. New unit test in `src/config.rs::tests`

- [x] 4.1 Add `load_config_with_some_pins_global_to_override_path`: build two distinct TOML files in a `TempDir` (`global-A.toml` defining `cli-A`, `global-B.toml` defining `cli-B`), call `load_config(&repo, Some(&global_a))`, assert the returned `clis` contains `cli-A` and does NOT contain `cli-B`. This proves `Some(_)` actually controls the user-level read.
- [x] 4.2 Add `load_config_with_some_nonexistent_returns_defaults`: pass `Some(&tmp.path().join("does-not-exist.toml"))`, assert no error is returned and the user-level side of the merge is `PawConfig::default()`. This proves a missing override path is not an error and matches the existing `load_config_file` "NotFound ⇒ None" branch.
- [x] 4.3 Add `load_config_override_does_not_affect_repo_resolution`: write a `.git-paw/config.toml` to the `TempDir` defining `default_cli = "claude"`, write a separate `global.toml` defining `default_cli = "gemini"`, call `load_config(&tmp, Some(&global_path))`, assert the merged `default_cli` is `"claude"` (repo overrides user). This proves the override parameter is purely about the user-level read, not the repo-level resolution.
- [x] 4.4 (Optional, lower priority) Add `load_config_with_none_reads_platform_default_global` if the test can be written without polluting the dev machine's real `~/Library/Application Support/git-paw/config.toml`. If not possible without `serial_test` + env-var manipulation, document this in a code comment and rely on the eight production call sites + the existing v0.4 test suite to cover the `None` branch instead. The MODIFIED requirement's scenario "None preserves platform-default user-config resolution" SHALL still be considered satisfied: the production call sites that pass `None` and continue to pass `--from-specs` / `start` / `add-cli` end-to-end against the platform-default path are the behavioural evidence. [Skipped per spec — code comment in `src/config.rs::tests` documents that the `None` branch is covered behaviourally by the 8 production call sites and the v0.4 suite; writing the test would either pollute the real platform-default path or require brittle env-var manipulation.]

## 5. Quality gates

- [ ] 5.1 `just check` (fmt + clippy + tests) passes on the change branch. Specifically, the four previously-failing tests in `tests/config_integration.rs` (lines 31, 77, 191, 406 in the v0.5.0 baseline) MUST pass with at least one custom CLI registered at the dev machine's platform-default user-config path, proving the leak is fixed.
- [ ] 5.2 `just deny` passes (no new dependencies — this change is signature + caller updates only).
- [ ] 5.3 No `unwrap()`/`expect()` introduced in the new `load_config` body or the three/four new unit tests' production-side helpers. Inside the test bodies themselves, the existing `tempfile` `.expect()` pattern is allowed and consistent with surrounding tests.
- [ ] 5.4 The new `load_config` signature has a complete Rustdoc block including the `None` ⇒ v0.4-default contract and a one-line cross-reference to `load_config_from`.

## 6. Docs

- [ ] 6.1 No `--help` text changes — there is no new flag.
- [ ] 6.2 No README changes — the change is invisible to end users.
- [ ] 6.3 No mdBook chapter updates — this is an internal API; the user-facing config reference (`docs/src/configuration.md` or equivalent) does not document `load_config`.
- [ ] 6.4 The `--api-docs` rustdoc surface SHALL show the new parameter on `load_config` (covered by 1.3 — the Rustdoc update).

## 7. Backward-compat verification

- [ ] 7.1 Run the v0.4 test suite (or the unchanged tests within `tests/config_integration.rs`) against this change to confirm production behaviour is byte-identical when `None` is passed. The existing tests that already use `load_config_from(&global_path, &repo_root)` SHALL continue to pass without modification.
- [ ] 7.2 Smoke-test the production commands (`git paw start`, `git paw add-cli`, `git paw dashboard`, `git paw supervisor`) on a real session to confirm the eight call-site updates didn't introduce a regression. The dogfood pass after this lands SHALL note explicitly whether `add-cli` / `remove-cli` continue to write to the same platform-default path as before.

## 8. Cross-change findings (optional, if surfaced during implementation)

These are captured as drift items only if surfaced during this work; they are NOT required for this change to ship.

- [ ] 8.1 If a `GIT_PAW_CONFIG_DIR` env-var override is requested during code review (e.g. for sandbox/CI runners), file it as a separate change. Per `design.md` D1 the env var composes on top of the `Option<&Path>` API (precedence: explicit `Some` → env var → `crate::dirs::config_dir()`); no spec change to *this* requirement is needed when the env var lands later.
- [ ] 8.2 If reviewers identify additional `tests/*.rs` files that call `load_config(&path)` (one-argument form), update them in this change's scope. As of the v0.5.0 baseline, only `tests/config_integration.rs` uses the one-argument form (verified by `grep -rn "load_config(" tests/`); other test files either use `load_config_from` or don't call the loader directly.
