## 1. Input-box reading (`src/supervisor/drive.rs`)

- [ ] 1.1 Add `input_box_rows(capture) -> Option<Vec<String>>` returning the sigil row's text plus following `│ … │` continuation rows up to the box's bottom border; make `input_box_text` its whitespace-joined form; keep the numbered-option exclusion
- [ ] 1.2 Add `input_box_shows_placeholder(capture) -> bool` using the resolved profile's `paste_buffer_pattern`
- [ ] 1.3 Unit tests: single-row box (unchanged), wrapped two-row box joins in order, placeholder detected, option line excluded, unrecognised shape → `None`

## 2. Nudge verification (`send_nudge`)

- [ ] 2.1 After the `Enter`, sleep `settle_delay` (injected clock) before the verify capture
- [ ] 2.2 Replace the `== Some(text)` check: present when the box is non-empty and (sigil-row text is a prefix of the sent text, or a placeholder is showing); submitted when the box is empty or `pane_is_mid_response`; any other non-empty content → treated as submitted (never `C-u` foreign text)
- [ ] 2.3 Factor the attempt loop (`C-u` → text → settle → `Enter` → verify) into a helper reused by task 3
- [ ] 2.4 Unit tests with `SequencedCapturer`: wrapped nudge still present → recovery fires and delivers; placeholder → recovery fires; empty box → delivered, no extra keys; verify occurs after the settle sleep (clock records two sleeps per attempt); existing `stale_input_line_triggers_clear_retype_enter_recovery` / `a_persistently_stale_line_is_reported_wedged_and_escalated` / `nudge_waits_the_settle_delay_before_enter` still pass

## 3. Idle-pane stranded text (`submit_buffered_input`, `sweep_pane`)

- [ ] 3.1 After the first `Enter`, re-capture after the settle delay; if still buffered and readable, run the shared recovery helper under the pane claim with the whole-box text; if a placeholder remains, skip `C-u` and escalate `nudge-wedged`
- [ ] 3.2 Resolve the pane's settle delay the same way the correction path does (`resolve_submit_delay_ms(cli, &config.clis)` from the `/status` row's `cli`, falling back to the default)
- [ ] 3.3 Unit tests: Enter clears the box → no further keys; text survives Enter → `C-u`, whole-box re-type, `Enter`; placeholder survives → no `C-u`, escalation; mid-response pane untouched (existing test); persistently stranded → exactly one escalation across two sweeps

## 4. Wedged lines in the summary (`DriveDeps`, `drive_loop`)

- [ ] 4.1 Add a `wedged: Vec<Escalation>` accumulator to `DriveDeps`; `send_nudge` (and the stranded-line path) push there deduped on `(agent_id, shape)` in addition to `alerts.escalate`
- [ ] 4.2 Fold `wedged` into `escalations` in `drive_loop` before the outcome is derived; update `run_drive_loop` and test constructors
- [ ] 4.3 Unit test through `drive_loop_in_tmp`: a wedged orchestrator hand-off yields one `nudge-wedged` entry in `DriveSummary.escalations` and outcome `escalated-for-review`

## 5. Docs and gates

- [ ] 5.1 `docs/src/user-guide/supervisor.md` (the nudge-mechanics paragraph around lines 158-168, which today describes text → settle → `Enter` → capture-verify → `C-u` recovery): state the verify-by-input-box rule, the redraw wait, and that wedged lines appear in the summary; grep `assets/agent-skills/` to confirm the paste-buffer recovery guidance already matches (no change expected)
- [ ] 5.2 Every scenario maps to a named test; `cargo fmt`; `just check` + `just deny` green; `mdbook build docs/` succeeds
