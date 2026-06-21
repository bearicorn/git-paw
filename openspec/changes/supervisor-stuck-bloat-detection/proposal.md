## Why

In the v0.8.0 dogfood, a wave stalled for hours because the supervisor had
no way to distinguish a genuinely-stuck agent from a slow-but-progressing
one. Three failure modes recurred: an agent's CLI hit `Request timed out`
(the API stream failed) and sat dead; an agent's context bloated past
~250k tokens (`/clear to save 250k tokens`) and degraded until it froze;
and an agent made no progress across a long window yet looked "idle." Worse,
the supervisor twice mis-classified a *prompt-blocked* agent as idle and
made a wrong "wind down" call from branch-tip/file counts alone — once
leaving an agent blocked **waiting on the supervisor for 1054 minutes**.
For v0.9.0 unattended operation, the supervisor MUST detect and recover
from these states so a wave cannot stall forever.

## What Changes

- Extend `sweep.sh detect-stuck` (and its per-agent `stuck-eval` core) to
  detect three new stuck shapes in addition to the existing
  stuck-on-prompt:
  - **stream-timeout**: a `Request timed out` / transport-error marker in a
    pane → the CLI's API call failed; the agent is stalled. Flag + recover
    (nudge/restart).
  - **context-bloat**: a `/clear to save <N>k tokens` marker where `N`
    exceeds a configurable threshold (default ~250k observed) → proactively
    flag/restart rather than waiting for the freeze.
  - **no-progress**: across one heartbeat window (~25 min), an agent's
    task-checkbox count AND commit count are both unchanged → nudge or
    investigate.
- **Read LIVE pane state, not just counts.** The detector classifies an
  idle-looking agent by first inspecting its pane capture. A pane showing a
  permission/paste marker is **blocked-on-prompt** (route to the existing
  approval/classifier path), NOT no-progress. Only an agent with no prompt
  marker AND no checkbox/commit movement is no-progress.
- Add a **blocked-on-supervisor timeout**: an `agent.blocked` whose
  `payload.from` is the supervisor (or whose pane shows it is waiting on the
  supervisor) that stays unanswered past a configurable window is itself a
  detectable stuck state, surfaced for the supervisor to act on.
- The supervisor skill SHALL tolerate **multiple feedback→fix→re-verify
  cycles** per agent (mcp-server took 7, dev-allowlist 6) and SHALL NOT
  treat "not yet verified after N cycles" as stuck — re-verify cycles are
  normal progress, not a stall.
- Surface each new shape as a synthetic `agent.status` with a documented
  `phase`/`detail` (reusing the open phase enum) so the dashboard, MCP, and
  the unattended drive loop can consume it. Dedup per `(agent, shape)` as
  the existing stuck path already does.
- Document tunable thresholds in `[supervisor]` config and the supervisor
  skill so the detection cadence and limits are configurable, not hardcoded.

## Capabilities

### New Capabilities
<!-- none — this change extends existing detection machinery -->

### Modified Capabilities
- `stuck-prompt-detection`: extend `sweep.sh` detection from
  stuck-on-prompt only to also cover no-progress (checkbox+commit
  heartbeat), blocked-on-supervisor timeout, and the
  read-pane-before-classifying rule that distinguishes prompt-blocked from
  no-progress. Add the synthetic-publish phases for the new shapes.
- `supervisor-stream-timeout-recovery`: add detection of the
  `Request timed out` marker in a pane (the recovery section today covers
  the supervisor's *own* stream timeout; this adds detecting + recovering a
  *coding agent's* stream timeout), and the rule that N re-verify cycles is
  not a stall.
- `coordination-context-budget`: add a configurable context-bloat
  threshold and the supervisor-side rule that an agent past the threshold
  (observed via `/clear to save <N>k tokens`) is flagged proactively.

## Impact

- `assets/scripts/sweep.sh` — new detection branches in `stuck_eval` /
  `detect-stuck`, new marker regexes, no-progress heartbeat state, and
  blocked-on-supervisor timeout check.
- `assets/agent-skills/supervisor.md` — "Detecting stuck agents" and
  "Stream-timeout recovery" sections extended; new "N re-verify cycles is
  not stuck" guidance.
- `assets/agent-skills/coordination.md` — context-bloat threshold prose.
- `src/config.rs` (`SupervisorConfig`) — new optional threshold fields
  (no-progress window, context-bloat token threshold,
  blocked-on-supervisor window), all `#[serde(default)]`, backward
  compatible.
- Consumed by `unattended-drive-loop` (sibling change) which acts on the
  emitted phases. No new runtime dependencies.
