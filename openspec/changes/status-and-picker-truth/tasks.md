## 1. Liveness: separate "agent panes exited" from "session absent" (DS-9)

- [ ] 1.1 Failing test first: in `src/tmux/tests.rs`, rename `session_liveness_for_reports_stale_when_all_agent_panes_are_bare_shells` (~:2581) to `..._reports_agents_exited_...` and assert `SessionLiveness::AgentsExited`. Keep `session_liveness_for_reports_stale_when_the_session_does_not_exist` (~:2621) asserting `Stale`. Together they cover the scenario "Exited agent panes are a distinct liveness outcome from an absent session"
- [ ] 1.2 Add `SessionLiveness::AgentsExited` (doc comment: tmux session exists, launch grace period over, no agent pane runs a non-shell process) in `src/tmux/session.rs`. Return it from `session_liveness_for_with` (~:170) where it now returns `Stale`. Update the enum doc and the `session_liveness_for` doc. Leave `classify_liveness` and `session_liveness_with` unchanged
- [ ] 1.3 In `src/session.rs` `DisplayStatus::from_receipt` (~:92), add an explicit `(_, L::AgentsExited) => Self::Stopped` arm and update its doc comment. Add `AgentsExited` to the receipt × liveness tests (~:1088-1145): `Active` and `Paused` give `Stopped`, never `Stale`
- [ ] 1.4 Grep `SessionLiveness` and `session_liveness_for` across `src/`. Confirm that every `matches!(…, Alive)` site (`commands/start.rs` ~:62/~:385, `commands/supervisor.rs` ~:386, `commands/pause.rs` ~:34, `commands/add.rs` ~:46) needs no change, and that the plain-probe sites (`main.rs` `invalidate_if_stale` ~:1314, `cmd_purge_stale` ~:1383, `commands/attach.rs` ~:40) still call `session_liveness`

## 2. Status rendering

- [ ] 2.1 In `src/commands/status.rs`, set `tmux_running` to `matches!(liveness, Alive | AgentsExited)`. When liveness is `AgentsExited`, print `  ↳ agent panes exited — run 'git paw start' to recover them` under the `Status:` line. Leave the stale hint as it is (it prints only for `DisplayStatus::Stale`). Update the module comment that says the dead-panes session is "not reported healthy" so it reflects the new mapping
- [ ] 2.2 Rewrite `tests/agent_pane_liveness_e2e.rs`. For the real tmux session with bare-shell panes and `created_at` 120s back:
  - `status --json` contains `"status":"stopped"` and `"tmux_running":true`, and not `"stale"`;
  - plain `status` shows `🟡 stopped`, `agent panes exited`, `git paw start` and `Tmux:    running`, and does not contain `no longer exists`.
  These cover the scenarios "A session whose agent panes all exited reports Stopped", "Status shows exited agent panes as stopped with tmux running" and "JSON status for exited agent panes reports stopped with tmux running"
- [ ] 2.3 In the same file, add a test that `purge --stale` run in that repo exits 0, prints the nothing-to-purge message, and leaves the receipt file in place (scenario "purge --stale leaves a session whose agent panes exited")
- [ ] 2.4 Add or extend an e2e in `tests/session_stale_hygiene.rs` for an absent tmux session: `status --json` gives `"status":"stale"` and `"tmux_running":false` (scenario "An absent tmux session still reports stale with tmux not running")
- [ ] 2.5 Confirm the existing `session_liveness_for_reports_alive_when_an_agent_pane_is_live` still covers "A session with at least one live agent pane remains Active"

## 3. Doctor hygiene

- [ ] 3.1 Failing test first: in `src/doctor_tests.rs`, add `exited_agent_session_warns_with_the_start_remedy`. With `HygieneProbe { exited_agent_sessions: vec!["paw-demo"], .. }` the `session state` check is ⚠, its detail names `paw-demo` and says the agent panes exited, its remedy contains `git paw start`, and the remedy does not contain `purge --stale`. Keep `stale_session_warns_with_the_purge_remedy` unchanged
- [ ] 3.2 Add `exited_agent_sessions: Vec<String>` (with a doc comment) to `HygieneProbe` in `src/doctor.rs`. Fill it in `probe_hygiene` (~:1441) when the probe returns `AgentsExited` for an `active` or `paused` receipt. In `check_hygiene` (~:902), when there are no stale sessions but there are exited-agent sessions, emit the ⚠ described in design D4. Update the full-field fixture at `src/doctor_tests.rs` ~:90
- [ ] 3.3 Update the doctor half of `tests/agent_pane_liveness_e2e.rs`: `session state` is `warn`, its detail names the session and says the agent panes exited, and its remedy names `git paw start` and not `purge --stale` (scenario "Session whose agent panes exited")

## 4. default_cli pre-selects in the bare-start picker (DS-8)

- [ ] 4.1 Failing tests first, in `src/interactive/resolver.rs` tests, using the existing `last_select_cli_default` capture on `TrackingPrompter`:
  - `run_selection_passes_default_cli_to_uniform_picker`: `Some("beta")` reaches `select_cli` and the branches map to the returned CLI;
  - `run_selection_still_prompts_when_default_cli_set`: the mode prompt and the picker both run, and the prompter's choice wins;
  - `run_selection_without_default_cli_passes_none`;
  - `cli_flag_overrides_default_cli_without_prompting`: `--cli alpha` plus default `beta` gives no prompts and every branch maps to `alpha`
