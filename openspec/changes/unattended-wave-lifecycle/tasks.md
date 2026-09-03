## 1. GP-05 — exit summary reports resolved final state

- [ ] 1.1 Resolve each agent's final state in the drive-loop exit summary from terminal artifacts (verified/merged branch via `WorkerPhase`), not the last `agent.status`
- [ ] 1.2 Print a wave-level outcome line (e.g. "N/M branches merged") when per-agent resolution is unavailable
- [ ] 1.3 Tests: a fully verified+merged wave prints no `working` final state and reports completed; the existing outcome+escalations summary still holds

## 2. GP-06 — drive loop bound to a session instance

- [ ] 2.1 Give the drive loop a durable session-instance token (start receipt / PID / start-timestamp) and verify it against the live session each tick before acting
- [ ] 2.2 Exit the loop when its bound instance is purged/stopped/replaced; never act on a new same-named session's panes
- [ ] 2.3 Tests: purge a session under a running loop, start a new same-named session, assert the old loop sends no keys to the new panes and exits; a live-session loop keeps driving

## 3. Orchestrator pane in the session file

- [ ] 3.1 Add an additive orchestrator/supervisor entry (pane 0 + resolved CLI) to `RepoSessionFile`, `#[serde(default)]`
- [ ] 3.2 Tests: a supervisor-mode session file records the orchestrator pane distinct from agents; a pre-change file loads without error; `sweep.sh` still enumerates agents

## 4. Docs

- [ ] 4.1 Unattended-operation guide: exit-summary semantics + drive-loop session-instance lifecycle
- [ ] 4.2 Session-state / configuration reference: the orchestrator field
- [ ] 4.3 `mdbook build docs/` succeeds

## 5. Gates

- [ ] 5.1 `just check` green; no `unwrap()`/`expect()` in non-test code; public items documented
- [ ] 5.2 Backward-compat: older session files load; disabled/absent behaviour unchanged
- [ ] 5.3 Every spec scenario maps to a test
