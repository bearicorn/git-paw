## Why

Two approval-path defects surfaced in the v0.10.0 dogfood, both about *detecting a live prompt before sending keystrokes*:

1. **Multi-option prompts are missed (`sweep.sh approve` blind spot).** Claude Code's permission prompt for a command with a "don't ask again" option renders as `Do you want to proceed?` + a numbered option list (`1. Yes` / `2. …` / `3. No`) + the `Esc to cancel` footer — five-plus lines. The live-prompt detection's ~4-line window sees only the tail (options + footer) and misses the `Do you want to proceed?` marker above it, so the supervisor's `sweep.sh approve` reports "prompt cleared, no keys sent" and the operator falls back to manual `send-keys` every time. This was the dominant residual approval friction.
2. **Stray keystrokes when the prompt already cleared (F5).** When keys are sent to a pane whose prompt has since cleared (the agent moved on, or another approver won the race), the digits land as literal chat input — polluting the agent's context (observed: an agent reading a stray `1`/`2` as "messages").

## What Changes

- The live-prompt detection SHALL reliably recognise Claude Code's **multi-option** permission prompts — matching on the numbered-option glyphs / the `Do you want to proceed?` marker (widening the window as needed) — so a 3-option prompt is detected, not just single-line `Esc to cancel` cases. This applies to both the in-tool auto-approver and the bundled `sweep.sh approve` helper.
- Before dispatching approval keystrokes, the sender SHALL **re-confirm the prompt is still live** on a fresh capture immediately prior to sending; if it is not, it SHALL send nothing (no stray digits).

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `automatic-approval`: MODIFY **Live-prompt gate** to recognise multi-option prompts (option-glyph / proceed-marker matching, not only a tail `Esc to cancel`) and to re-confirm liveness on a fresh capture immediately before sending keystrokes, sending nothing if the prompt has cleared.

## Impact

- `src/supervisor/auto_approve.rs` (the live-prompt gate) and `assets/scripts/sweep.sh` (`approve` re-capture window) — kept in lockstep.
- Cuts the dominant approval-babysitting cost (multi-option prompts now auto-clearable) and eliminates the stray-keystroke context pollution.
