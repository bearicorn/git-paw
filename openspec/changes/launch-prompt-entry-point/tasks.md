## 1. Task prompt and composition (pure)

- [ ] 1.1 `assets/boot-task-prompts/openspec.md`: prose points at the sidecar and says the workflow command follows as the next message (no `/opsx:apply`); new `assets/boot-task-prompts/openspec.entry.md` = `/opsx:apply {{SPEC_ID}}`; add both to `lang_agnostic_skill_audit`'s asset list
- [ ] 1.2 `src/main.rs`: `build_task_prompt -> TaskPrompt { prose, entry_point }` with one exhaustive `SpecBackendKind` match selecting both templates; update the per-backend tests to the `supervisor-launch` scenarios (OpenSpec prose has no `/opsx:apply`, entry point `/opsx:apply my-change`; others none; no-spec prose verbatim)
- [ ] 1.3 `assets/worktree-root-rule.md` (`{{WORKTREE_ROOT}}`) and `compose_launch_prompt(boot, prose, worktree_root)` replacing `combine_boot_and_task_prompt`; unit tests for order and blank-line joins, with and without boot block and prose

## 2. Idle wait and entry-point submit

- [ ] 2.1 Shared idle predicate (no live prompt, not mid-response, empty or unrecognised input box) extracted beside the drive loop's, reused by both
- [ ] 2.2 `submit_entry_points(session, panes_with_entry_points, budget)`: one concurrent poll loop, two consecutive idle captures after the prompt's `Enter`, re-capture before typing, split-send submit, stderr line per expired pane; fake capturer/clock unit tests over fixtures (idle, mid-response, live prompt, buffered input, unknown CLI, expiry)
- [ ] 2.3 Wire into spec-driven `start` (`start.rs:818-842`), supervised `start` (`supervisor.rs:897-911`) and `add` to an active session (`add.rs:285-295`); `AttachedAgent` carries the entry point; bare pre-fill uses `compose_launch_prompt(Some(boot), None, root)`
- [ ] 2.4 `WorktreeEntry.pending_entry_point` (additive, `serde(default, skip_serializing_if)`); paused `add` sets it beside `pending_boot_prompt`; restart from pause submits the held prompt, then the entry point via 2.2, keeping both held when the pane stays blocked
- [ ] 2.5 Point `doctor-spec-workflow-check`'s instructed-command probe at the entry-point selector from 1.2 (remove its separate match); update AGENTS.md §7's `SpecBackendKind` site list

## 3. End-to-end

- [ ] 3.1 `tests/start_spec_launch_parity_integration.rs`: an OpenSpec spec-driven launch with a stub CLI shows the composed prompt (ending in the worktree rule) submitted, then `/opsx:apply <id>` as a second submitted line; a Markdown launch types no second line
- [ ] 3.2 `tests/pause_e2e.rs`: `add --from-spec` while paused records the held entry point; restart submits prompt then entry point and clears both; an older session JSON without the field restarts unchanged
- [ ] 3.3 Bare `start` pre-fill ends with the worktree rule naming the pane's recorded worktree path; no `Enter`

## 4. Docs and gates

- [ ] 4.1 `docs/src/user-guide/spec-driven-launch.md` (two messages, the idle wait, the stderr hint on expiry), `docs/src/user-guide/coordination.md` (the worktree rule); `mdbook build docs/`
- [ ] 4.2 `cargo fmt`, `just lint`, `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast` (real exit code), `just deny`
- [ ] 4.3 Security and safety review (`security-and-safety-review`): new keystrokes only into a re-checked idle composer, never into a prompt; template-only text; no CLI names in Rust
- [ ] 4.4 `openspec validate launch-prompt-entry-point --strict`
