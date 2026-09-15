## Context

`send_nudge` (`src/supervisor/drive.rs`) is the loop's only free-text actuator: claim the pane,
re-confirm readiness on a fresh capture, `send-keys -l <text>`, sleep `settle_delay`, `send-keys
Enter`, capture, and — if `input_box_text(capture) == Some(text)` — `C-u` + re-type + `Enter` up to
`NUDGE_RECOVERY_ATTEMPTS` (2), else escalate `nudge-wedged` to the broker. `input_box_text` scans
lines bottom-up for the first `│ <sigil> …` row and returns that row's text; continuation rows of a
wrapped box (`│   … │`, no sigil) are skipped. Nudge texts are `one_line_capped` to 600 characters;
an 80–120-column pane shows roughly 70–110 characters per row, so every production nudge wraps and
the equality check never matches — `Delivered` is returned for a line that is still sitting there.
The verify capture is taken with no delay after the `Enter`.

Separately, `sweep_pane` calls `submit_buffered_input` for any non-prompt pane: if the pane is idle
(no live prompt, no mid-response marker) and `input_box_text` is non-empty, it sends one `Enter`.
That is the lone keystroke GP-19 shows to be a no-op against a stale line. The two paths together
explain the field observation exactly: a wrapped nudge is mis-verified as delivered, then every
sweep presses `Enter` on it forever, and the summary (which lists only `sweep_pane` and correction
escalations, not `send_nudge`'s broker-only `nudge-wedged`) reports `0 escalations`.

GP-16 (wind-down nudging) was re-verified against `drive_loop`: the `winding_down` gate from
`9e997d4` is in place and tested; see the proposal. Nothing here changes wind-down.

## Goals / Non-Goals

**Goals:**
- Verification that can actually detect a stale nudge in a real pane.
- The idle-pane stranded-text path uses the same recovery as the nudge path.
- Wedged lines are visible in the exit summary, once.
- Zero new abstraction: the fix lives in `send_nudge`, `input_box_text`, and `submit_buffered_input`.

**Non-Goals:**
- Splitting `drive.rs` or extracting an actuator module (v0.18.0).
- Re-diagnosing *why* Claude Code ignores a lone `Enter` on a stale line; the observable contract is
  what is specified.
- Changing the approval path (`send_approval`), which sends only a digit + `Enter` and is unaffected.
- Changing wind-down behaviour (GP-16 — fixed in 0.16.0).

## Decisions

### D1 — Verify by input-box state, not text equality

`send_nudge`'s verify becomes: read the whole input box; the nudge is **present** if the box is
non-empty and either its text starts with the nudge's first row (prefix match on the sigil row
suffices: a wrapped rendering's first row is a prefix of the sent text) or it matches the profile's
`paste_buffer_pattern`; it is **submitted** if the box is empty or `pane_is_mid_response(capture)`.
Any other non-empty content (text this call did not type) is treated as submitted-and-moved-on, so
recovery never `C-u`s a line the loop cannot account for. This keeps the recovery's safety argument
(a `C-u` only ever clears a line this call typed) intact while making it reachable.

### D2 — Wait for the redraw before verifying

`capture-pane` immediately after `send-keys` can return the frame from before the CLI processed the
`Enter`. The verify capture is taken after sleeping `settle_delay` again (the injected clock, so tests
observe it). `submit_delay_ms` therefore governs both waits; no new knob.

### D3 — Read the whole input box

A `input_box_rows(capture) -> Option<Vec<String>>` reads the sigil row plus the following `│ … │`
rows up to the box's bottom border, in display order; `input_box_text` becomes its
whitespace-joined form. Nudges are already whitespace-normalised by `one_line_capped`, so re-typing
the joined text reproduces the loop's own nudge exactly; for a human's multi-row line it reproduces
the visible content, which is the best available restoration.

### D4 — Route the idle-pane path through the same recovery

`submit_buffered_input` keeps its cheap first step (fresh capture, still buffered → `Enter`), then
re-captures after the settle delay. If the text is gone: done. If present and readable (not a
placeholder): call the existing recovery loop with the read text (the same claim, the same bounded
attempts, the same `nudge-wedged` verdict) — implemented by factoring `send_nudge`'s attempt loop
into a helper both call, not by a new type. If present and a placeholder: no `C-u` (it cannot be
re-typed), escalate `nudge-wedged`.

The `Enter`-first step is kept because it is what submits a genuinely swallowed-`Enter` paste, costs
one keystroke, and preserves today's behaviour for the case it does fix.

### D5 — Wedged escalations reach the summary, deduped

`send_nudge` already calls `deps.alerts.escalate` for a wedged line. The loop's `escalations` (the
summary list) is separate and unreachable from `send_nudge`'s callers (`hand_to_orchestrator`
swallows the outcome). The smallest bridge: `DriveDeps` gains an owned `wedged: Vec<Escalation>`
accumulator that `send_nudge` pushes to (keyed `(agent_id, shape)` so a persistently stuck line is
recorded once), and `drive_loop` folds it into `escalations` before deriving the outcome — the same
place `drive-loop-exit-summary-truth` appends its `[unmerged]` entries. `DriveDeps` is built in
`run_drive_loop` and in tests, so the field is not a public-API change.

### D6 — GP-16 disposition

Verified fixed; no delta. The residue (sweep before fetch on the exit tick) is recorded in the
proposal for the v0.18.0 split and is not the reported symptom.

### D7 — Unhappy paths

- **A human is typing in the pane:** their partial line is non-empty text the loop did not type;
  D1 treats it as "moved on" for a nudge verify (no `C-u`), and D4's stranded-line path would only
  re-type it verbatim after an `Enter` — never lose it. A placeholder is never cleared.
- **An unrecognised CLI (no box shape):** `input_box_rows` is `None`, `pane_has_buffered_input` is
  false, verify falls back to "submitted" — exactly today's behaviour for that CLI.
- **A truly wedged pane:** bounded attempts, one escalation on the broker and in the summary, loop
  moves on; the human sees `nudge-wedged` instead of `0 escalations`.
- **Rate-limited CLI:** its box may show the limit line rather than the nudge; `drive-loop-exit-
  summary-truth` owns that detection; here the nudge is simply reported wedged if it stays.

## Risks / Trade-offs

- **Prefix matching could misjudge a coincidental same-prefix line as the loop's own text** and
  `C-u` it. The claim, the fresh pre-send capture, and the fact that the loop just typed that exact
  text make this vanishingly unlikely; the re-type restores the content anyway.
- **One extra settle wait per nudge** (D2) — hundreds of milliseconds to a few seconds per send,
  on a 15-second cadence.
- **Whole-box reading depends on the box border glyphs** the embedded Claude profile already
  relies on (`│ >`); a CLI whose box renders differently degrades to today's behaviour, not worse.
- **Test fixtures must grow realistic:** the existing single-row fixtures stay green but no longer
  prove the field case; the wrapped and placeholder fixtures are the load-bearing additions.
