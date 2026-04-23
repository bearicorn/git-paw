## 1. ApprovalLevel enum

- [ ] 1.1 Define `pub enum ApprovalLevel { Manual, Auto, FullAuto }` in `src/config.rs`
- [ ] 1.2 Derive `Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default`
- [ ] 1.3 Apply `#[serde(rename_all = "kebab-case")]` so values serialize as `"manual"`, `"auto"`, `"full-auto"`
- [ ] 1.4 Apply `#[default]` on `Auto` variant
- [ ] 1.5 Add doc comments on the enum and each variant describing the security implications

## 2. SupervisorConfig struct

- [ ] 2.1 Define `pub struct SupervisorConfig` with fields: `enabled: bool`, `cli: Option<String>`, `test_command: Option<String>`, `agent_approval: ApprovalLevel`
- [ ] 2.2 Derive `Debug, Clone, PartialEq, Eq, Serialize, Deserialize`
- [ ] 2.3 Apply `#[serde(default)]` on `enabled` and `agent_approval` fields
- [ ] 2.4 Add doc comments on the struct and each field
- [ ] 2.5 Add `supervisor: Option<SupervisorConfig>` field to `PawConfig` with `#[serde(default)]`

## 3. Permission flag mapping

- [ ] 3.1 Implement `pub fn approval_flags(cli: &str, level: &ApprovalLevel) -> &'static str` in `src/config.rs`
- [ ] 3.2 Map: claude + FullAuto → `"--dangerously-skip-permissions"`, codex + FullAuto → `"--approval-mode=full-auto"`, codex + Auto → `"--approval-mode=auto-edit"`, all Manual → `""`, unknown CLI → `""`
- [ ] 3.3 Add doc comments with examples

## 4. Default config generation

- [ ] 4.1 Update `generate_default_config()` to include a commented-out `[supervisor]` section with `enabled`, `cli`, `test_command`, `agent_approval` examples
- [ ] 4.2 Match the style of existing commented sections (`[broker]`, `[specs]`, `[logging]`)

## 5. Init prompts

- [ ] 5.1 In `src/init.rs` (or `src/main.rs` init handler), after existing init steps, prompt: "Enable supervisor mode by default? (y/n)"
- [ ] 5.2 If yes, prompt: "Test command to run after each agent completes (e.g. 'just check', leave empty to skip):"
- [ ] 5.3 Write `[supervisor]` section to the generated config with user's answers
- [ ] 5.4 If no, write `[supervisor]\nenabled = false` to explicitly disable (prevents future prompts during start)

## 6. Config merge rules

- [ ] 6.1 Verify that `supervisor` field follows repo-wins merge semantics (same as `broker`, `specs`, `logging`)
- [ ] 6.2 Add `supervisor | Repo wins` to the merge rules documentation

## 7. Unit tests

- [ ] 7.1 Test: config with no `[supervisor]` section loads as `supervisor = None`
- [ ] 7.2 Test: config with `[supervisor]` containing all fields parses correctly
- [ ] 7.3 Test: config with partial `[supervisor]` (only `enabled = true`) defaults other fields
- [ ] 7.4 Test: invalid `agent_approval = "yolo"` fails to parse
- [ ] 7.5 Test: `SupervisorConfig` round-trips through save and load
- [ ] 7.6 Test: `approval_flags("claude", FullAuto)` → `"--dangerously-skip-permissions"`
- [ ] 7.7 Test: `approval_flags("codex", Auto)` → `"--approval-mode=auto-edit"`
- [ ] 7.8 Test: `approval_flags("codex", FullAuto)` → `"--approval-mode=full-auto"`
- [ ] 7.9 Test: `approval_flags("unknown", FullAuto)` → `""`
- [ ] 7.10 Test: `approval_flags("claude", Manual)` → `""`
- [ ] 7.11 Test: `approval_flags` is deterministic (call twice, same result)
- [ ] 7.12 Test: `generate_default_config()` contains commented `[supervisor]` section
- [ ] 7.13 Test: existing v0.3.0 config (no supervisor section) loads without error

## 8. Quality gates

- [ ] 8.1 `cargo fmt` clean
- [ ] 8.2 `cargo clippy --all-targets -- -D warnings` clean
- [ ] 8.3 `cargo test` — all tests pass (new + existing)
- [ ] 8.4 `cargo doc --no-deps` — no warnings
- [ ] 8.5 `just check` — full pipeline green
- [ ] 8.6 Verify all existing config tests still pass (backward compat)

## 9. Handoff readiness

- [ ] 9.1 Confirm `src/config.rs` exposes `SupervisorConfig`, `ApprovalLevel`, `approval_flags` as public API
- [ ] 9.2 Confirm `PawConfig.supervisor` is `Option<SupervisorConfig>` with `serde(default)`
- [ ] 9.3 Confirm no changes outside `src/config.rs`, `src/init.rs`, and test files
- [ ] 9.4 Commit with message: `feat(config): add [supervisor] config section with approval policy`
