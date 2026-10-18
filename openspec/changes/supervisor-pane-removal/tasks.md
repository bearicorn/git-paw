## 1. Compiler-guided deletion of the supervisor role

- [ ] 1.1 Delete `SurfaceRole::Supervisor` (from `supervisor-role-typing`) and the supervised layout's reserved supervisor index; the supervised `SessionLayout` becomes dashboard 0, agents 1..N (legacy `dashboard_pane = Some(1)` sessions keep agents from 2 per design D6); fix every compile error under `src/supervisor/`
- [ ] 1.2 `drive.rs`: remove `orchestrator_pane_index`, `orchestrator_present`, `hand_to_orchestrator`, `orchestrator_ready`, `ORCHESTRATION_NUDGE_INTERVAL`, `orchestration_nudge_text`, `SweepContext.orchestrator` / `orchestrator_settle_delay`, `DriveConfig.supervisor_cli` / `orchestration_nudge_interval` (and its `GIT_PAW_DRIVE_ORCHESTRATION_NUDGE_MS` test override), `PaneSweepOutcome::OrchestratorCliUnavailable`, `DriveOutcome::CliUnavailable` (the exit summary's outcome list drops `cli-unavailable`); keep `Escalation::handoff_text`, `question_handoff_text`, `merge_handoff_text` as pure renderers; the loop sweeps `Coding` surfaces only
- [ ] 1.3 CLI-unavailability: coding-only scan; dashboard text never scanned; summary states the orchestrator's CLI is not monitored (tests: the "CLI unavailability is an explicit non-success exit" scenarios, including "A banner quoted by the dashboard does not trigger")
- [ ] 1.4 Delete/rewrite drive-loop unit tests for pane-0 coverage (the `supervisor` agent-id escalation and pane-0 buffered-input submit), orchestrator hand-off (including the permission-prompt relay hand-off) and orchestration nudge; add "escalation is recorded as a broker review item only", "merge decision is recorded, not injected", "a permission-prompt escalation is never typed into a surface" and "a prompt drawn on the dashboard is not escalated"
- [ ] 1.5 Gate: `cargo build` clean with the variant gone; drive-loop unit suites green

## 2. One surface-eligibility rule for every keystroke sender (GP-46)

- [ ] 2.1 `drive.rs`: prompt detection, escalation, nudges, stale-input recovery and buffered-input submits act only on surfaces resolved as `Coding` through `SessionLayout`; the per-pane claim is kept
- [ ] 2.2 Tests: the drive loop sends nothing to the dashboard in bare and supervised layouts, nor to an unresolved surface; it still submits buffered input on a coding surface (the `approval-auto` *Keystrokes reach only coding-agent surfaces* scenarios)
- [ ] 2.3 `assets/scripts/sweep.sh` `cmd_approve`: refuse any pane not listed as a coding agent's `pane_index` in the discovery roster, fall back to refusing pane 0 with no roster; the v0.17.0 live re-confirm and option-1 guard are unchanged (`1` + `Enter`); `bash -n`; `tests/sweep_sh_approve*.rs` cover the four `broker-agent-helper` scenarios plus the broker-less first-agent scenario; refresh the local, gitignored `.git-paw/scripts/sweep.sh` copy (`git paw init`) before dogfooding

## 3. Launch flow and orchestrator context

- [ ] 3.1 `commands/supervisor.rs`: drop the supervisor `PaneSpec`, `resolve_supervisor_flags` (and its second caller, `commands/recover.rs:290`), `build_supervisor_framing`, the framing injection and the required-orchestrator-CLI error (agent CLI chain `--cli` > `default_cli` > `[supervisor].cli`, error only when none resolves; rewrite `tests/cli_supervisor_no_config.rs::supervisor_without_section_or_default_cli_still_errors` for the new message); keep the no-self-publish guard; print the `.git-paw/AGENTS.local.md` hint with the attach hint
- [ ] 3.2 Keep the governance section in the sidecar when the pane goes (DS-1 already ships: `skills::supervisor_boot_context`, `skills.rs:928`, called at `commands/supervisor.rs:677`); the orchestrator assignment keeps carrying no spec or file-ownership content (GP-63); tests: `tests/governance_supervisor_boot.rs` stays green unchanged, and the e2e launch asserts the sidecar has the section with `[governance] dod`, none without governance, after the skill heading, and no `### Spec` / `### File Ownership` heading
- [ ] 3.3 `tmux/command.rs::build_supervisor_session`: dashboard is pane 0 via `new-session -c <repo_root>`; every agent split carries `-c <worktree>`; delete the `swap-pane` and compensated cwds; pane 0 title/`@paw_role` = `dashboard`
- [ ] 3.4 Layout geometry: supervised offset 1, no top-row horizontal split; keep row geometry (dashboard row 50%, agent rows `50 / rows`, ≤5 per row, equal-width rebalance)
- [ ] 3.5 Delete `assets/supervisor-framing.md` and its `include_str!` + byte-identity test
- [ ] 3.6 Tests: e2e launch (dashboard 0, agents 1..N, no repo-root agent CLI, full-width top row, hints on stdout, launch without `[supervisor].cli`/`default_cli` given `--cli`, `[supervisor].cli` as last agent fallback, missing agent CLI errors); layout unit tests for 5/7/10/20 agents with row-major indices from 1; `--dry-run` reports agent approval only

## 4. State, recovery, offsets

- [ ] 4.1 Stop writing `RepoOrchestratorEntry` at its writers, which are in the commands: `commands/supervisor.rs:857`, `commands/recover.rs:344`, `restart_from_pause` (`commands/start.rs:1008` via `helpers.rs::orchestrator_entry_for_mode`, deleted), and the carry-forwards in `commands/add.rs:249` / `commands/remove.rs:162` (`orchestrator` never emitted; `session.rs` keeps the type and `#[serde(default)]`); rewrite the orchestrator-entry tests in `tests/discovery_file_pane_indices.rs` (from `:432`) to assert no entry is written; `dashboard_pane = Some(0)` for every supervised start, with or without the broker (scenario "A broker-less supervised start still records the dashboard")
- [ ] 4.2 Offsets derive from the recorded `dashboard_pane` through `SessionLayout` (`Some(n)` → `n + 1`; `None` → the earlier rules: supervised → 2, bare → 1 with the broker and 0 without, dashboard index 0 for pause and restart); tests for `Some(0)`, legacy `Some(1)` (and that its panes 0/1 are never treated as coding), `None` in a supervised and a bare session (scenario "A session without dashboard_pane follows the earlier rules")
- [ ] 4.3 `commands/recover.rs`: supervised recovery rebuilds dashboard + N agents (`N + 1`), agents from 1 with `-c <worktree>`, re-saves `dashboard_pane = Some(0)` (including for a legacy session); default-config fallback applies on recovery
- [ ] 4.4 `coordination/inventory.rs`: `match_pane` no longer maps the `supervisor` row to pane 0 (`inventory.rs:188`, now the dashboard); the row, when the broker lists one, gets no pane and mode `Unknown`; rewrite `supervisor_resolves_to_pane_zero` and `build_inventory_against_fake_broker_no_tmux`. The `supervisor` wire identity stays: the drive loop keeps publishing escalations (`drive.rs:2808`) and branch-refresh feedback as `supervisor`, and the broker keeps seeding the row's CLI (`main.rs:1024`); a test pins that a drive-loop escalation creates no `supervisor` roster row (scenario "Drive-loop escalations create no supervisor row")

## 5. Config

- [ ] 5.1 `[supervisor].approval` stays parsed and affects no pane command (test: byte-identical pane commands with and without the key); no warning; earlier configs load and launch

## 6. Docs (relocation framing)

- [ ] 6.1 `docs/src/architecture.md`: supervised layout → dashboard pane 0 full-width, agents from 1; the orchestrator runs outside tmux via the broker, bundled helpers, or MCP
- [ ] 6.2 `docs/src/quick-start-supervisor.md` (re-indexed; no supervisor pane), `docs/src/user-guide/supervisor.md` (pane layout; remove "Run the supervisor pane at native full-auto"; `[supervisor].cli` / `approval` semantics), `user-guide/dashboard.md` (pane 0 in every mode, no inbox), `user-guide/governance.md` (governance reaches the orchestrator context), `configuration/README.md` (`[supervisor]` keys)
- [ ] 6.3 README Features bullets and Quick Start: Supervisor Mode use the relocation framing; `git paw start --supervisor --help` updated; `mdbook build docs/` succeeds

## 7. Gates

- [ ] 7.1 Every scenario in this change maps to a named test; removed requirements' tests deleted with them
- [ ] 7.2 `just check`, `just deny` green; full suite green against the merge-base (`cargo test --no-fail-fast`)
- [ ] 7.3 Security/safety note recorded: no auto-approved LLM process at the repo root inside the session; one role-based surface-eligibility rule for every keystroke sender, verified by test; no git-paw process approves a prompt
