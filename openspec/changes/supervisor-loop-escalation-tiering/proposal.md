## Why

In an unattended session two components can approve the same permission prompt:
the in-process drive loop (the mechanical approver of classifier-*safe* prompts)
and the supervisor agent running `sweep.sh`, whose skill tells it to sweep every
pane and approve what it sees. Both target the same live prompt, and the approval
digit lands as literal pane text (v0.11.0 dogfood, "approval-digit race", 3×).

The root cause is **duplication**, not a missing lock: the supervisor
blanket-approves the *same* safe prompts the loop already owns. The fix is to
make the two approve **disjoint sets** and give escalation a single uniform
channel:

- The drive loop stays a dumb mechanical approver: approve safe, escalate the
  rest to the broker as a review item — with **no supervisor awareness**.
- The supervisor becomes a **consumer** of that escalation stream, not a second
  approver. It knows at boot whether a loop is running; when one is, it drains
  the loop's escalations first, then sweeps for its reasoning duties, and never
  blanket-approves safe prompts.

Because the loop approves only *safe* prompts and the supervisor acts only on
*escalated (non-safe)* prompts, the sets are disjoint and the race cannot occur —
with no runtime liveness detection, heartbeat, pane-view heuristic, or claim
marker. When no supervisor is running, escalations simply persist in the broker
log for the human/driver.

This SUPERSEDES the `supervisor-auto-approve-hardening` change: its #2 (per-poll
pane resolution) and #3 (approve-on-detect) are already implemented in the drive
loop, and its #4 (sweep.sh live-prompt block-window parity) already shipped in
v0.11.0; only its #1 (the approval race) remained, and this change resolves it by
de-duplication rather than a claim marker.

## What Changes

- **MODIFY** `unattended-operation` "Escalation of risky and unknown prompts is
  non-blocking": the loop's escalation is a **uniform, supervisor-agnostic**
  review item on the broker, drainable by a supervisor if one is running and
  otherwise readable by the human. The loop remains the **sole approver of
  classifier-safe prompts**.
- **ADD** `supervisor-injection` requirement: when the session is unattended (a
  drive loop is running), git-paw injects into the supervisor's boot context that
  a loop is auto-approving safe prompts and that the supervisor consumes
  escalations rather than blanket-approving.
- **ADD** `supervisor-skill-discipline` requirement: when a drive loop is running,
  the supervisor SHALL, each cycle, **drain the loop's escalations first**
  (reason → targeted approve / feedback) **then** perform its normal sweep
  (verify, merge, conflicts, detect-stuck, status), and SHALL NOT blanket-approve
  safe prompts. With no loop (attended), it does the full sweep + approve as today.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `unattended-operation`: loop escalation is a uniform supervisor-agnostic review
  channel; loop is the sole approver of safe prompts.
- `supervisor-injection`: supervisor boot context carries the drive-loop
  coordination directive when unattended.
- `supervisor-skill-discipline`: escalation-first-then-sweep, no blanket-approve
  when a drive loop is running.

## Impact

- **Code:** supervisor boot-context assembly (inject the "loop running" directive
  when unattended). The drive loop already escalates to the broker
  (`Alerts::escalate`) and already approves only safe prompts — no loop change
  beyond confirming the escalation is drainable by the supervisor inbox.
- **Skill:** `assets/agent-skills/supervisor.md` reframed (escalation-first,
  no blanket-approve when a loop runs). ⚠ pinned by `*_skill_content.rs` tests.
- **Tests:** boot-context injection carries the directive under `--unattended`;
  skill-content assertions for the new guidance; regression tests confirming the
  loop still approves only safe prompts and escalates the rest.
- **Supersedes** `supervisor-auto-approve-hardening` (removed).
- No config surface change; unattended behaviour only; attended path unchanged.
