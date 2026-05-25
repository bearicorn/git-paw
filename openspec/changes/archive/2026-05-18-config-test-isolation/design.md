## Context

`config::load_config(repo_root)` is the workhorse loader used by every production code path that needs the merged `PawConfig`. It internally calls `global_config_path()` → `crate::dirs::config_dir()` and reads `~/Library/Application Support/git-paw/config.toml` (macOS) or `~/.config/git-paw/config.toml` (Linux), then merges the per-repo `.git-paw/config.toml` on top.

That is the right behaviour in production, but it is wrong in `tests/config_integration.rs`. Four tests there call `load_config(tmp.path())` and assert things like `config.clis.is_empty()` or `config == PawConfig::default()`. The repo-level `TempDir` is isolated, but the global-level path is whatever the dev machine has at `~/Library/Application Support/git-paw/config.toml`. If the maintainer has ever run `git paw add-cli claude-oss /opt/homebrew/bin/claude` (for example, during the v0.5.0 OSS-config dogfood), that CLI is now persistently registered in the global config and bleeds into every subsequent test run.

The drift item proposes two fix candidates: (a) a `GIT_PAW_CONFIG_DIR` env-var override, or (b) an explicit `user_config_path: Option<PathBuf>` parameter on `load_config`. This design picks **(b)** and explains why.

`load_config_from(global_path, repo_root)` already exists and already accepts an explicit global path. It is the right primitive but the wrong UX for tests: every test would need to construct *and pass* a second `TempDir` path, even when it just wants "no global config, please." The `Option<&Path>` parameter on `load_config` is a thin sugar over `load_config_from` that lets tests express "isolate me" with a single `Some(&path)` instead of having to call the lower-level function and manage two paths.

## Goals / Non-Goals

**Goals:**
- The four failing tests in `tests/config_integration.rs` pass on any dev machine, regardless of which custom CLIs are registered globally.
- The fix is discoverable: the parameter name (`user_config_path`) and Rustdoc make it obvious to future test authors that this is how you isolate the global side.
- No production behaviour change. Every existing `load_config(&repo_root)` call becomes `load_config(&repo_root, None)` with byte-identical semantics.
- The fix is minimally invasive: one function signature change, eight production call-site updates (all pass `None`), and N test-site updates.

**Non-Goals:**
- A `GIT_PAW_CONFIG_DIR` env-var override. Considered as alternative A1 below and rejected for the test-isolation use case.
- Sandbox-mode / CI-runner config redirection. If users later want env-var-driven config redirection for sandboxing reasons, it can layer on top of this API without breaking it.
- Renaming `load_config_from` (it has confusing param order). Separate cleanup change.
- Reworking the eight `src/main.rs` call sites to share a helper. Mechanical, out of scope.
- Fixing other pre-existing test-isolation bugs (none known beyond this one).

## Decisions

### D1. Explicit `Option<&Path>` parameter, not an env var

**Choice:** Add `user_config_path: Option<&Path>` to `load_config`. `Some(p)` reads `p` as the global config; `None` falls back to `global_config_path()` (the v0.4 production behaviour).

**Why:**
- **Test-local reasoning.** A test that passes `Some(&tmp.path().join("global.toml"))` is unambiguously isolated. There is no environment to inspect, no parallel-test ordering hazard, no `std::env::set_var` race. An env var would need either `serial_test` gating (forcing serial execution of the test file) or a per-test set/unset wrapper, both of which are louder than just passing a path.
- **Discoverable.** A future test author reading the four updated tests sees the explicit `Some(&path)` and immediately understands "this controls the global-config read." An env var would be invisible at the call site.
- **No precedence question.** With an env var we have to decide: does `GIT_PAW_CONFIG_DIR` override the user's *production* config when they happen to set it for unrelated reasons? Does it interact with XDG variables? The function-parameter approach has no such ambiguity — it is purely a function-call-site mechanism, never read in production code paths.
- **Aligns with existing test escape hatch.** `load_config_from(&global_path, &repo_root)` already takes an explicit global path, and `tests/config_integration.rs::add_custom_cli_with_*` already uses it. The new `Option<&Path>` parameter on `load_config` is the same escape hatch, made ergonomic for the common case where the global config doesn't even exist.

**Alternatives considered:**

