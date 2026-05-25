## 1. Code fix in `cmd_supervisor` (revert to single-Enter)

- [x] 1.1 Revert the supervisor send-keys loop in `src/main.rs::cmd_supervisor` to a single `tmux send-keys -t <target> <prompt> Enter` invocation per pane — no sleeps, no extra Enters. Add a doc comment above the loop explaining that any further keystrokes risk accepting a follow-on permission prompt; recovery from paste-buffer state is the supervisor skill's responsibility.
- [x] 1.2 Remove the helper `tmux::build_supervisor_submit_argv_triple` (and the earlier `_pair` variant) — no longer needed once the loop is back to a single send-keys call.
- [x] 1.3 Remove the `SUBMIT_DELAY_MS` constant from `src/main.rs` — no inter-Enter delay is needed.
- [x] 1.4 Remove the unit tests on `build_supervisor_submit_argv_triple` in `src/tmux.rs::tests` and the `submit_delay_ms_is_300` test in `src/main.rs::tests`.

## 2. Skill update in `assets/agent-skills/supervisor.md`

- [x] 2.1 Add a new "Launch-time pane sweep" workflow step (numbered 1.5 between Baseline and Watch) instructing the supervisor agent to inspect every coding-agent pane immediately after attaching.
- [x] 2.2 In the launch-time sweep, enumerate the four pane categories (paste-buffer, permission prompt, working, idle) with example indicators and the default action for each.
- [x] 2.3 In the launch-time sweep, document the safe-command policy for permission prompts: safe-by-pattern (curl-to-localhost-broker, cargo, git commit/push) → "Yes, don't ask again"; confined-to-worktree (file edits, `git -C <worktree>`) → "Yes, allow all edits"; unknown → escalate via `agent.question`.
- [x] 2.4 State explicitly that the proactive sweep complements (does not replace) the existing `[supervisor.auto_approve]` background poll thread.
- [x] 2.5 Update the existing paste-buffer recovery sub-case under stall detection to note it applies both in the stall loop AND proactively at launch (cross-reference the launch sweep section).
- [x] 2.6 Keep the existing paste-buffer indicator examples (Claude Code's `Pasted text #N`) and lenient-detection heuristic.

## 3. Tests

- [x] 3.1 Skill-content test: load `assets/agent-skills/supervisor.md` via the embedded skills loader and assert the paste-buffer recovery sub-case heading is present (substring match: `paste-buffer` or `paste buffer`, case-insensitive). _(already added in earlier iteration)_
- [x] 3.2 Skill-content test: assert the supervisor skill mentions `Pasted text` (the Claude indicator example).
- [x] 3.3 Skill-content test: assert the supervisor skill mentions `tmux capture-pane` and `tmux send-keys` in the paste-buffer recovery sub-case context.
- [x] 3.4 Skill-content test: assert the supervisor skill explicitly notes the Enter recovery is safe-by-default (substring on `safe-by-default`, `no-op`, or equivalent phrasing).
- [x] 3.5 Skill-content test: assert the launch-time pane sweep section is present (substring match: `launch-time` or `launch sweep`, case-insensitive) AND lists all four pane categories (paste-buffer, permission prompt, working, idle).
- [x] 3.6 Skill-content test: assert the launch-time pane sweep section references the safe-command auto-approval keystroke pattern (`Down` + `Enter` to select "Yes, don't ask again").

## 4. Quality gates

- [x] 4.1 `just check` (fmt + clippy + tests) passes on the change branch (subject to the pre-existing `tests/config_integration.rs` brittleness — see drift item 24)
- [x] 4.2 `just deny` passes (no new dependencies — this change is code-only)
- [x] 4.3 No `unwrap()`/`expect()` introduced in the new code paths
- [x] 4.4 The simplified launch loop has a doc comment explaining the rationale

## 5. Docs

- [x] 5.1 No mdBook changes needed — this is internal behaviour, not a CLI surface change.
- [x] 5.2 No `--help` text changes — there is no new flag.
- [x] 5.3 No README changes — the change is invisible to end users except that prompts now actually submit (once the supervisor attaches and runs the launch-time sweep).

## 6. Dogfood verification

- [x] 6.1 Build the binary on the prompt-submit-fix branch
- [x] 6.2 Run `git paw start --from-specs --supervisor` against the v0.5.0 spec set
- [x] 6.3 Verify that the supervisor's launch-time sweep recovers paste-buffer-stuck agents and approves the broker-curl permission prompts within seconds of attach (dogfood evidence: all 11 agents unstuck and registered with the broker within ~10s of the supervisor running the sweep).
- [x] 6.4 If a paste-buffer-stuck pane is misclassified by the indicator heuristic, extend the indicator list in `supervisor.md`. (Not needed this round — `Pasted text #N` was the only indicator we observed.)

## 7. Cross-change findings (captured as drift items)

These were surfaced during the prompt-submit-fix dogfood but belong to other changes / future work. Documented here so they don't get lost.

- [x] 7.1 MILESTONE drift item 29: `task_prompt` keeps only the first line of spec content (`src/main.rs:817-820`). Boot-prompt truncation root cause. Scheduled as `boot-prompt-full-body` v0.5.0 cleanup change.
- [x] 7.2 MILESTONE drift item 30: dashboard supervisor row exists even when no supervisor process is running (`cmd_supervisor` publishes "Supervisor booting" before the D2 non-TTY skip path). Schedule into `supervisor-as-pane`.
- [x] 7.3 MILESTONE drift item 31: `build_status_message` has no CLI parameter; supervisor row shows `cli=''`. Schedule into `supervisor-as-pane`.
- [x] 7.4 MILESTONE drift item 32: skill wire-format examples for `agent.feedback` and `agent.status` are stale (missing `from`, `modified_files` fields). Schedule into `v040-hardening` (extends drift item 11).
- [x] 7.5 MILESTONE drift item 33: dashboard prompt-inbox panel doesn't follow `agent.feedback` reply threads. Deferred to v0.6.0 #9 (dashboard inbox restructure alongside MCP) per drift item 21.
- [x] 7.6 MILESTONE v1.0.0 feature: Per-CLI Broker-Curl Allowlist Seeding — every CLI's hook provider should seed the localhost broker curl pattern into the CLI's permission allowlist so agents never hit the bootstrap broker-curl permission prompt.
