## Context

`live_prompt_markers_at_tail` (`src/supervisor/auto_approve.rs:1347`) decides whether a
pane capture shows a live permission modal. It filters blank lines, then checks two
fixed windows measured from the **end** of the capture:

- `LIVE_PROMPT_TAIL = 4` — a textual marker anywhere in the last 4 non-blank lines, or
- `LIVE_PROMPT_BLOCK = 15` — an option glyph inside those 4 **plus** a textual marker
  inside the last 15.

Both windows assume the modal is the last thing drawn. Current Claude Code breaks that
assumption: it renders a persistent task list below the input area, so a live modal can
sit six or more non-blank lines above the end of the capture and match neither window.

`approval_send_gate` (`src/supervisor/approval_gate.rs`) routes every in-binary
keystroke approver through `live_prompt_in_tail`, and `is_live_capture` in
`assets/scripts/sweep.sh` is a hand-maintained bash mirror of the same logic, held in
step by `tests/sweep_sh_live_gate_parity.rs`. One defect therefore disables both
approvers at once.

Observed during the v0.17.0 dogfood: `sweep.sh approve 1` reported
`cleared before send, no keys sent` while the modal was plainly on screen, and three
agents wedged on their first prompt until approved by hand.

## Goals / Non-Goals

**Goals:**

- Detect a live modal regardless of CLI-rendered content below it.
- Keep the phantom-approval guard: a genuinely scrolled-away prompt must still fail.
- Keep the Rust gate and the bash mirror in lockstep, with parity fixtures that cover
  the trailing-region case.
- Leave the classifier untouched — this is detection, not a safety decision.

**Non-Goals:**

- Making the window sizes configurable per CLI. A `cli-prompt-profiles` field was
  considered and rejected (see Decisions).
- Changing which option index gets selected. That is `broad-grant-scope-safety`.
- Adding the connection-drop markers to the embedded profile. That requirement is
  already owned by `drive-loop-exit-summary-truth`'s delta and is folded in there.
- Any change to `sweep.sh`'s other subcommands.

## Decisions

**Anchor on the modal block, searching upward.** Instead of asking "is a marker in the
last N lines", scan backwards from the end of the non-blank lines for the structural
markers, allowing a bounded number of trailing non-prompt lines before the footer.
Expressed as a search, the fix is independent of how tall the modal is *and* of how
much the CLI draws beneath it.

**Keep a bound rather than searching the whole capture.** An unbounded search would
re-introduce exactly the phantom-approval bug the windows were added to prevent: a
scrolled-away prompt from two commands ago would read as live. The bound is the whole
safety property here, so it stays, just measured from the modal rather than from the
end of the buffer.

**Rejected: a per-CLI profile field for the window sizes.** It looks attractive because
`cli-prompt-profiles` already exists for "how this CLI's terminal looks". But a window
size is not a description of the CLI's surface — it is a tuning constant for our scan,
and making it configurable would mean every consumer must re-tune it whenever their CLI
changes its chrome. The anchored search needs no tuning at all, which is the stronger
fix. It also keeps faith with the existing `cli-prompt-profiles` requirement that
"Detection algorithms are unchanged by profile sourcing".

**Update both implementations in one change.** The bash mirror is not optional: the
bundled helper is what a supervisor pane and an external orchestrator actually call.
Shipping the Rust fix alone would leave the helper blind and the parity test red.

## Risks / Trade-offs

- **Widening detection could admit a stale prompt.** Mitigated by keeping the trailing
  region bounded and by an explicit scenario asserting a scrolled-away prompt still
  fails. The bound should be chosen from real captures, not guessed.
- **The bash mirror can drift again.** This is a pre-existing hazard (`assets/scripts/`
  and the tracked `.git-paw/scripts/` copy both exist and have drifted before). The
  parity test is the control; this change extends it rather than relying on discipline.
- **Other CLIs may render chrome we have not seen.** The anchored search is
  shape-agnostic by construction, which is precisely why it is preferred over tuning
  constants — but the bound is still an assumption, and a CLI with a very tall footer
  would need it raised.
- **Interaction with `drive-loop-exit-summary-truth`.** That change's
  `cli_unavailable_markers` scan is specced against "the same tail discipline". If it
  merges first, its marker scan is born with this blind spot and its tests will encode
  the broken behaviour. Sequencing is the mitigation: this change lands first, or the
  two land together.
