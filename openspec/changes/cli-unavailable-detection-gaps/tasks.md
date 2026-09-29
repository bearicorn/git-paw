## 1. Failing tests first

- [x] 1.1 A banner followed by a six-line task-list region is detected for a coding pane — fails before the fix
- [x] 1.2 A CLI whose `[clis.<name>]` override sets `cli_unavailable_markers` to `out of tokens` is detected by the loop when its tail shows that text — fails before the fix
- [x] 1.3 The same banner across sweeps whose earlier transcript lines change yields exactly one escalation — fails before the fix
- [x] 1.4 Two different banners on one agent yield two escalations — fails before the fix
- [x] 1.5 A roster with no orchestrator pane still escalates a coding pane, and the summary states that no orchestrator pane was monitored

## 2. Window

- [x] 2.1 Replace the fixed `LIVE_PROMPT_TAIL` slice in `cli_unavailable_line` with the bounded-trailing-region helper exposed by `live-prompt-tail-window`
- [x] 2.2 Verify the existing scrolled-away scenario still ignores a marker beyond the bound

## 3. Per-CLI markers

- [x] 3.1 Plumb each pane's CLI (already in the roster / session state) to `cli_unavailable_line`; do not re-derive it
- [x] 3.2 Resolve `cli_unavailable_markers` with `resolve_prompt_profile(cli, &clis)` for this field only; fall back to the embedded default when the CLI is unknown
- [x] 3.3 Assert the loop and `git paw __prompt-profile --field cli_unavailable_markers` agree for an overridden CLI

## 4. Dedup key

- [x] 4.1 Key `cli-unavailable` escalations on `(agent_id, matched marker)`, not on `approval_dedup_key`
- [x] 4.2 Do not key on the banner line's full text, which may carry a reset time or countdown

## 5. No-orchestrator honesty

- [x] 5.1 When the roster has no orchestrator pane, add a summary line stating that no orchestrator pane was monitored for CLI unavailability
- [x] 5.2 Do NOT widen the pane-0 / supervisor-role resolution

## 6. Spend-limit marker (GP-50)

- [x] 6.1 Add `org's monthly spend limit` to the embedded profile's `cli_unavailable_markers` in `src/config/prompt_profile.rs` (narrowed from the bare `monthly spend limit`, which is plausible wording in an unrelated consumer app)
- [x] 6.2 Test: the full spend-limit banner matches `cli_unavailable_markers` and not `stream_error_markers`; the loop escalates a coding pane showing it

## 7. Docs and gates

- [x] 7.1 Update the unattended-operation chapter: banners are detected above CLI chrome, per-CLI markers apply, an organisation spend limit is detected, and the no-orchestrator limitation is stated
- [x] 7.2 Verify every scenario in `specs/supervisor-unattended-operation/spec.md` and `specs/cli-prompt-profiles/spec.md` maps to a test
- [x] 7.3 `cargo fmt`, then `just lint`
- [x] 7.4 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, verifying the real exit code
- [x] 7.5 `openspec validate cli-unavailable-detection-gaps --strict`

## 8. Gate-review fixes (blocking safety finding)

- [x] 8.1 Orchestrator self-echo: `handoff_text` for a `cli-unavailable` verdict must paraphrase the matched banner instead of quoting it verbatim, since the text is typed into the orchestrator's own pane, which the bounded-trailing-region scan re-reads on the next sweep
- [x] 8.2 Test: `handoff_text` for every embedded `cli_unavailable_markers` entry never quotes the marker verbatim
- [x] 8.3 Test: a full drive-loop run whose orchestrator pane shows the echoed hand-off text does NOT exit `cli-unavailable`
- [x] 8.4 Narrow the GP-50 marker from the bare `monthly spend limit` to `org's monthly spend limit`; update the `cli-prompt-profiles` delta requirement text (the spend-limit scenario's full banner text is unaffected); keep it out of `stream_error_markers`
- [x] 8.5 Fix `supervisor.md`'s false claim that `sweep.sh` classifies a pane identically against `cli_unavailable_markers` — it does not consult that field at all today; `__prompt-profile` is a read path only
- [x] 8.6 Add the spend-limit marker to the `cli_unavailable_markers` row in `cli-prompt-profiles.md`
- [x] 8.7 Note in `design.md` that a per-CLI override applies from a pane's second sweep onward (the `/status` snapshot starts empty) and that the orchestrator pane resolves the same way, not hardcoded to the embedded default
- [x] 8.8 `cargo fmt`, then `just lint`
- [x] 8.9 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, verifying the real exit code
- [x] 8.10 `openspec validate cli-unavailable-detection-gaps --strict`
