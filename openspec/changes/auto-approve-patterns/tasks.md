## 1. Configuration

- [ ] 1.1 Add `AutoApproveConfig` struct to `src/config.rs` with fields `enabled: bool`, `safe_commands: Vec<String>`, `stall_threshold_seconds: u64`, `approval_level: ApprovalLevelPreset`
- [ ] 1.2 Add `approval_level` enum (`Off`, `Conservative`, `Safe`) with serde rename rules
- [ ] 1.3 Embed `AutoApproveConfig` as `Option<AutoApproveConfig>` on `SupervisorConfig` with `#[serde(default, skip_serializing_if = "Option::is_none")]`
- [ ] 1.4 Implement `Default` for `AutoApproveConfig` (`enabled: true`, `safe_commands: []`, `stall_threshold_seconds: 30`, `approval_level: Safe`)
- [ ] 1.5 Implement preset resolution (`Off` forces `enabled = false`; `Conservative` strips `git push` + curl) and threshold floor clamp (min 5s) with stderr warning
- [ ] 1.6 Add unit tests: defaults; preset application; threshold clamp warning; backward-compat parse of v0.3.0 config

## 2. Safe-command classification

- [ ] 2.1 Add `safe_commands` module under `src/broker/` (or `src/supervisor/`) exposing `default_safe_commands()` and `is_safe_command(captured: &str, whitelist: &[String]) -> bool`
- [ ] 2.2 Implement prefix matching with whitespace boundary (`cargo test` matches `cargo test --foo` but not `cargotest`)
- [ ] 2.3 Build effective whitelist as union of defaults + config extras
- [ ] 2.4 Unit tests: each default class; flag variations; non-matching prefixes; config extension; empty `safe_commands` keeps defaults

## 3. Permission-prompt detection

- [ ] 3.1 Add `PermissionType` enum (`Curl`, `Cargo`, `Git`, `Unknown`) to a new `permission_prompt` module
- [ ] 3.2 Implement `detect_permission_prompt(pane_index: usize, session: &str) -> Option<PermissionType>` using `tmux capture-pane -p -t <session>:<pane>`
- [ ] 3.3 Define detection markers as `const &[&str]` (e.g. `"requires approval"`, `"do you want to proceed"`) — make them tweakable via constants for now
- [ ] 3.4 Wire detection so it only runs when stall detection has flagged the agent (one capture per poll tick, not per agent unconditionally)
- [ ] 3.5 Unit tests: each `PermissionType` mapping; absence returns `None`; rate-limit honoured (no capture for healthy agents)

## 4. Automatic approval

- [ ] 4.1 Add `auto_approve_pane(session: &str, pane_index: usize, kind: PermissionType) -> io::Result<()>` that sends `BTab`, `Down`, `Enter` as three separate `tmux send-keys` invocations
- [ ] 4.2 Skip firing when `enabled = false` or `kind == Unknown`
- [ ] 4.3 Publish an `agent.status` (or new variant) broker message tagged `auto_approved` with agent id + matched whitelist entry, **before** dispatching keystrokes
- [ ] 4.4 Surface `Unknown` prompts via `agent.question` so they appear in the dashboard prompts inbox
- [ ] 4.5 Unit tests with mocked tmux invoker: keystroke sequence shape; disabled-config no-op; `Unknown` no-op; pre-fire log entry

## 5. Stall-detection integration

- [ ] 5.1 Add `detect_stalled_agents(state: &BrokerState, threshold: Duration) -> Vec<String>` returning agent ids with status `working` and stale `last_seen`
- [ ] 5.2 In `cmd_supervisor`'s 30s poll loop, call detection → for each stalled agent: capture-pane → classify → auto-approve OR forward to dashboard
- [ ] 5.3 Skip terminal-status agents (done/verified/blocked/committed) — they are not stalled even if quiet
- [ ] 5.4 Integration test (`tests/supervisor_integration.rs`) using a temp tmux session: stalled agent + safe command → keystrokes dispatched; stalled agent + unsafe command → no keystrokes, question published

## 6. Curl allowlist

- [ ] 6.1 Add `setup_curl_allowlist(broker_url: &str, claude_settings_path: &Path) -> Result<(), PawError>` that writes `allowed_bash_prefixes` to `.claude/settings.json` covering `/publish`, `/status`, `/poll`, `/feedback`
- [ ] 6.2 Preserve existing `allowed_bash_prefixes` entries — merge, don't overwrite
- [ ] 6.3 Call `setup_curl_allowlist` from `cmd_supervisor` before agent panes are launched
- [ ] 6.4 Update on broker URL change (re-invoke when session is recreated by `recover_session`)
- [ ] 6.5 Unit tests: fresh write; merge with existing entries; new endpoint addition; invalid existing JSON handled gracefully (error, not panic)

## 7. Documentation and skill template

- [ ] 7.1 Update `assets/agent-skills/supervisor.md` to document auto-approve flow and how to disable it
- [ ] 7.2 Add `[supervisor.auto_approve]` section to `docs/src/configuration.md` (or equivalent reference)
- [ ] 7.3 Add example to `--help` for `git paw start --supervisor` mentioning auto-approve config

## 8. Release readiness

- [ ] 8.1 `just check` (fmt + clippy + tests) green
- [ ] 8.2 `just deny` green
- [ ] 8.3 No new `unwrap()`/`expect()` in non-test code
- [ ] 8.4 All public items documented with `///`
- [ ] 8.5 Dogfood session verifies: stall detection fires, auto-approve dispatches `BTab Down Enter`, curl allowlist prevents first-curl prompt
