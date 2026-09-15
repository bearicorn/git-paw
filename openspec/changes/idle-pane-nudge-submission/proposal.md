## Why

Drive-loop nudges still fail to submit into an **idle** pane on 0.16.0 (GP-19). A busy pane queues
and consumes the text; an idle pane is left holding it as an unsubmitted input line, and raising
`submit_delay_ms` to 4000 changed nothing — so this is not paste-settle timing. By hand, a lone
`Enter` / `C-m` against that stale line does nothing; `C-u`, re-type, `Enter` submits it. A 20-minute
external-dogfood wave burned its budget hand-delivering messages while `main` never advanced.

0.16.0's `drive-loop-actuator-robustness` (GP-15) did add exactly that recovery to
`src/supervisor/drive.rs::send_nudge`: text → settle → separate `Enter` → capture → if the text is
still on the input line, `C-u` + re-type + `Enter`, up to `NUDGE_RECOVERY_ATTEMPTS`. Reading the code
shows why it never fires in the field, and a second site the fix never reached:

1. **The verification cannot see a real nudge.** `send_nudge` judges "still present" by
   `input_box_text(&verify) == Some(text)` — exact equality with the whole sent text.
   `input_box_text` returns only the sigil row (`│ > …`) of the input box; a wrapped rendering's
   continuation rows carry no sigil and are skipped. Every production nudge is long
   (`reengagement_text`, `orchestration_nudge_text`, the hand-off texts — up to
   `INJECTED_TEXT_MAX_CHARS = 600` characters), so in any real pane it wraps, the comparison is
   unequal, and the stale line is reported `Delivered`. Recovery is dead code in production; the unit
   tests pass because their fixture text (`please continue`) fits one row. The capture is also taken
   immediately after the `Enter`, before the CLI has necessarily redrawn.
2. **The sweep-time idle-pane path is the lone `Enter` the finding describes.** Each sweep,
   `sweep_pane` → `submit_buffered_input` sees an idle pane with text in its box and sends exactly
   one `Enter` — the keystroke GP-19 shows is a no-op against a stale line. Nothing there clears or
   re-types. So a nudge that (1) mis-verified as delivered sits in the box, and every 15 s the loop
   confirms it is stuck by pressing the key that does not work.

The milestone's diagnosis — "the `C-u`+retype+`Enter` recovery isn't applied by the actuator for
idle/stale lines" — holds, narrowed to these two sites. No new abstraction is needed; the `drive.rs`
split stays in v0.18.0.

**GP-16 re-verified — already fixed, dropped from scope.** 0.16.0's `9e997d4` fetches `/status`
before dispatching any nudge, computes `winding_down` from that tick's own fetch, gates
`observe_worker_messages`' hand-offs on it, and places the orchestration nudge and the correction pass
after the completion break (`drive_loop`, the `winding_down` block); the covering tests
(`no nudge keystrokes on the tick the wave completes`, the wind-down cases around
`drive.rs:5600–5690`) pass, and the requirement "The drive loop stops nudging panes during wind-down"
is already in `openspec/specs/supervisor-unattended-operation`. The only residue is that
`sweep_pane` runs before the fetch, so on the exit tick a brand-new escalation hand-off or a
buffered-input `Enter` can still go out — before the loop has decided to exit, within the
requirement's letter, and not the reported symptom (nudge text re-accumulating in idle agent panes
after the loop is gone). This change specifies nothing for GP-16; the sweep/fetch ordering can be
revisited in the v0.18.0 split if the residue ever matters.

## What Changes

- **Verification judges the input box, not text equality.** After the `Enter`, the nudge counts as
  still present when the input box is non-empty and shows the nudge's leading text (a wrapped
  rendering) or a paste-buffer placeholder (the profile's `paste_buffer_pattern`); it counts as
  submitted only when the box is empty or the pane has left idle (mid-response marker). The
  verification capture is taken after the settle delay, not immediately.
- **The idle-pane path gets the real recovery.** A stranded line on an idle pane is first given the
  cheap `Enter`; if a fresh capture still shows it, the same bounded `C-u` + re-type + `Enter`
  recovery runs, re-typing the text read from the whole input box (sigil row plus continuation
  rows). The loop never clears text it cannot re-type verbatim — a paste-buffer placeholder gets the
  `Enter` and, if it stays, an escalation instead of a `C-u`.
- **Wedged lines reach the exit summary.** A nudge or stranded line still present after every attempt
  is escalated `nudge-wedged` (as today, to the broker) **and** appears in the summary's escalation
  list, deduped per pane so a stuck line is reported once, not every 15 seconds. Today those
  escalations go to the broker only, which is how a wave with wedged nudges printed `0 escalations`.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `supervisor-unattended-operation`: **send-keys nudges send a follow-up Enter** — verification
  semantics (input-box emptiness, wrapped/placeholder renderings, redraw allowance) and wedged
  nudges in the exit summary; **Idle pane with buffered input is submitted** — bounded clear +
  re-type + `Enter` recovery after the lone `Enter`, never clearing what cannot be re-typed,
  escalation of a wedged stranded line.

## Impact

- **Code — offending sites:** `src/supervisor/drive.rs` — `send_nudge` (the
  `input_box_text(&verify).as_deref() != Some(text)` check and the immediate verify capture),
  `input_box_text` (sigil-row-only read; gains a whole-box reader), `submit_buffered_input` (lone
  `Enter`), `pane_has_buffered_input`, `sweep_pane` (call site), `NudgeOutcome` /
  `NUDGE_WEDGED_VERDICT` (wedged escalations must reach `DriveSummary.escalations`),
  `hand_to_orchestrator` / `send_reengagement` (callers of `send_nudge`). The paste placeholder is
  matched with the existing profile field `paste_buffer_pattern` (`cli-prompt-profiles`); no new
  profile field.
- **Enum-variant ripple:** none (`BrokerMessage`, `SpecBackendKind` untouched; `NudgeOutcome` is
  private to `drive.rs`).
- **Tests:** unit tests in `drive.rs` (`SequencedCapturer` fixtures gain a wrapped multi-row input box
  and a placeholder box; the existing `stale_input_line_triggers_clear_retype_enter_recovery`,
  `a_persistently_stale_line_is_reported_wedged_and_escalated`,
  `loop_submits_a_stranded_directive_on_an_idle_pane`, `nudge_waits_the_settle_delay_before_enter`
  are extended, not weakened); `tests/unattended_drive_e2e.rs` only if a dummy CLI can render the
  box shape.
- **Docs / skills:** `docs/src/user-guide/supervisor.md` if it describes the nudge mechanics;
  the bundled paste-buffer recovery guidance in `assets/agent-skills/` already prescribes
  `C-u` + re-type + `Enter` and needs no change (verify by grep at implementation time).
- **Behaviour an existing user notices:** stranded nudges now get submitted or reported; the summary
  can show `nudge-wedged` entries it previously omitted. No config, CLI, or persisted-state change;
  `submit_delay_ms` keeps its meaning (settle between text and `Enter`) and additionally governs the
  redraw wait before verification.
- **Security / safety:** the only keystrokes added are the same `C-u` / text / `Enter` the actuator
  already sends, still under the exclusive per-pane claim and a fresh pre-send capture (TOCTOU
  guard). New safety rule: the loop never clears input it cannot restore, so a human's or another
  process's partial line is either re-typed verbatim or left alone. Attempts stay bounded; a wedged
  pane is escalated, never retried forever.
- **Dependencies:** none.