- **A1. `GIT_PAW_CONFIG_DIR` env var.** Reads the env var inside `global_config_path()`; if set, joins it instead of `crate::dirs::config_dir()`. *Rejected* — env-var-driven test isolation is famously brittle (cargo runs tests in parallel by default; `std::env::set_var` is `unsafe` since Rust 1.74 in multi-threaded contexts and *will* race against any other test that reads `HOME` / `XDG_CONFIG_HOME` / a sibling env var). The fix would either need `serial_test` (slowing the test suite) or per-test scoped guards (more code than the function-parameter approach).

- **A2. Refactor every test to use `load_config_from(&global_path, &repo_root)` directly.** *Rejected* — `load_config_from` is the right primitive, but the test-side ergonomics are worse: every test has to build *both* paths even when the test doesn't care about the global side. The `Option<&Path>` wrapper expresses "I don't care about the global side, just keep it out of my way" in one token.

- **A3. Make `load_config` always read a `TempDir` in `#[cfg(test)]` builds.** *Rejected* — `cfg(test)` only triggers when *this crate* is being tested; downstream consumers (the integration test binary in `tests/`) compile against the release crate and don't see the `cfg(test)` branch. Doesn't actually fix the failing tests.

- **A4. Move the failing assertions to `src/config.rs::tests` where they can call `load_config_from` directly.** *Rejected* — the integration tests live in `tests/` deliberately, because they test the public crate API as a downstream consumer sees it. Moving them inside the crate hides the bug rather than fixing it.

**Cost:** One signature break in the public crate API. `cargo doc` for `git-paw` will show the new parameter. No downstream consumer is known, so the source-break risk is theoretical; if it bites someone, the migration path is `load_config(repo)` → `load_config(repo, None)`.

### D2. Parameter type is `Option<&Path>`, not `Option<PathBuf>`

**Choice:** `user_config_path: Option<&Path>`.

**Why:**
- Matches the existing convention in the file: `load_config_from(global_path: &Path, repo_root: &Path)`, `repo_config_path(repo_root: &Path) -> PathBuf`. `&Path` is preferred at function boundaries; `PathBuf` is only used when ownership is required (struct fields, returned values).
- Callers in tests already have a `&Path` via `tmp.path()` or `path_buf.as_path()`; no extra `.clone()` or `.to_path_buf()` allocations needed.
- Cheap to convert internally with `p.to_path_buf()` when the loader needs an owned value to pass to `load_config_from`.

**Alternatives considered:** `Option<PathBuf>` (forces caller to allocate even when they already have a `&Path`), `impl Into<Option<&Path>>` (cute but unnecessary; the explicit `Option` is fine).

### D3. Update all `load_config(tmp.path())` calls in `tests/config_integration.rs`, not just the four failing today

**Choice:** Every `load_config(tmp.path())` call in `tests/config_integration.rs` (lines 31, 55, 77, 114, 149, 177, 191, 406 in the v0.5.0 codebase) becomes `load_config(tmp.path(), Some(&tmp.path().join("global.toml")))` (or equivalent isolated path that doesn't exist, so the global side returns defaults).

**Why:**
- The four currently-failing tests are the only *observed* leakage today, but the other call sites are latent regressions: a future change that adds an assertion on `config.clis` or `PawConfig::default()` equality would trip them immediately. Fixing them all now leaves the test file consistent and removes the trap.
- The migration cost per call site is one extra argument; the savings (no future debugging session re-discovering this drift item) are large.

**Alternatives considered:** Only fix the four failing tests. *Rejected* — leaves a known trap for the next contributor.

### D4. New unit test lives in `src/config.rs::tests`, not `tests/config_integration.rs`

**Choice:** The new unit test for the override parameter lives next to `load_config` itself in `src/config.rs::tests`.

**Why:**
- It's an API-contract test (the function's signature semantics), not a workflow test. Unit tests are the natural home.
- It can use `tempfile` cleanly inside `#[cfg(test)]` without needing the `tests/` directory's per-file overhead.
- The integration tests in `tests/config_integration.rs` are *consumers* of the override — they exercise it implicitly. The unit test is the one that asserts the override actually controls the read independently of `global_config_path()`.

### D5. No deprecation shim for the old signature

**Choice:** The signature change is hard — there is no `load_config_v2` or `#[deprecated]` overload. Every call site is updated in the same commit.

**Why:**
- The public crate API is small (single binary, no known external consumers).
- A deprecation shim doubles the API surface and adds a maintenance tax to delete later.
- `cargo build` immediately surfaces every missing-argument call site; the migration is mechanical and complete in one pass.

