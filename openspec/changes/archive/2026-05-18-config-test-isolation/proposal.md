## Why

During the v0.5.0 OSS-config dogfood pass, the maintainer registered a `claude-oss` custom CLI via `git paw add-cli claude-oss …` so the dogfood session could exercise the alternate `CLAUDE_CONFIG_DIR=~/.claude-oss claude` invocation. The next `just check` run failed: four tests in `tests/config_integration.rs` started asserting on a `PawConfig` that contained an extra CLI no test had ever written.

Root cause: those tests call `load_config(tmp.path())`, which loads both the per-repo `.git-paw/config.toml` (correctly scoped to the `TempDir`) and the global config (via `global_config_path()` → `crate::dirs::config_dir()`, which on the dev machine resolves to `~/Library/Application Support/git-paw/config.toml`). Any globally-registered CLI from `git paw add-cli` leaks into the in-memory `PawConfig` the tests then assert against. The four failing assertions all check counts or absence of CLIs:

- `tests/config_integration.rs:31` — `load_config_returns_defaults_when_no_files_exist` asserts `config.clis.is_empty()`.
- `tests/config_integration.rs:79` — `repo_config_with_custom_clis` asserts `config.clis.len() == 2`.
- `tests/config_integration.rs:192` — `empty_config_file_is_valid` asserts `config == PawConfig::default()`.
- `tests/config_integration.rs:407` — `config_with_many_custom_clis` asserts `config.clis.len() == 10`.

`just check` only fails when a global CLI is registered on the dev machine — CI passes because CI runs on a clean home — so this didn't surface until a real dogfood scenario forced the maintainer to add `claude-oss` globally. The bug is pre-existing (the tests have been wrong since v0.2.0); the global CLI registration only made it observable.

This is MILESTONE drift item 24 ("Config-test isolation brittleness, pre-existing, surfaced by global CLI registration"). The drift item lists two fix candidates: an env-var override (`GIT_PAW_CONFIG_DIR`) on `load_config`, or an explicit `user_config_path: Option<PathBuf>` argument on `load_config`. This change picks the latter — see `design.md` for rationale.

The fix is intentionally narrow: a single signature evolution on `load_config`, four test call-site updates, and one new unit test. No production behaviour changes (all internal callers pass `None`, preserving v0.4 resolution semantics).

## What Changes

**`load_config` signature evolution** (`src/config.rs`):

Today:
```rust
pub fn load_config(repo_root: &Path) -> Result<PawConfig, PawError> {
    let global_path = global_config_path()?;
    load_config_from(&global_path, repo_root)
}
```

After:
```rust
pub fn load_config(
    repo_root: &Path,
    user_config_path: Option<&Path>,
) -> Result<PawConfig, PawError> {
    let global_path = match user_config_path {
        Some(p) => p.to_path_buf(),
        None => global_config_path()?,
    };
    load_config_from(&global_path, repo_root)
}
```

`Some(p)` pins the global-config read to an explicit path (typically a `TempDir`-rooted path in a test); `None` preserves v0.4 behaviour (`global_config_path()` → `~/Library/Application Support/git-paw/config.toml` on macOS, `~/.config/git-paw/config.toml` on Linux).

`load_config_from(&Path, &Path)` is unchanged — it already takes an explicit global-path argument and is the existing escape hatch for tests that need fine-grained control. The new `Option` parameter on `load_config` is the more discoverable wrapper for the common test case ("just give me an isolated load").

**Internal callers updated to pass `None`** (`src/main.rs` — 8 call sites at lines 72, 230, 340, 1094, 1326, 1397, 1748, 1830):

Every existing production call site becomes `config::load_config(&repo_root, None)`. Behaviour is byte-identical to v0.4 — the `None` branch calls `global_config_path()` exactly as before.

**Four failing tests updated** (`tests/config_integration.rs` lines 31, 77, 191, 406 in the v0.5.0 codebase):

