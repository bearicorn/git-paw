## 1. Failing checks first

- [ ] 1.1 New `tests/production_panic_free.rs`: `crate_roots_enable_unwrap_and_expect_lints` (`include_str!` `src/lib.rs` and `src/main.rs`; assert each has a crate-level `#![warn(` / `#![deny(` attribute naming both `clippy::unwrap_used` and `clippy::expect_used`) and `clippy_toml_exempts_test_code` (read `clippy.toml`; assert `allow-unwrap-in-tests = true` and `allow-expect-in-tests = true`) — both SHALL fail before the change
- [ ] 1.2 Same file: `add_without_branch_or_spec_errors_without_panic` and `replay_without_branch_or_list_errors_without_panic` (`assert_cmd` inside a temp git repo with `HOME`/`XDG_DATA_HOME` overridden per `core-ci-hygiene`; assert non-zero exit, a usage error on stderr, and no `panicked`)
- [ ] 1.3 Add `static_regexes_compile` unit tests calling every regex accessor in `src/agents.rs`, `src/broker/server.rs`, `src/opsx/role_guard.rs`, `src/specs/speckit.rs`, `src/specs/superpowers.rs`, `src/supervisor/auto_approve.rs`, `src/supervisor/drive.rs`
- [ ] 1.4 Record the baseline: `cargo clippy --all-targets -- -D warnings -W clippy::unwrap_used -W clippy::expect_used` with a scratch `CLIPPY_CONF_DIR` holding the two `allow-*-in-tests` keys; expect exactly the 20 `src/` findings listed in the proposal (plus integration-test noise that D1 scopes out)

## 2. Static-regex helper

- [ ] 2.1 Create `src/static_regex.rs` (`//!` module doc; `pub(crate) fn static_regex(pattern: &'static str) -> Regex` with a `///` doc comment, a function-scoped `#[allow(clippy::expect_used)]` and the unreachability comment from design D2; unit test `compiles_a_static_pattern`); declare `mod static_regex;` in `src/lib.rs`
- [ ] 2.2 Route the 14 static compiles through it, keeping each site's `LazyLock`/`OnceLock`: `src/agents.rs` (`SUPERVISOR_PID_REGEX`, `LAST_VERIFIED_COMMIT_REGEX`; drop the now-stale "panic-surface" comment), `src/broker/server.rs` (agent-id + placeholder accessors), `src/opsx/role_guard.rs` (archive-message, `archive_move_re`, `spec_addition_re`), `src/specs/speckit.rs` (phase-heading, incomplete-task, complete-task), `src/specs/superpowers.rs` (task heading), `src/supervisor/auto_approve.rs` (`file_prompt_regex`, option-line accessor) and `src/supervisor/drive.rs` (paste-buffer accessor) — the last three via `claude_prompt_profile().<field>.as_str()`

## 3. Invariant sites

- [ ] 3.1 `src/specs/speckit.rs`: make `ensure_implicit_phase` return the current phase index; remove both `current_phase_idx.expect(..)`; `phase_less_file_uses_implicit_phase` and `current_phase_handles_implicit_phase` stay green
- [ ] 3.2 `src/coordination/inventory.rs` `get_or_refresh`: restructure per design D3 (no `expect`; previous snapshot kept when `refresh` fails); remove the `# Panics` doc section; add a unit test that a failing refresh on a stale cache returns `Err` and leaves the prior snapshot readable if none covers it
- [ ] 3.3 `src/mcp/query/learnings.rs` `parse`: replace `map.get_mut(section).expect(..)` with `if let Some(entries) = map.get_mut(section)`; existing `parses_sections_and_entries` stays green

## 4. clap-guarantee sites

- [ ] 4.1 `src/commands/add.rs` `cmd_add`: `branch_arg.ok_or_else(|| PawError::BranchError(..))?`
- [ ] 4.2 `src/commands/replay.rs` `cmd_replay`: `branch.ok_or_else(|| PawError::ReplayError(..))?`

## 5. Enforcement

- [ ] 5.1 Add `#![warn(clippy::unwrap_used, clippy::expect_used)]` to `src/lib.rs` (beside `#![warn(missing_docs)]`) and to the top of `src/main.rs`
- [ ] 5.2 Create `clippy.toml` at the repo root with `allow-unwrap-in-tests = true` and `allow-expect-in-tests = true` (do NOT add the lints to `[lints.clippy]` in `Cargo.toml` — design D1)
- [ ] 5.3 Prove the gate bites: temporarily add a `.expect("x")` to a non-test function in `src/`, run `just lint; echo $?`, confirm non-zero with `clippy::expect_used`, then revert
- [ ] 5.4 `grep -rn "allow(clippy::expect_used)\|allow(clippy::unwrap_used)" src/` returns only the helper

## 5b. Binary-crate unit tests run by default (GP-52)

- [ ] 5b.1 Remove `test = false` from the `[[bin]]` entry in `Cargo.toml` — it has been there since the first commit, so the 60+ `#[test]` functions in `src/main.rs` and `src/commands/*.rs` never ran under `cargo test`, `just check`, or CI (they pass today when run explicitly with `cargo test --bin git-paw`)
- [ ] 5b.2 Run `cargo test --bin git-paw` and `cargo clippy --all-targets -- -D warnings`; fix any test or lint failure the newly compiled test modules surface
- [ ] 5b.3 Add a test (e.g. in `tests/`) that parses `Cargo.toml` and asserts the `[[bin]]` entry does not set `test = false`; prove a deliberately failing bin-side test fails a plain `cargo test`, then revert it

## 6. Docs

- [ ] 6.1 `CONTRIBUTING.md` (~:79) and `docs/src/contributing.md` (~:72): the rule is lint-enforced (`clippy::unwrap_used` / `clippy::expect_used` at the crate roots, tests exempt via `clippy.toml`); build-time regex patterns go through `static_regex`
- [ ] 6.2 `docs/src/architecture.md`: add `src/static_regex.rs` to the module diagram/table
- [ ] 6.3 `AGENTS.md` Code Style: append to the "No `unwrap()` or `expect()`" bullet that it is enforced by `just lint` and that static regexes use `static_regex`
- [ ] 6.4 No `--help`, README, or configuration-reference change (no user-facing surface)

## 7. Spec traceability

- [ ] 7.1 Every scenario in `specs/core-project-conventions/spec.md` and `specs/core-ci-hygiene/spec.md` maps to a test that exists and runs under `cargo test`, or to the named gate

## 8. Gates

- [ ] 8.1 `cargo fmt`, then `just lint; echo $?` — confirm `0`
- [ ] 8.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast > target/test.log 2>&1; echo $?` — confirm `0`, then read the log
- [ ] 8.3 `mdbook build docs/`
- [ ] 8.4 `openspec validate production-panic-free --strict`
