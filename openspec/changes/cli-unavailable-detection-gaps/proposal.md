## Why

`drive-loop-exit-summary-truth` shipped CLI-unavailability detection into the drive loop.
A code analysis of the merged result (`cli_unavailable_line` / `sweep_pane` in
`src/supervisor/drive.rs`) found four gaps, each verified against the code:

1. **It scans the window GP-31 showed to be broken.** `cli_unavailable_line` inspects the
   last `LIVE_PROMPT_TAIL` (4) non-blank lines. Current Claude Code renders a multi-line
   task list under the composer, which pushes a session-limit or `/login` banner out of that
   window — the banner is missed and the run is never flagged. The requirement even defines
   the scan as "the same tail discipline the live-prompt scan uses", so it inherits that
   scan's blind spot; `live-prompt-tail-window` fixes the live-prompt side only.
2. **It ignores per-CLI marker overrides.** It reads the embedded Claude profile directly,
   while `git paw __prompt-profile --field cli_unavailable_markers` serves the *resolved*
   per-CLI value to `sweep.sh`. A non-Claude CLI's limit or login banner is therefore never
   seen by the loop, and with an override set the loop and the helper classify the same pane
   differently. Detecting "a CLI that has stopped serving" is meant to cover any CLI.
3. **Its dedup key comes from unrelated transcript content.** Coding-pane escalations are
   keyed with `approval_dedup_key(agent_id, &capture)`, whose command slice is the last
   `Bash(...)` line anywhere in the capture — a tool call from before the banner, not the
   banner. Two different banners on one agent collapse into one key, and the key changes
   whenever that line scrolls out of the capture window, re-firing the escalation. The
   requirement says escalations are deduped on `(agent_id, shape)`.
4. **Orchestrator coverage depends on a supervisor pane.** The exit path fires only for
   the pane whose role resolves to the supervisor id. With no supervisor pane in the roster
   — the v0.18.0 model — the orchestrator's own CLI death is invisible, and nothing in the
   summary says so.

## What Changes

- The banner scan SHALL tolerate the same bounded trailing region of non-prompt content as
  the live-prompt gate, reusing the helper `live-prompt-tail-window` exposes.
- The loop SHALL resolve `cli_unavailable_markers` for the pane's own CLI (per-CLI override,
  falling back to the embedded default) — for this field only. Threading a full per-CLI
  profile through the loop is v0.19.0 work and is out of scope.
- A `cli-unavailable` escalation SHALL be deduplicated on the agent and the matched marker,
  never on incidental transcript content.
- With no orchestrator pane in the roster, worker-pane detection SHALL still work, and the
  exit summary SHALL state that no orchestrator pane was monitored rather than implying full
  coverage. The pane-0 / supervisor-role logic SHALL NOT be widened — v0.18.0 replaces it.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `supervisor-unattended-operation`: *CLI unavailability is an explicit non-success exit*.

## Impact

- `src/supervisor/drive.rs` — `cli_unavailable_line`, the coding-pane branch of
  `sweep_pane`, and the exit summary.
- Needs each pane's CLI. The roster / session file already records it per agent; plumb that
  value, do not re-derive it.
- **Sequencing**: lands after `live-prompt-tail-window` (reuses its helper) and after
  `idle-pane-nudge-submission` (both edit `drive.rs`). Wave 3.
- **Delta coordination**: `idle-pane-nudge-submission` modifies two *other* requirements in
  `supervisor-unattended-operation`; no requirement-name collision.
