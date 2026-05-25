## 1. Bug A — `cmd_supervisor` SHALL fall back to default `SupervisorConfig`

- [x] 1.1 In `src/main.rs::cmd_supervisor` (~line 817), replace the `config.supervisor.as_ref().ok_or_else(... "supervisor mode enabled but [supervisor] config missing" ...)?` with a fallback pattern. Use a local `let default_supervisor_cfg = SupervisorConfig::default();` plus `let supervisor_cfg = config.supervisor.as_ref().unwrap_or(&default_supervisor_cfg);`. Import `SupervisorConfig` from `git_paw::config`.
- [x] 1.2 Apply the SAME fix in `src/main.rs::recover_supervisor_session` (~line 1627). The hard-error pattern there must be replaced with the default-fallback pattern.
- [x] 1.3 Confirm by grep that no other call site in `src/` hard-errors on `config.supervisor.is_none()`. If any are found, update them too.
- [x] 1.4 Verify the existing `[supervisor].cli > default_cli > error` chain (around `src/main.rs:822-831`) is untouched — the error path is still reached when both CLI sources are missing.
- [x] 1.5 Test: `tests/cli_supervisor_no_config.rs` — `git paw start --supervisor --branches a,b` against a tempdir with `default_cli = "echo"` and no `[supervisor]` section exits 0 and prints the supervisor-session-launched message. Stderr SHALL NOT contain "supervisor mode enabled but [supervisor] config missing".
- [x] 1.6 Test: same setup with NO `default_cli` AND no `[supervisor]` → exits non-zero with the existing "requires either [supervisor].cli or default_cli" error.

## 2. Bug B — Resumed agent panes SHALL start in worktree cwd

- [x] 2.1 Reproduce: write a `tests/cli_recover_cwd.rs` integration test that (a) launches a 2-branch supervisor session against `echo` as the CLI, (b) `git paw stop`s it, (c) `git paw start`s it, (d) reads `tmux display-message -t <session>:0.<pane> -p "#{pane_current_path}"` for each agent pane, (e) asserts the value equals the worktree path. The test SHALL fail today (BEFORE the fix) — confirm that, then move on.
- [x] 2.2 Fix the first-agent split in `src/tmux.rs::build_supervisor_session` (~line 716). The current code does `let first_cmd = format!("cd {} && {}", first.worktree, first.cli_command);`. Replace this by adding `-c <first.worktree>` to the relevant tmux command that creates pane 2 (the first agent area). The `send-keys` then sends only `first.cli_command`, not the `cd && cli` chain.
- [x] 2.3 Fix the bare-session builder's subsequent-pane path in `src/tmux.rs::TmuxSessionBuilder::build` (~line 326-345). The `split-window` SHALL pass `-c <pane.worktree>` and `send-keys` SHALL send only `pane.cli_command` (drop the `let pane_cmd = format!("cd {} && {}", ...)` construction).
- [x] 2.4 Re-run the test from 2.1; it SHALL pass.
- [x] 2.5 Audit other tmux builders (`build`, `build_dashboard_session` if present) for the same `cd <worktree> && <cli>` race pattern and apply the same fix everywhere.
- [x] 2.6 Unit tests in `src/tmux.rs::tests`: assert the constructed tmux command sequence for a 2-branch session contains `-c <branch-a-worktree>` and `-c <branch-b-worktree>` in the agent splits AND does NOT contain any `cd ... &&` substring in the `send-keys` arguments.

## 3. Drift 68 §8c — Bundle `sweep.sh` and install via `git paw init`

- [x] 3.1 Generalize the existing user-local `~/.claude-oss/scripts/paw-supervisor-sweep.sh` (or its equivalent) into `assets/scripts/sweep.sh`. Strip every hardcoded value:
  - Session name: read from `<repo>/.git-paw/sessions/*.json`'s `session_name` field (most recently modified if multiple).
  - Repo parent / worktree paths: read from each session JSON entry's `worktree_path` field.
  - Broker URL: read `[broker].port` from `<repo>/.git-paw/config.toml` (default 9119), construct `http://127.0.0.1:<port>`.
  - Test command: read `[supervisor].test_command` from config; commands depending on it SHALL gracefully no-op when unset.
