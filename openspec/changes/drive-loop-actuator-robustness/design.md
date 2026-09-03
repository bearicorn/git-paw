## Context

The drive loop's only actuator is typing into panes, and that send misfires
intermittently — text lands in a paste buffer or on a stale input line and a bare
follow-up `Enter` does not submit it (peer-4-poker, 2026-09-03). Because it is
intermittent it fails invisibly: correction feedback, merge directives, and cross-agent
answers silently evaporate. A related lifecycle bug keeps the loop typing into idle
panes during wind-down.

## Goals / Non-Goals

**Goals:** a nudge reliably reaches the agent (verified, with recovery); the loop stops
typing once it winds down; the supervisor skill documents the recovery that actually
works.

**Non-Goals:** replacing tmux keystroke injection with a different IPC (out of scope);
the per-CLI interaction contract generalisation (v0.18). GP-06's session-identity guard
(change `unattended-wave-lifecycle`) is complementary but separate.

## Decisions

- **Reuse the boot prompt's settle-delay resolver on the nudge path.** The boot prompt
  already uses `[clis.<name>].submit_delay_ms` (per `cli-resolution`); the nudge path
  did not. Applying the same resolver between text and `Enter` fixes the common
  paste-buffer case with existing config. Alternative rejected: a new nudge-specific
  delay knob (redundant).
- **Verify-then-recover, not fire-and-forget.** After the `Enter`, capture the pane; if
  the text is still on the input line, clear it (`C-u`), re-type, `Enter`, bounded
  retries. A bare second `Enter` is a proven no-op against a stale line, so the recovery
  must re-establish the input, not just re-press Enter. Alternative rejected: more blind
  Enters (the documented approach that fails).
- **Correct the skill to match.** The supervisor skill documented the single-Enter
  recovery; it is corrected to the `C-u` + re-type + `Enter` sequence so a supervisor
  acting by hand does the thing that works.
- **Wind-down guard.** The loop checks its exit/wind-down state before dispatching any
  nudge, so idle panes are not re-loaded with unsubmitted text after the loop decides to
  exit.

## Risks / Trade-offs

- **`C-u` clears legitimate in-progress input the agent typed** → only recover a pane the
  loop itself just nudged and verified as unsubmitted (the loop owns that input line via
  the per-pane approval claim); do not `C-u` a pane with agent-authored content.
- **Retry storm on a genuinely wedged pane** → bound the attempts and fall back to an
  escalation rather than looping.

## Migration Plan

Behavioural; reuses existing `submit_delay_ms`. No config or state change. Rollback is a
straight revert.

## Open Questions

None.
