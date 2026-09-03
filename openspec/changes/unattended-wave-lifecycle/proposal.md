## Why

Two unattended-mode lifecycle defects (peer-4-poker, 2026-08-31) plus a state-file
gap make a finished wave misreport and let a torn-down session's drive loop leak into
the next. GP-05: the exit summary reports a completed wave as still "working." GP-06:
`purge`/`stop` leave the in-process drive loop running, so it keeps sweeping the
session *name* and drives the NEXT same-named session. Separately, the per-repo
session file omits the orchestrator pane, hiding the tiered-model split from tooling.

## What Changes

- **GP-05** — the exit summary resolves each agent's final state from terminal
  artifacts (a verified and/or merged branch, via the `WorkerPhase` lifecycle) rather
  than the agent's last broker-recorded `agent.status`, and prints a wave-level
  outcome line, so a fully verified+merged wave is not reported as "working."
- **GP-06** — the drive loop binds to a specific session *instance* (a stable
  instance token, not just the name) and stops acting once that instance is purged,
  stopped, or replaced, so it never drives a subsequently-created same-named session.
- Record the orchestrator/supervisor pane (pane 0) and its resolved CLI in the
  per-repo session JSON, additive and back-compatible.

No new commands or config. Additive/behavioural; existing sessions and files load and
behave unchanged.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `supervisor-unattended-operation`: the exit summary reports each agent's resolved
  final state and a wave-level outcome (GP-05); the drive loop is bound to a session
  instance and stops when that instance is torn down (GP-06).
- `session-state`: the per-repo session JSON additionally records the orchestrator
  pane and its CLI.

## Impact

- **Code:** the unattended drive loop and its exit summary in `src/supervisor/…`
  (reuse the `WorkerPhase` enum from v0.14 for terminal-state resolution; add a
  session-instance identity check); `src/session/…` (`RepoSessionFile` gains an
  additive orchestrator/supervisor entry).
- **Enum-variant ripple:** none — `WorkerPhase` is reused, not extended; the
  `RepoSessionFile` change is an additive field, not a new enum variant.
- **Backward compatibility:** the orchestrator field uses `#[serde(default)]`; older
  session files load unchanged and the bundled `sweep.sh` ignores the extra field.
- **Docs:** the unattended-operation guide (exit-summary semantics + drive-loop
  lifecycle) and the session-state / configuration reference (orchestrator field).