- [x] 3.2 Subcommands the script SHALL implement (one-line CLI usage shown):
  - `snapshot` — capture-pane tail of every coding-agent pane (returns one block per pane).
  - `capture <pane>` — single-pane full tail-50 capture.
  - `approve <pane>` — `Down` + `Enter` to the pane (sticky "Yes, don't ask again" choice).
  - `status` — broker `/status`, one line per agent, filtering phantoms (see §3.3).
  - `status --all` — broker `/status` with no filter.
  - `worktrees-status` — count of uncommitted files per agent worktree.
  - `inbox` — supervisor inbox payloads (`agent.question` / `agent.feedback` / `agent.blocked` only).
  - `feedback-gate <agent-id> <gate-name> <message text...>` — publish `agent.feedback` with the bracketed gate-name prefix on each error.
  - `verified <agent-id> <message text...>` — publish `agent.verified`.
  - `status-publish <message text...>` — publish an `agent.status` from `agent_id = "supervisor"` to mark a sweep action.
- [x] 3.3 `status` subcommand SHALL filter `agent_id` values that don't match `^(supervisor|feat[-/].+)$` from the rendered output. When phantoms exist, the script SHALL print a single trailing line summarizing them (e.g. `phantoms (use --all to show): a, b`). `--all` flag bypasses the filter and suppresses the summary line.
- [x] 3.4 In `src/init.rs::run_init`, after the existing config.toml write, embed the script content via `include_str!("../../assets/scripts/sweep.sh")` and write it to `<repo>/.git-paw/scripts/sweep.sh`. Set executable mode `0o755` on Unix via `std::os::unix::fs::PermissionsExt`.
- [x] 3.5 The script SHALL be re-installed on subsequent `git paw init` invocations (overwriting prior content).
- [x] 3.6 Rewrite `assets/agent-skills/supervisor.md` so every multi-pane `tmux capture-pane` example, every `for p in ...` loop, and every raw curl POST to `/publish` (for the verified / feedback / status families) invokes the corresponding `sweep.sh` subcommand. The supervisor's first-curl bootstrap MAY remain as a direct curl (since the session JSON does not yet exist on first publish).
- [x] 3.7 Replace any `<agent-id>` / `<your question>` / `<your specific question>` literal placeholder syntax in the skill's remaining curl examples with `__FILL_IN__`-shaped placeholders so accidental submit produces a broker-side rejection (per §4) or an obvious string in the dashboard rather than phantom agents.
- [x] 3.8 Integration test `tests/cli_init_writes_sweep_script.rs`: run `git paw init` in a `TempDir`, assert `<tempdir>/.git-paw/scripts/sweep.sh` exists and is executable. Read the first line; assert it starts with `#!/usr/bin/env bash` (or `#!/bin/bash`).
- [x] 3.9 Integration test `tests/sweep_sh_session_discovery.rs`: write a `.git-paw/sessions/paw-myproject.json` with a known session name; run `<tempdir>/.git-paw/scripts/sweep.sh status` against a mock HTTP server on port 9119; assert the script targets the discovered session name (not hardcoded `paw-git-paw`).
- [x] 3.10 Skill-content test in `src/skills.rs::tests`: the resolved supervisor skill content SHALL contain at least one invocation of `.git-paw/scripts/sweep.sh snapshot`, `.git-paw/scripts/sweep.sh capture`, `.git-paw/scripts/sweep.sh approve`, `.git-paw/scripts/sweep.sh verified`, `.git-paw/scripts/sweep.sh feedback-gate`. The content SHALL NOT contain the string `for p in 2 3 4 5`.

## 4. Drift 69 §8d — Broker validation in `src/broker/server.rs::publish`

- [x] 4.1 Add two compiled regexes via `std::sync::OnceLock<regex::Regex>`: `AGENT_ID_RE = ^(supervisor|feat/[a-z0-9][a-z0-9-]+|feat-[a-z0-9][a-z0-9-]+)$` and `PLACEHOLDER_RE = ^<.*>$`.
- [x] 4.2 In the `publish` handler, after deserialization but before persistence, validate `BrokerMessage`'s top-level `agent_id`. On regex mismatch return HTTP 400 with body `{"error":"invalid agent_id","value":"<the offending value>","detail":"agent_id must be 'supervisor' or match feat-{name} / feat/{name}"}`.
- [x] 4.3 Validate placeholder syntax on payload string fields. For each variant of `BrokerMessage`:
  - `Status { payload }`: check `payload.message`
  - `Feedback { payload }`: check `payload.message`; iterate `payload.errors[]` and check each
  - `Blocked { payload }`: check `payload.needs`
  - `Question { payload }`: check `payload.question`
  On placeholder match, return HTTP 400 with `{"error":"field looks like an unfilled placeholder","field":"<field-name>","value":"<offending value>","detail":"substitute the real value before publishing"}`.
