## 1. Hidden __dashboard subcommand

- [ ] 1.1 Add `Dashboard` variant to the `Command` enum in `src/cli.rs` with `#[clap(hide = true, name = "__dashboard")]`
- [ ] 1.2 Add `about` and `long_about` strings (even though hidden, for internal docs)
- [ ] 1.3 Add unit test: `__dashboard` parses to `Command::Dashboard`
- [ ] 1.4 Add unit test: `--help` output does not contain `__dashboard`

## 2. Dashboard command handler

- [ ] 2.1 Add a `Command::Dashboard` match arm in `src/main.rs` dispatch
- [ ] 2.2 Check for `$TMUX` env var; if absent, return `PawError` with message "this is an internal command that should only be run by git-paw inside tmux"
- [ ] 2.3 Load config via existing `load_config()`
- [ ] 2.4 Compute `log_path` from session state directory: `session_state_dir()?.join("broker.log")`
- [ ] 2.5 Construct `BrokerState::new_with_log_path(Some(log_path))`
- [ ] 2.6 Call `start_broker(config.broker, state.clone())?` to get `BrokerHandle`
- [ ] 2.7 Call `run_dashboard(state, handle)?` (blocks until `q` or error)
- [ ] 2.8 Return `Ok(())`

## 3. Session state broker fields

- [ ] 3.1 Add optional fields to `SessionData` in `src/session.rs`: `broker_port: Option<u16>`, `broker_bind: Option<String>`, `broker_log_path: Option<PathBuf>`
- [ ] 3.2 Annotate all three with `#[serde(default, skip_serializing_if = "Option::is_none")]`
- [ ] 3.3 Add unit test: session with broker fields round-trips through save/load
- [ ] 3.4 Add unit test: v0.2.0 session JSON (no broker fields) loads with all broker fields as `None`
- [ ] 3.5 Add unit test: session with broker fields serializes them; session without omits them from JSON

## 4. TmuxSessionBuilder set_environment

- [ ] 4.1 Add a `env_vars: Vec<(String, String)>` field to `TmuxSessionBuilder`
- [ ] 4.2 Add `pub fn set_environment(&mut self, key: &str, value: &str) -> &mut Self` method that pushes to `env_vars`
- [ ] 4.3 In the `build()` method, emit `tmux set-environment -t <session> <key> <value>` for each env var BEFORE any `send-keys` commands
- [ ] 4.4 In the dry-run output, include `set-environment` commands
- [ ] 4.5 Add unit test: `set_environment` emits correct tmux command string
- [ ] 4.6 Add unit test: `set-environment` appears before `send-keys` in command queue
- [ ] 4.7 Add unit test: multiple env vars both appear
- [ ] 4.8 Add unit test: dry-run output includes `set-environment`

## 5. Start flow — broker wiring

- [ ] 5.1 In the `start` flow in `src/main.rs`, after loading config and before building the tmux session:
  - Check `config.broker.enabled`
  - If enabled:
    - Insert a `PaneSpec` at position 0 for the dashboard: `worktree = repo_root`, `command = "git paw __dashboard"`, `title = "dashboard"`
    - Call `builder.set_environment("GIT_PAW_BROKER_URL", &config.broker.url())`
    - Set `session_data.broker_port = Some(config.broker.port)`
    - Set `session_data.broker_bind = Some(config.broker.bind.clone())`
    - Set `session_data.broker_log_path = Some(session_state_dir()?.join("broker.log"))`
- [ ] 5.2 Ensure coding agent pane indices shift by 1 when broker is enabled (pane 0 is dashboard, agents start at pane 1)
- [ ] 5.3 Ensure `pipe-pane` log targets, pane titles, and `select-pane` commands use correct pane indices after the shift
- [ ] 5.4 Ensure AGENTS.md generation still works for all agent panes (not for the dashboard pane)

## 6. Stop flow — verify broker shutdown