Each call site becomes `load_config(tmp.path(), Some(&tmp.path().join("global-config.toml")))` (or an equivalent isolated path that doesn't exist, so the global side returns defaults). The four currently-passing-but-leaky tests now pass independently of whatever global CLIs are registered on the dev machine.

The other `load_config(tmp.path())` calls in the same file (lines 55, 114, 149, 177) are NOT failing on the dev machine because their assertions don't touch `clis` count / `PawConfig::default()` equality. They are still latent regressions waiting to happen and SHALL also be updated as part of this change — the cost of the migration is the same and the cleanup leaves the test file consistent.

**New unit test** for the override parameter (`src/config.rs::tests`):

A single behavioural test asserts that passing `Some(path_to_existing_global_with_CLI)` causes that CLI to appear in the merged config, AND that passing `Some(path_to_nonexistent)` returns defaults — proving the override actually controls the global-config read independently of the dev machine's real global config.

**Affected sites NOT changed:**

- `load_config_from(&Path, &Path)` — already takes explicit paths; no signature change needed. Used directly by some `src/config.rs::tests` and `tests/config_integration.rs::add_custom_cli_with_*` tests that already pin both paths.
- `load_repo_config(&Path)` — only reads the repo-level config, never touches the global path. No isolation problem.
- `global_config_path()` — unchanged; still resolves to `crate::dirs::config_dir().join("git-paw/config.toml")` for the production case.
- `add_custom_cli` / `remove_custom_cli` — these are the unparameterised wrappers around `add_custom_cli_to` / `remove_custom_cli_from`. They already use `global_config_path()` and are intended for the production CLI surface, not tests; no test calls them.

**Not in scope (deferred):**

- A `GIT_PAW_CONFIG_DIR` env-var override that also redirects `add-cli` / `remove-cli` writes. The function-parameter approach is sufficient for the test-isolation goal and avoids the env-var-vs-config-precedence question. If users later request env-var-driven config redirection (e.g. for sandbox/CI runners), it can layer on top of `load_config` without breaking the `Option<&Path>` API.
- Renaming `load_config_from` to something clearer (it has confusing parameter-order semantics). Out of scope; a follow-up `config-loader-rename` change can address it after this lands.
- Reworking the eight `src/main.rs` `load_config` call sites to share a single helper. Mechanical, low-value, out of scope.

## Capabilities

### New Capabilities
*(none — this change extends an existing requirement)*

### Modified Capabilities

- `configuration`: the existing "Config loading SHALL work with real files" requirement keeps its scenarios and grows new scenarios stating (i) `load_config` accepts an optional user-config path override, (ii) `None` preserves v0.4 behaviour (reads `global_config_path()`), and (iii) `Some(path)` pins the global-config read to that path even if a different file exists at `global_config_path()`.

## Impact

**Code:**
- `src/config.rs::load_config` — signature gains `user_config_path: Option<&Path>`. Body branches on the option.
- `src/main.rs` — 8 production call sites updated to pass `None`. Pure mechanical change.
- `src/config.rs::tests` — one new unit test for the override parameter.

**Tests:**
- `tests/config_integration.rs` — every `load_config(tmp.path())` becomes `load_config(tmp.path(), Some(&isolated_global))` where `isolated_global` is a `TempDir`-rooted path. The four tests that were failing on the dev machine pass independently of the dev machine's global config.
- One new unit test in `src/config.rs::tests` proves the override is honoured both when the override path exists and when it doesn't.

**Docs:**
- `--help` text: unchanged (no CLI surface change).
- README: unchanged.
- mdBook: unchanged (this is an internal API).
- Rustdoc on `load_config`: updated to document the new parameter and the `None` ⇒ v0.4-default contract.

**Backward compatibility:** Source-breaking for direct library consumers of `git-paw`'s `config::load_config` (the signature gains a parameter). The crate is published on crates.io but the public API surface is documented as "for the binary's internal use"; no external consumer is known. Production behaviour with `None` is byte-identical to v0.4.

**Mismatches resolved:**
- MILESTONE drift item 24 (config-test isolation brittleness) — resolved. The four failing tests now pass on any dev machine regardless of registered global CLIs.
- Eliminates the latent regression risk in the other `load_config(tmp.path())` call sites that happen to not assert on CLI counts today.