- [x] 4.4 Update every existing `/publish` test caller in `tests/` to use a valid `agent_id` (e.g. `feat-x`, `feat-test`, `supervisor`). Audit list: `tests/broker_integration.rs`, `tests/conflict_detection_integration.rs`, `tests/learnings_mode_integration.rs`, `tests/e2e_*.rs`, `tests/dashboard_render.rs`, `tests/terminal_status_integration.rs`. Update text fixtures as needed.
- [x] 4.5 Unit test `agent_id_rejects_single_letter`: POST `agent_id = "a"`; assert HTTP 400 and body contains `invalid agent_id`.
- [x] 4.6 Unit test `agent_id_rejects_placeholder`: POST `agent_id = "<agent-id>"`; assert HTTP 400.
- [x] 4.7 Unit test `agent_id_rejects_empty`: POST `agent_id = ""`; assert HTTP 400.
- [x] 4.8 Unit test `agent_id_accepts_supervisor`: POST `agent_id = "supervisor"`; assert success (200 or 204).
- [x] 4.9 Unit test `agent_id_accepts_feat_dash` and `agent_id_accepts_feat_slash`: POST `feat-test-branch` and `feat/test-branch`; both succeed.
- [x] 4.10 Unit test `payload_question_rejects_placeholder`: POST `agent.question` with `payload.question = "<your specific question>"`; assert HTTP 400 with body containing `placeholder` and `question`.
- [x] 4.11 Unit test `payload_question_accepts_real_content`: POST `agent.question` with `payload.question = "Should we use bcrypt?"`; assert success.
- [x] 4.12 Integration test `phantom_agents_cannot_appear_in_status`: launch a broker, POST `{agent_id:"a"}`, GET `/status`, assert `a` is NOT in the agents list.

## 5. Quality gates

- [x] 5.1 `cargo fmt` and `cargo clippy --all-targets -- -D warnings` clean.
- [x] 5.2 `just check` green (with `GIT_PAW_ALLOW_LIVE_SESSION=1` if running alongside an active dogfood session; document the env-var in the test's leading doc comment if it's a hard requirement).
- [x] 5.3 `mdbook build docs/` clean.
- [x] 5.4 `openspec validate supervisor-bugfixes-v0-5-x --strict` passes.
- [x] 5.5 `just deny` clean.
- [x] 5.6 Coverage gate (>= 80% on logic) preserved or improved.

## 6. Documentation

- [x] 6.1 `docs/src/user-guide/supervisor.md` — add a short subsection naming `.git-paw/scripts/sweep.sh` and the canonical subcommands, with a code-block example.
- [x] 6.2 `docs/src/user-guide/init.md` (or wherever `git paw init` is documented) — note the new script written on init.
- [x] 6.3 `--help` text on `git paw init` SHALL mention that `.git-paw/scripts/sweep.sh` is installed.
- [x] 6.4 Rustdoc on `src/init.rs::run_init` SHALL document the script-install behavior.
- [x] 6.5 No CHANGELOG.md edits (autogenerated by `git cliff` at release prep).

## 7. Release notes (in archive's release-notes.md, NOT CHANGELOG)

- [x] 7.1 Bug A fix: `git paw start --supervisor` without `[supervisor]` config now works (was: hard error).
- [x] 7.2 Bug B fix: resumed coding-agent panes now start in their worktree cwd (was: started in repo root due to send-keys race).
- [x] 7.3 New: `.git-paw/scripts/sweep.sh` installed by `git paw init` provides snapshot / capture / approve / status / inbox / feedback-gate / verified / status-publish subcommands.
- [x] 7.4 New: broker `/publish` validates `agent_id` and payload placeholder syntax; phantom agents from copy-pasted curl examples are now rejected at the API boundary with HTTP 400.