- [ ] 4.2 Add a trailing `default_cli: Option<&str>` parameter to `run_selection` (~:268), with a doc line saying it pre-selects in the uniform-mode picker and never skips it. Pass it to `prompter.select_cli` (~:299). Update the 13 existing test call sites to pass `None`
- [ ] 4.3 Pass `config.default_cli.as_deref()` from `src/commands/start.rs` (~:137) and `src/commands/supervisor.rs` (~:464)
- [ ] 4.4 Per-branch mode: add `default: Option<&str>` to `Prompter::select_cli_for_branch` (`src/interactive/mod.rs`) with the same doc semantics as `select_cli`; in `TerminalPrompter` (`src/interactive/prompter.rs` ~:67) replace `.default(0)` with the name → index lookup `select_cli` uses (factor one helper); update `TrackingPrompter` (~:486, capture the per-branch default) and `CancelOnSpecsPrompter` (~:1123); pass `default_cli` from `run_selection`'s `CliMode::PerBranch` arm. Tests: `run_selection_passes_default_cli_to_every_per_branch_picker` (scenario "default_cli pre-selects in every per-branch picker") and a `TerminalPrompter`-free unit test of the shared index helper (scenario "Per-branch picker starts on a configured default": listed name → its index, `None`/unlisted → 0)
- [ ] 4.5 Supervised launch honours the picker (DS-8): in `src/commands/supervisor.rs`'s no-`--branches`/no-spec arm (~:455-472), keep `selection.mappings` and insert each `(branch, cli)` into the per-branch override map `effective_agent_cli` consults (`spec_cli_overrides`), instead of `map(|(b, _)| b)`. Keep the `--branches` arm on the `agent_cli` fallback. Extract the arm's branch→CLI resolution into a small pure function if that is what makes it unit-testable with a `TrackingPrompter`
- [ ] 4.6 Tests for *Supervised launch honours the CLIs chosen in the interactive picker*: uniform pick `beta` with `default_cli = "alpha"` launches `feat/a` with `beta`; per-branch picks map `feat/a → alpha`, `feat/b → beta`; `--branches feat/a` without `--cli` still uses `default_cli` and shows no picker. Assert on the dry-run plan (`Agent CLI` / per-branch lines) or the extracted pure function; add one PTY e2e (`tests/cli_prompt_matrix.rs`, `#[serial]`, gated on `pty::tmux_available()`) for the uniform case with `--supervisor --dry-run`
- [ ] 4.7 PTY e2e in `tests/cli_prompt_matrix.rs`, `#[serial]`, gated on `pty::tmux_available()`. Write a config with two custom CLIs whose `command` is on PATH and set `default_cli` to the one that sorts second by display name. Run `start --branches feat/a --dry-run`, wait for `CLI assignment mode` and press Enter, wait for `Select AI CLI for all branches` and press Enter, then wait for the pane to show `feat/a → <default_cli>` (scenario "Bare git paw start pre-selects the configured default CLI")

## 5. Docs

- [ ] 5.1 `docs/src/cli-reference.md` `git paw status` (~:345-357): in the status table, `🟡 stopped` also covers "the tmux session is up but every agent pane exited". Mention the `git paw start` hint, and say that `tmux_running` is `true` in that case and `🔴 stale` / `"stale"` means only that the tmux session is gone
- [ ] 5.2 `docs/src/user-guide/doctor.md` Hygiene row (~:93): split the case. A tmux session that is gone gets ⚠ with `purge --stale`. A session whose agent panes all dropped back to a bare shell gets ⚠ with `git paw start`
- [ ] 5.3 `docs/src/user-guide/session-lifecycle.md` (~:160-171): confirm it still reads correctly (dead-panes sessions are recovered by `start`; only a vanished tmux session is `🔴 stale`). Fix it if not
- [ ] 5.4 `docs/src/configuration/README.md`: change the `default_cli` setting (~:145-151) from "skip the CLI picker" to "pre-selected in the interactive CLI picker when `--cli` is not passed (you can still choose another)", and the full-config example comment (~:18) to match. Confirm README.md ~:253 ("pre-select in the interactive picker") and `generate_default_config` (`src/config/mod.rs` ~:866) already agree
- [ ] 5.5 Grep `docs/src` and README for any other claim that dead agent panes show as `stale` or that `default_cli` skips the picker, and fix each
- [ ] 5.6 `docs/src/configuration/README.md` / `docs/src/user-guide/supervisor.md`: `default_cli` also pre-selects in per-branch mode, and a supervised launch without `--branches` uses the CLIs picked in the picker (the `--cli` > `default_cli` > supervisor-CLI fallback applies only when no picker is shown)

## 6. Gates

- [ ] 6.1 Verify every scenario in `specs/` maps to a test named above
- [ ] 6.2 `cargo fmt`, then `just lint`
- [ ] 6.3 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast > test.log 2>&1; echo $?` — the exit code must be 0 (read it, do not pipe it)
- [ ] 6.4 `mdbook build docs/`
- [ ] 6.5 `openspec validate status-and-picker-truth --strict`
