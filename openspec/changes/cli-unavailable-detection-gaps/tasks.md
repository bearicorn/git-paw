## 1. Failing tests first

- [ ] 1.1 A banner followed by a six-line task-list region is detected for a coding pane — fails before the fix
- [ ] 1.2 A CLI whose `[clis.<name>]` override sets `cli_unavailable_markers` to `out of tokens` is detected by the loop when its tail shows that text — fails before the fix
- [ ] 1.3 The same banner across sweeps whose earlier transcript lines change yields exactly one escalation — fails before the fix
- [ ] 1.4 Two different banners on one agent yield two escalations — fails before the fix
- [ ] 1.5 A roster with no orchestrator pane still escalates a coding pane, and the summary states that no orchestrator pane was monitored

## 2. Window

- [ ] 2.1 Replace the fixed `LIVE_PROMPT_TAIL` slice in `cli_unavailable_line` with the bounded-trailing-region helper exposed by `live-prompt-tail-window`
- [ ] 2.2 Verify the existing scrolled-away scenario still ignores a marker beyond the bound

## 3. Per-CLI markers

- [ ] 3.1 Plumb each pane's CLI (already in the roster / session state) to `cli_unavailable_line`; do not re-derive it
- [ ] 3.2 Resolve `cli_unavailable_markers` with `resolve_prompt_profile(cli, &clis)` for this field only; fall back to the embedded default when the CLI is unknown
- [ ] 3.3 Assert the loop and `git paw __prompt-profile --field cli_unavailable_markers` agree for an overridden CLI

## 4. Dedup key

- [ ] 4.1 Key `cli-unavailable` escalations on `(agent_id, matched marker)`, not on `approval_dedup_key`
- [ ] 4.2 Do not key on the banner line's full text, which may carry a reset time or countdown

## 5. No-orchestrator honesty

- [ ] 5.1 When the roster has no orchestrator pane, add a summary line stating that no orchestrator pane was monitored for CLI unavailability
- [ ] 5.2 Do NOT widen the pane-0 / supervisor-role resolution

## 6. Docs and gates

- [ ] 6.1 Update the unattended-operation chapter: banners are detected above CLI chrome, per-CLI markers apply, and the no-orchestrator limitation is stated
- [ ] 6.2 Verify every scenario in `specs/supervisor-unattended-operation/spec.md` maps to a test
- [ ] 6.3 `cargo fmt`, then `just lint`
- [ ] 6.4 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, verifying the real exit code
- [ ] 6.5 `openspec validate cli-unavailable-detection-gaps --strict`