If a downstream consumer is later identified, the migration is `load_config(repo)` → `load_config(repo, None)` (no behaviour change).

## Risks / Trade-offs

- **[Source-break for unknown external consumers of `config::load_config`]** → Mitigation: documented in the changelog with the trivial migration (`None` for v0.4 behaviour). The crate's documented API surface is "for the binary's internal use"; if external consumers exist, they accepted that contract.

- **[Tests pass the wrong path and silently shadow the override]** → Mitigation: the new unit test (D4) proves both branches of the `Option` work as documented. Test-site reviewers can grep for `load_config(.*, Some(` to verify every isolated test does the right thing.

- **[Future env-var override conflicts with the parameter]** → Mitigation: if a future `GIT_PAW_CONFIG_DIR` env var is added, it lives inside `global_config_path()` and is shadowed by `Some(path)` on `load_config`. Precedence is: explicit `Some(path)` → env var → `crate::dirs::config_dir()`. The function-parameter design composes cleanly with future env-var layering.

- **[Mechanical update misses one of the 8 `src/main.rs` call sites and CI silently passes]** → Mitigation: `cargo build` fails on every missed call site because the signature is now (`&Path`, `Option<&Path>`) and the missing argument is a compile error. There is no silent failure mode.

- **[`load_config_from` and `load_config` have similar but distinct semantics, increasing reader confusion]** → Acknowledged. `load_config_from(global_path, repo_root)` takes two required paths; `load_config(repo_root, Option<global>)` takes one required path plus an optional override. The Rustdoc on both is updated to cross-reference the other. The naming-cleanup follow-up is mentioned in `proposal.md`'s "Not in scope".

## Migration Plan

This is a function signature change + mechanical caller updates. No data, no config, no schema changes.

1. **Code change** in `src/config.rs::load_config` — add `user_config_path: Option<&Path>`. Body branches on the option (`Some(p) => p.to_path_buf()`, `None => global_config_path()?`). Update the Rustdoc to document the new parameter and the `None` → v0.4-default contract.

2. **Production call-site updates** in `src/main.rs` — 8 sites (lines 72, 230, 340, 1094, 1326, 1397, 1748, 1830). Each becomes `config::load_config(&repo_root, None)`. Pure mechanical change.

3. **Test call-site updates** in `tests/config_integration.rs` — every `load_config(tmp.path())` becomes `load_config(tmp.path(), Some(&tmp.path().join("global.toml")))` (or equivalent; the override path is intentionally an unused `TempDir`-rooted path so the global side returns defaults).

4. **New unit test** in `src/config.rs::tests` — asserts:
   - `Some(&existing_global_with_CLI)` causes that CLI to appear in the merged result.
   - `Some(&nonexistent_path)` returns defaults (no leakage from the dev machine's real global config).
   - `None` falls back to `global_config_path()` (covered by behaviour, not a new test — the existing 8 production call sites and the v0.4 test suite already exercise this branch).

5. **Rollback** — revert the signature change and the call-site updates. The four failing tests start failing again on dev machines with registered global CLIs.

No flag, no opt-in. The fix is universally beneficial (`None` ≡ v0.4 behaviour; production paths unchanged).

## Open Questions

- *Should the new parameter be `user_config_path: Option<&Path>` or `global_config_path_override: Option<&Path>`?* Both name the same thing; the spec calls it "the user-level / global config." The proposal uses `user_config_path` because it matches the drift item's vocabulary ("user-level config under `dirs::data_dir()`"). The implementer MAY rename it if reviewer feedback prefers `global_config_path_override` — the spec scenarios refer to the override by behaviour, not by parameter name.

- *Should this same Option-on-`load_config` pattern also be added to `add_custom_cli` / `remove_custom_cli`?* No — those already have explicit `_to` / `_from` variants (`add_custom_cli_to(path, …)`, `remove_custom_cli_from(path, …)`) that the test suite uses directly. The asymmetry exists because `load_config` is called everywhere in production but `add_custom_cli` is called from exactly one place (the CLI handler in `cmd_add_cli`).

- *Will a future `GIT_PAW_CONFIG_DIR` env var conflict with this API?* No. Precedence becomes: explicit `Some(path)` → env var (when added) → `crate::dirs::config_dir()`. The function-parameter override always wins because it's resolved before `global_config_path()` is called.
