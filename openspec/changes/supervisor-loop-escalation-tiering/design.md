## Context

Unattended sessions run two potential approvers on the same panes: the in-process
drive loop (`src/supervisor/drive.rs`, approves classifier-safe prompts) and the
supervisor agent running `sweep.sh`, whose skill tells it to sweep and approve.
They race on safe prompts (v0.11.0 "approval-digit race"). The drive loop already
approves only safe prompts and escalates the rest to the broker
(`Alerts::escalate`); the supervisor already drains its broker inbox with
cursor-based polling. So the fix is mostly a **reframe + de-duplication**, not new
plumbing.

## Goals / Non-Goals

**Goals:**
- Remove the approval race by construction (disjoint approval sets), with no
  runtime liveness detection, heartbeat, pane-view heuristic, or claim marker.
- Keep escalation a single uniform channel; make the supervisor a consumer of it.
- Preserve attended behaviour (supervisor is sole approver when no loop runs).

**Non-Goals:**
- No change to the classifier's safe/unsafe verdicts.
- No change to the drive loop's approval mechanics (option-digit, re-confirm).
- No new escalation transport — reuse the broker.

## Decisions

- **Awareness lives in the supervisor, not the loop.** The loop stays dumb and
  supervisor-agnostic; git-paw injects a "a drive loop is running — consume
  escalations, don't blanket-approve" directive into the supervisor's boot context
  when `--unattended`. This makes supervisor liveness a **boot-time mode fact**,
  not a runtime signal — dissolving the heartbeat/pane-view question entirely.
  Alternatives considered: loop detects supervisor via broker heartbeat or
  pane-in-CLI-view — rejected as more code for a fact already known at launch.
- **Disjoint sets, not a referee.** Loop approves the safe set; supervisor acts
  only on escalated (non-safe) prompts. No prompt is eligible for both → no race.
  A claim marker (the earlier `supervisor-auto-approve-hardening` #1) would only
  referee a duplication that shouldn't exist; removing the duplication is simpler
  and safer.
- **Escalations first, then sweep.** The supervisor drains loop escalations before
  its verify/merge/status sweep, because an escalation means "an agent is blocked
  right now" — higher latency-priority than the reasoning duties.

## Risks / Trade-offs

- [A hung-but-present supervisor could leave escalations undrained] → Mitigation:
  escalations persist on the broker (durable review items), so they are never
  lost; a human/driver can still read them, and stuck-detection surfaces a
  wedged supervisor. This is strictly better than today's race.
- [Skill prose is pinned by `*_skill_content.rs` tests] → grep the literals
  before editing `supervisor.md`; update the pins in the same change.

## Open Questions

- Exact form of the escalation "review item" the supervisor drains — reuse the
  existing `Alerts::escalate` broker publication vs. a dedicated escalation
  message the supervisor filters on. Resolve at apply; prefer reusing the
  existing broker escalation the supervisor inbox already receives.
