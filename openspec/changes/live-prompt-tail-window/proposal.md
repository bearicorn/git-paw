## Why

The live-prompt gate cannot see a permission modal when the CLI renders anything
below it. Current Claude Code draws a persistent task list under the input area, so a
live modal sits six-plus non-blank lines above the end of the capture — outside both
windows `live_prompt_markers_at_tail` inspects (`LIVE_PROMPT_TAIL = 4`, and
`LIVE_PROMPT_BLOCK = 15` only when an option glyph is already inside the 4). The gate
returns "no live prompt", `approval_send_gate` returns `PromptCleared`, and nothing is
ever approved.

Reproduced against the shipped 0.16.0 binary with two fixtures differing only by an
appended task footer — `approve option=1 (safe)` versus `no-op (not live)` — and
confirmed live by toggling the footer off mid-session with `ctrl+t`. Because
`approval_send_gate` → `live_prompt_in_tail` is the single gate every in-binary
keystroke approver passes through, and `tests/sweep_sh_live_gate_parity.rs` keeps the
bundled bash mirror equivalent to it, **both** approvers fail identically: an
unattended wave against a current Claude Code approves nothing and every agent wedges
on its first prompt.

This also gates `drive-loop-exit-summary-truth`, which specs its new
`cli_unavailable_markers` scan against "the same tail discipline the live-prompt scan
uses" — that feature inherits the blind spot unless this lands with it.

## What Changes

- The live-prompt scan SHALL anchor on the **modal block** rather than on a fixed count
  of trailing lines: it searches upward from the end of the capture for the prompt's
  structural markers, tolerating a bounded trailing region of non-prompt content
  (a task list, a status strip, a progress footer) between the modal and the end.
- A prompt that has scrolled away SHALL still fail the gate — the tolerated trailing
  region is bounded, so the change widens detection without weakening the
  phantom-approval guard that the window existed to provide.
- The bundled `sweep.sh` mirror (`is_live_capture`) SHALL be updated in the same shape,
  and `tests/sweep_sh_live_gate_parity.rs` SHALL gain fixtures carrying a trailing task
  region so the two cannot drift apart on this axis again.
The connection-drop marker gap found in the same dogfood (a real mid-wave CLI death
printed `API Error: Connection closed mid-response` and matched **no** marker list,
leaving a pane byte-identical to one that finished cleanly) is **not** handled here.
`drive-loop-exit-summary-truth` already carries a MODIFIED delta for the embedded
Claude profile requirement; two changes modifying one requirement cannot both archive,
so the markers are folded into that change as an amendment instead.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `approval-auto`: the *Live-prompt gate* requirement gains trailing-region tolerance —
  detection anchors on the modal block, not on a fixed tail line count.

## Impact

- `src/supervisor/auto_approve.rs` — `live_prompt_markers_at_tail`,
  `LIVE_PROMPT_TAIL` / `LIVE_PROMPT_BLOCK` (constants become an anchored search).
- `src/supervisor/approval_gate.rs` — `live_prompt_in_tail`, `is_live_prompt` callers;
  no signature change expected.
- `assets/scripts/sweep.sh` **and** the tracked `.git-paw/scripts/sweep.sh` copy —
  `is_live_capture`. Both must be updated together (they drift; see the repo's
  helper-sync note).
- `tests/sweep_sh_live_gate_parity.rs` — new trailing-region fixtures.
- **Sequencing**: lands before or with `drive-loop-exit-summary-truth`, whose
  `cli-unavailable` scan reuses this discipline and whose delta owns the embedded
  profile requirement. No API or config breakage; the change is detection-only and
  strictly widens what is recognised as live.
