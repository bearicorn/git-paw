## 1. Port, tmux adapter, fake (wave 1 — no call site changes)

- [ ] 1.1 `src/terminal/mod.rs`: `TerminalBackend`, `SurfaceId`, `SurfaceInfo`, `SessionPlan`, `SurfaceSpec`, `Key`; module and item docs; `pub mod terminal` in `src/lib.rs`
- [ ] 1.2 `src/terminal/tmux.rs` (`TmuxBackend`) over `src/tmux/` builders + `CommandRunner`, absorbing `main.rs::submit_prompt_to_pane`, `launch-prompt-entry-point`'s idle-wait captures and entry-point submit (`submit_entry_points`), `permission_prompt::capture_pane`, `keys::TmuxKeyDispatcher`, `TmuxPaneEnumerator`, `TmuxPaneCapture`, the inventory's `list-panes` / title reads, `commands/start.rs` boot pre-fill and restart-from-pause split as methods — argv unchanged; `SurfaceId ⇄ pane index` conversion private here
- [ ] 1.3 `src/terminal/fake.rs` (`FakeBackend`): records operations, returns scripted screens and owners
- [ ] 1.4 Argv-fixture tests: for fixed launch / add / remove / submit / entry-point submit / capture plans the emitted command strings equal the pre-refactor output (content + order) (scenario "Launch argv is unchanged")
- [ ] 1.5 Gate: `just check` green; no call site changed yet

## 2. Drive loop (wave 2)

- [ ] 2.1 Core `resolve_agent(owner, roster)` (exact canonical-path equality, no prefix, no basename) replaces `drive.rs`'s path branch; the dashboard resolves from `SessionLayout`'s dashboard position (converted to a `SurfaceId` inside the tmux backend), any other unmatched surface is `Unknown`; `PaneInfo` carries the backend-supplied owner (scenarios "A path below a worktree root is not an owner match", "The dashboard is found by the layout record, not by its owner")
- [ ] 2.2 Blanket-implement the seams that survive v0.17.0 `classifier-retirement` (`PaneEnumerator`, `PaneCapture` in `drive.rs`; `KeyDispatcher` in `keys.rs`) over `&dyn TerminalBackend`; delete their tmux bodies (`poll.rs`, `approve.rs` and `approval_send_gate` are already gone)
- [ ] 2.3 Fake-backend tests: a sweep over a scripted live prompt records a `permission-prompt` escalation for the owning agent and no text/key operation on that surface; a scripted buffered input records exactly the submit key; no process spawned; owner attribution ignores index order; basename collision does not misattribute (the `code-architecture` fake-backend scenarios and the `supervisor-unattended-operation` scenarios)
- [ ] 2.4 Gate: drive-loop unit suites + `tests/unattended_drive_e2e.rs` pass unchanged

## 3. Commands, coordination, readiness (wave 3)

- [ ] 3.1 `commands/{start,recover,remove,add,supervisor,helpers}.rs` (launch, `attach_session_logging` → `stream_to`, restart-from-pause), `coordination/inventory.rs`, `tmux/readiness.rs` capture go through the backend; `remove`'s `resolve_pane_id_for_worktree`, `restart_from_pause`'s `resolve_pane_index_for_worktree` (`tmux/session.rs:571`) and `add`'s `agents_without_live_pane` delegate their path comparison to the core `resolve_agent`; `inventory::match_pane` keeps its basename-suffix match, reading owners through `list_surfaces`; the `@paw_role` liveness filter moves behind the port's liveness unchanged; `pause` / `stop` / `attach` / `status` / `doctor` are not migrated
- [ ] 3.2 Fake-backend launch test: a two-worktree plan with the broker enabled records one dashboard surface and two coding surfaces with labels, owners and launch commands; `launch-prompt-entry-point`'s idle wait reads screens through `capture_tail` (the shared idle predicate stays pure over the screen text) and its entry-point submit goes through `send_text` + `send_key(Enter)` — fake-backend test for "Entry-point idle wait and submit are asserted without tmux"
- [ ] 3.3 Gate: `tests/e2e_supervisor_launch.rs`, `add_remove_e2e.rs`, `pause_e2e.rs`, `recover_integration.rs`, `coordination_inventory_integration.rs`, `attach_e2e.rs` pass unchanged

## 4. Guards, persistence, docs (wave 4)

- [ ] 4.1 Source audit: no `tmux` process or hand-built pane target outside `src/tmux/**`, `src/terminal/tmux.rs` and the named `doctor` / `selftest` probes; failure names the file
- [ ] 4.2 Source audit: `src/terminal/mod.rs` public identifiers contain none of `tmux`, `pane`, `send_keys`, `capture_pane`, `pipe_pane`, `split_window`, `pane_current_path`; no `src/tmux/` file imports `crate::supervisor`
- [ ] 4.3 Round-trip test: pre-refactor `Session` and `RepoSessionFile` fixtures load and re-save byte-identically; `sweep.sh` reads the discovery file unchanged; parity test: for owners equal to a worktree root (the surface case), `sweep.sh`'s `resolve_agent_for_path` and the core `resolve_agent` name the same agent; a comment on `resolve_agent_for_path` documents its longest-realpath-prefix match as a deliberate difference (it maps arbitrary paths, not surfaces); `bash -n` on `assets/scripts/sweep.sh`
- [ ] 4.4 `docs/src/architecture.md` module diagram + responsibilities gain `src/terminal/` (module-list guard green); `docs/src/architecture-direction.md` "Where the code is today" Terminal row updated; `mdbook build docs/` succeeds
- [ ] 4.5 Every scenario maps to a named test; `just check` + `just deny` green; full suite green vs merge-base (`cargo test --no-fail-fast`); no `unwrap`/`expect` in non-test code
