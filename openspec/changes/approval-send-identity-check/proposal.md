## Why

Both in-binary auto-approvers classify a prompt from one capture and send the approval keys after a
second capture, but the second capture only checks that *a* prompt is live — not that it is the
prompt that was classified. If the pane moves on in between (a human or `sweep.sh approve` answers
the first prompt, or the CLI shows the next queued tool call) and a different prompt appears, the
keys approve that prompt with the old verdict: a safe `ls` verdict can approve `rm -rf ~`, and when
the verdict chose option 2 the new command gets a permanent "don't ask again" grant. The window is
not microseconds: both paths publish the `auto_approved` audit message over HTTP (up to ~4.5 s on a
slow broker) between classifying and re-confirming. The per-pane claim keeps other approvers out
only after it is taken, which is after classification, and the dedup key is never consulted on the
send path, so neither closes the gap.

## What Changes

- The send step of both approvers — the unattended drive loop (`drive.rs::sweep_pane` →
  `send_approval`) and the dashboard poll loop (`poll.rs::dispatch_approval` → `auto_approve_pane`
  → `approval_send_gate`) — SHALL derive the **prompt identity** from the fresh pre-send capture and
  send only when it equals the identity derived from the capture the verdict was computed from.
- The identity is the prompt's command text (the command slice the dedup shape already keys on),
  its file-operation target path, and its option list. Including the options means a reused option
  index still selects the option it was resolved against.
- On a mismatch the approver sends nothing, releases the claim, and discards the verdict; the next
  sweep classifies the new prompt from scratch. A verdict is never carried over to another prompt.
- One pure function, shared by both approvers, derives and compares the identity (functional core),
  so the two send paths cannot drift.
- No new config, CLI flag, broker variant or dependency. This change only removes sends: a bug in
  it causes a missed approval, never an extra one.

This change is small and self-contained (one pure function, two call sites, one new gate outcome).
It can be pulled into v0.17.0 if wanted: it does not depend on v0.18 and goes in either before or
after v0.18's gate consolidation (see design.md, D5).

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `approval-auto`: ADDED *Approval keystrokes reach only the prompt that was classified*;
  MODIFIED *Approval keystrokes require a re-confirmed live prompt* (the re-confirm also checks the
  prompt's identity).
- `supervisor-unattended-operation`: MODIFIED *Auto-approve classifier-safe prompts* (the drive
  loop's send applies the same identity check).

## Impact

- **Code:** `src/supervisor/approval_gate.rs` (new pure `PromptIdentity` derivation and
  comparison; `approval_send_gate` takes the expected identity; `GateOutcome` gains a
  prompt-changed variant); `src/supervisor/approve.rs` (`ApprovalRequest` carries the expected
  identity; `auto_approve_pane` maps the new outcome to not-sent); `src/supervisor/poll.rs`
  (`drive_outcomes` / `dispatch_approval` derive the identity from the capture they classified);
  `src/supervisor/drive.rs` (`sweep_pane` passes the classified capture's identity to
  `send_approval`, which applies the shared comparison). Public API ripple: `ApprovalRequest`
  gains a field, which touches `tests/auto_approve_integration.rs`.
- **Enum-variant ripple:** none. `BrokerMessage` and `SpecBackendKind` are untouched. `GateOutcome`
  is matched only in `approve.rs` and the gate's tests.
- **Docs:** `docs/src/user-guide/security-posture.md` and the auto-approve section of
  `docs/src/user-guide/supervisor.md` (what the re-confirm checks). No `--help`, README or
  configuration-reference change.
- **Cross-version dependencies:**
  - v0.17 `live-prompt-tail-window` (archived on `feat/v0.17.0-specs`) also MODIFIED *Approval
    keystrokes require a re-confirmed live prompt*. This delta carries that archived text and only
    adds the identity clause, so it must be rebased onto the v0.17 main spec before archiving.
  - v0.17 `classifier-compound-segments`: makes `extract_command_slice` return every line of a
    header-form prompt, so the identity covers multi-line commands. Without it the identity has the
    same first-line-only blind spot as the classifier.
  - v0.17 `broad-grant-scope-safety`: picks the option by its text and reads the file target from
    the header (GP-56). The identity reuses those extractors, so it follows both changes.
  - v0.17 `approval-pane-map-refresh`: fixes sending to the wrong *pane*; this change fixes sending
    to the wrong *prompt* in the right pane. They don't overlap (that change ADDs a requirement).
  - v0.17 `idle-pane-nudge-submission`: nudges already re-check that no prompt is live before
    sending; no overlap.
  - v0.18 `supervisor-pane-removal` (D3: `send_approval` goes through the shared gate) and
    `drive-loop-functional-core` (`ApproveIfStillLive` effect): after them there is one gate and the
    effect carries the expected identity. Neither modifies the requirements this change modifies.
  - v0.24 `unattended-heartbeat-progress` counts a dispatched approval as progress; an aborted send
    returns not-sent and must not count.
- **Security / safety:** closes a classifier-bypass window on the approval send path. The threat
  analysis is in design.md.
