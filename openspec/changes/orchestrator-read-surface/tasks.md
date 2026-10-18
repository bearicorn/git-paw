## 1. Broker: long-poll and progress tracking

- [ ] 1.1 `BrokerState` gains a publish notification (`tokio::sync::Notify`) signalled after every log append, and `last_progress_at`; the progress classifier lives beside the delivery routing in `src/broker/delivery.rs`: progress is an `agent.status` whose status or phase changed, an `agent.artifact` / `agent.verified` / `agent.feedback` / `agent.advanced-main`, or a moved branch tip; a repeated status (whatever its modified files), uncommitted modified-file churn, `agent.question` (the drive loop's escalations and re-escalations) and `agent.learning` are not
- [ ] 1.2 `src/broker/watcher.rs`: read each worktree's `HEAD` commit beside `git status` every tick and mark progress when it changes (the first observation is a baseline, not progress)
- [ ] 1.3 `log` handler: optional `wait` (≤ 60 s cap, 400 on non-numeric); check → await notify-or-timeout → re-check, never holding the state lock across `.await` (`clippy::await_holding_lock` clean); an empty response reports `last_seq = since` with or without `wait` (v0.17 reports `0`, `server.rs:484`)
- [ ] 1.4 `status` handler: `wave {last_progress_unix_secs, idle_seconds, stall_threshold_seconds, stalled}`; `[broker] wave_stall_minutes` (default 30, `0` disables) in `src/config/broker.rs` + `git paw init` commented example
- [ ] 1.5 Server tests for every "GET /log long-polls on the since cursor" (including "An empty response without wait holds the cursor"), "GET /status reports wave staleness" (including "Uncommitted file churn is not progress" and "The drive loop's own escalations and learnings are not progress") and "Wave stall threshold is configurable" scenario, using a fixed clock where timing matters; classifier unit table over every `BrokerMessage` variant

## 2. Readers

- [ ] 2.1 `broker::publish::fetch_log_entries_over_http` gains an optional `since` and `wait`; `fetch_log_over_http` and the MCP intent/conflict readers (`src/mcp/query/{intents,conflicts}.rs`) pass `since = 0` and no wait; `HttpLogObserver` (`src/supervisor/drive.rs:2769`) requests `since=<cursor>` (client-side filter kept as a guard); test "The drive loop reads only new entries" (`broker-host-separation` later moves this reader into its `BrokerClient`)
- [ ] 2.2 `git paw status`: stalled-wave line (human) and `wave` object (`--json`); tests for the three "Status surfaces a stalled wave" scenarios
- [ ] 2.3 Dashboard status-line stalled indicator computed from the wave state of the in-process `BrokerState` the dashboard renders (the same object `/status` reports); render tests for the two "Dashboard shows a stalled-wave indicator" scenarios

## 3. Orchestrator entry points

- [ ] 3.1 MCP `wait_for_events` (read-only, schema, cap, `no_session` degrade) and `get_session_status` `wave` field; add `wait_for_events` to the closed `ALL_ALLOWED_TOOL_NAMES` list in `src/mcp/server.rs` (`registry_is_exactly_the_closed_allowlist` fails otherwise); `tests/mcp_e2e.rs` cases for the `mcp-server` scenarios
- [ ] 3.2 `assets/scripts/sweep.sh` `events <since> [wait]` (JSON out, no keystrokes, no publish); `bash -n`; tests for the three helper scenarios; export-agnosticism audit green (`.git-paw/scripts/sweep.sh` is gitignored and written by `git paw init`: refresh the local copy for dogfooding, nothing to commit)
- [ ] 3.3 `src/cli_setup/facts.rs` (from v0.17 `cli-setup-consent`): add `events` to the granted read-only `sweep.sh` subcommands, as `Bash(.git-paw/scripts/sweep.sh events *)`; tests for the `cli-setup` scenarios ("The allow entries are exactly the grant table" at twelve, "The read-only events subcommand is granted", the grant-table coverage test still enumerating every `sweep.sh` subcommand); doctor's out-of-date setup check fires for a v0.17 record lacking the entry, and clears after `--undo` + `--allowlist` (relies on `setup-cli-adopted-entries`' PP-22 fix, archived before this change)

## 4. Docs and gates

- [ ] 4.1 `docs/src/user-guide/coordination.md` (event surface: cursor, long-poll, wave staleness), `user-guide/mcp.md` (`wait_for_events`), `user-guide/dashboard.md` (stalled indicator), `cli-reference.md` (`status` wave line), configuration reference (`wave_stall_minutes`), `user-guide/cli-setup.md` + `user-guide/security-posture.md` (grant table lists `sweep.sh events`; upgrade re-run of `setup-cli --undo` then `--allowlist`); `mdbook build docs/` succeeds
- [ ] 4.2 Every scenario maps to a named test; `just check` + `just deny` green; full suite green vs merge-base (`cargo test --no-fail-fast`)
