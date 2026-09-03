## Why

The drive loop wakes agents by typing into their panes — and that send frequently does
not submit (peer-4-poker, 2026-09-03):

- **GP-15 (HIGH)** — nudges land in the CLI's paste buffer or on a stale input line
  unsubmitted. The documented single follow-up `Enter` recovery is a **no-op** against a
  stale input line (`Enter`/`C-m` do nothing); only clearing the line (`C-u`) + re-typing
  + `Enter` submits. And the nudge path does not use the per-CLI `submit_delay_ms` settle
  delay the boot prompt uses. The pump's only actuator silently misfires, so correction
  feedback, merge directives, and cross-agent answers evaporate — *intermittently*, so it
  fails invisibly, undermining the central claim that "the loop is the pump."
- **GP-16 (MEDIUM)** — during and after wind-down the loop keeps typing nudge text into
  idle agent panes, where it re-accumulates unsubmitted across sweeps, so agents never
  consume their verified/merged outcomes.

## What Changes

- The nudge path submits reliably: send the text, wait the per-CLI settle delay
  (`[clis.<name>].submit_delay_ms`, the same resolver the boot prompt uses), send `Enter`,
  then **verify submission via `capture-pane`**; if the text is still on the input line,
  recover with `C-u` (clear) + re-type + `Enter`, up to a bounded number of attempts. No
  reliance on a single combined text+Enter or a lone follow-up `Enter`.
- The supervisor skill's paste-buffer recovery is corrected: a bare `Enter`/`C-m` is a
  no-op against a stale input line; the robust recovery is `C-u` + re-type + `Enter`.
- The drive loop stops nudging panes once it has entered wind-down (no nudge on an
  exit-state sweep).

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `supervisor-unattended-operation`: nudge submission is robust (settle delay +
  capture-verify + `C-u` recovery); the loop stops nudging panes during wind-down.
- `skill-agent`: the embedded supervisor skill documents the corrected paste-buffer
  recovery (`C-u` + re-type + `Enter`, not a bare `Enter`).

## Impact

- **Code:** the drive-loop nudge/send path in `src/supervisor/…` (reuse the
  `submit_delay_ms` settle-delay resolver from `cli-resolution`; add capture-verify +
  `C-u` recovery; a wind-down guard); `assets/agent-skills/supervisor.md` skill content +
  its skill-content test.
- **Enum-variant ripple:** none.
- **Concurrency:** recovery keystrokes remain governed by the v0.14 per-pane approval
  claim, so the extra sends cannot double-fire across the loop and `sweep.sh`.
- **Backward compatibility:** behavioural — more reliable submission; reuses the existing
  `submit_delay_ms`, no new config.
- **Docs:** the `supervisor.md` user-guide chapter mirrors the corrected recovery.
