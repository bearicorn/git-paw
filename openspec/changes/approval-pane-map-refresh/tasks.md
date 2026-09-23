## 1. Failing tests first (`src/supervisor/poll.rs` tests)

- [ ] 1.1 `LiveRoster::resolve` with panes `0 /repo`, `1 /repo`, `2 <wt-a>`, `3 <wt-b>`, `4 <wt-c>` and roster A, B, C → `feat-c` maps to pane 4; a `tick_from_status` with `feat-c` stalled on a safe live prompt dispatches `1`/`Enter` to pane 4 (*An agent added mid-session is resolved and approved*) — use tempdir worktree paths so canonicalisation is real
- [ ] 1.2 Roster A, C (B removed) with panes `2 <wt-a>`, `3 <wt-c>` → a tick with `feat-c` stalled captures and dispatches only to pane 3 (record captured pane indices in the stub inspector) (*After a middle agent is removed…*)
- [ ] 1.3 Same layout, `feat-c`'s live prompt is a file edit of a path under `<wt-c>` (outside `<wt-a>`): it is approved into pane 3 — which only happens when the tick's worktree root is `<wt-c>`; under the old positional map (root `<wt-b>`/`<wt-a>`) it would escalate (*…never judged against a neighbour's worktree*)
- [ ] 1.4 Stalled agent whose worktree matches no pane → no capture, no keystrokes, no forward (*An unresolvable agent is skipped*)
- [ ] 1.5 Two panes with the same `pane_current_path` as one agent's worktree → no action for that agent (*An ambiguous match is skipped*)
- [ ] 1.6 `LiveRoster::for_session(None, ..)` and a session whose name differs both return `None` (*An unreadable roster takes no action*); pane 0/1 at the repo root never map to a coding agent

## 2. Live roster (lib)

- [ ] 2.1 `src/supervisor/drive.rs`: make `TmuxPaneEnumerator` `pub` (with a doc comment); add `AgentPane::roster_from_session(session: &Session) -> Vec<AgentPane>` (agent id = `slugify_branch(&wt.branch)`)
- [ ] 2.2 `src/supervisor/poll.rs`: add `pub struct LiveRoster` with `resolve(&[PaneInfo], &[AgentPane])` (via `drive::resolve_pane_agent`, `Coding` roles only, ambiguous ids dropped, worktree map only for resolved agents) and `for_session(Option<&Session>, &str, &[PaneInfo]) -> Option<LiveRoster>`; implement `PaneResolver` and `WorktreeResolver`; doc comments citing the requirement

## 3. Production wiring

- [ ] 3.1 `src/commands/supervisor.rs` `spawn_auto_approve_thread`: each tick, after the `/status` fetch, read `session::find_session_for_repo(&repo_root)` and `TmuxPaneEnumerator.list_panes(&session_name)`, build `LiveRoster::for_session`; on `None` skip the tick; pass the roster as `resolver` and `worktree_resolver`
- [ ] 3.2 Remove `pane_map` / `worktree_map` from `AutoApproveWiring` and from `cmd_dashboard` in `src/main.rs` (now orphaned); drop the now-unused `SUPERVISOR_PANE_OFFSET` import there if nothing else uses it
- [ ] 3.3 `drive_unattended_loop` (`src/commands/supervisor.rs`): build `agents` with `AgentPane::roster_from_session` instead of the inline mapping (behaviour unchanged)
- [ ] 3.4 Re-run the existing poll and auto-approve tests (`tests/auto_approve_integration.rs`, `tests/approvals_integration.rs`, `src/supervisor/poll.rs`) unchanged

## 4. Docs

- [ ] 4.1 `docs/src/user-guide/supervisor.md` *Auto-approve classification*: one paragraph — each tick the poll loop resolves panes from the live layout by `pane_current_path` against the session's worktrees, so agents added with `git paw add` are covered and a `git paw remove` never shifts approvals onto a neighbour; an agent whose pane cannot be matched is skipped (fails closed)
- [ ] 4.2 `docs/src/architecture.md`: the `poll.rs` row mentions per-tick path-based pane resolution
- [ ] 4.3 Verify every scenario in `specs/approval-auto/spec.md` maps to a test from section 1

## 5. Gates

- [ ] 5.1 `cargo fmt`, then `just lint`
- [ ] 5.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast > target/test.log 2>&1; echo $?` — exit code must be 0 (never trust a piped status)
- [ ] 5.3 `mdbook build docs/`
- [ ] 5.4 `openspec validate approval-pane-map-refresh --strict`
- [ ] 5.5 Security review (`security-and-safety-review` skill): confirm the change only narrows approvals — no path where an unresolved or ambiguous agent receives keystrokes