- [ ] 6.1 Confirm the existing `stop` handler (`tmux kill-session`) requires NO additional code for broker shutdown
- [ ] 6.2 Add integration test: start a session with broker enabled, call `stop`, verify the broker port is freed (attempt to bind to same port succeeds)
- [ ] 6.3 Add integration test: verify `broker.log` received a final flush (file exists and is non-empty if messages were published)

## 7. Purge flow — broker.log cleanup

- [ ] 7.1 In the `purge` handler in `src/main.rs`, after existing cleanup:
  - If `session_data.broker_log_path` is `Some(path)`, call `let _ = std::fs::remove_file(path);`
- [ ] 7.2 Add unit/integration test: purge deletes `broker.log` when it exists
- [ ] 7.3 Add unit/integration test: purge succeeds when `broker.log` does not exist

## 8. Status output — broker info

- [ ] 8.1 In the `status` handler in `src/main.rs`, when the session has broker fields:
  - Compute broker URL from `broker_bind` and `broker_port`
  - Attempt `GET /status` probe (reuse `probe_existing_broker` or a similar sync HTTP call with 500ms timeout)
  - If probe succeeds: display `Broker: <url> (running, N agents)`
  - If probe fails: display `Broker: <url> (not responding)`
- [ ] 8.2 When the session has no broker fields, display no broker line
- [ ] 8.3 Add integration test: status shows broker info when broker is running
- [ ] 8.4 Add integration test: status shows "not responding" when broker is down
- [ ] 8.5 Add integration test: status shows no broker line when broker was never enabled

## 9. Signal handling — SIGHUP

- [ ] 9.1 In the `__dashboard` handler (or in `run_dashboard`), trap SIGHUP alongside SIGINT so `tmux kill-session` does not bypass the final `BrokerHandle::drop`
- [ ] 9.2 Add a comment explaining why SIGHUP is trapped (tmux sends it when killing sessions)

## 10. Integration tests

- [ ] 10.1 Create `tests/broker_integration.rs` (or extend `tests/broker.rs`)
- [ ] 10.2 Test: `git paw start` with broker enabled creates a tmux session where pane 0 title is "dashboard"
- [ ] 10.3 Test: `git paw start` with broker enabled — `tmux show-environment` includes `GIT_PAW_BROKER_URL`
- [ ] 10.4 Test: `git paw start` with broker disabled — no dashboard pane, no `GIT_PAW_BROKER_URL`
- [ ] 10.5 Test: `git paw __dashboard` outside tmux returns an error mentioning "internal command"
- [ ] 10.6 Test: session state JSON includes broker fields when broker enabled
- [ ] 10.7 Test: `git paw stop` frees the broker port
- [ ] 10.8 Test: `git paw purge --force` removes `broker.log`
- [ ] 10.9 Test: `git paw status` with broker shows URL and agent count (or "not responding")

## 11. Quality gates

- [ ] 11.1 `cargo fmt` clean
- [ ] 11.2 `cargo clippy --all-targets -- -D warnings` clean
- [ ] 11.3 `cargo test` — all unit and integration tests pass
- [ ] 11.4 `cargo doc --no-deps` builds without warnings
- [ ] 11.5 `just check` — full pipeline green
- [ ] 11.6 Verify all existing v0.2.0 tests still pass (broker-disabled path unchanged)

## 12. Handoff readiness

- [ ] 12.1 Confirm `src/cli.rs` has the hidden `__dashboard` subcommand, not visible in `--help`
- [ ] 12.2 Confirm `src/session.rs` has optional broker fields with `serde(default)`
- [ ] 12.3 Confirm `src/tmux.rs` has `set_environment` on the builder, commands ordered before `send-keys`
- [ ] 12.4 Confirm `src/main.rs` wires broker conditionally (only when `config.broker.enabled`)
- [ ] 12.5 Confirm pane indexing is correct in both broker-enabled and broker-disabled paths
- [ ] 12.6 Confirm no changes outside `src/main.rs`, `src/cli.rs`, `src/session.rs`, `src/tmux.rs`, `tests/`
- [ ] 12.7 Commit with message: `feat(broker): wire broker and dashboard into session lifecycle`
