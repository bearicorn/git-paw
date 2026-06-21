## Context

git-paw runs many AI coding CLIs across tmux panes. When a CLI raises a permission modal ("Do you want to proceed?"), an *approver* clears it by sending keystrokes to the pane via `tmux send-keys`. Three approvers exist today:

1. **`assets/scripts/sweep.sh approve <pane>`** — the bundled helper the supervisor skill invokes; sends `Down` then `Enter` (`cmd_approve`, sweep.sh:331).
2. **The `automatic-approval` keystroke path** — the supervisor auto-approver; sends `BTab Down Enter` when a captured prompt classifies safe.
3. **A human** typing `1`/`2`+Enter into the pane directly.

All three send *blind*: they capture the pane at detection/decision time, then send keys some moments later without re-checking that the modal is still on screen. Between capture and send the prompt can clear — the agent timed out and moved on, the human cleared it first, or another approver won the race. The keys then land in the CLI's chat input as literal text, polluting context and leaving stray unsubmitted commands (the F5 race; v0.6.0 findings W15-13/W15-19). A real instance left an unsubmitted `/opsx:archive` in an agent's input box.

A separate, related failure: an early dedup attempt keyed pending approvals on the *prompt's trailing boilerplate text*. Because every CLI permission modal shares the same footer ("Do you want to proceed? ❯ Yes / No"), this collapsed every distinct command after the first into one signal and stalled the operator 3+ minutes (W15-19).

This change is the v0.9.0 "drift F" approval piece. The companion `unattended-drive-loop` change adds the in-tool `--unattended` poll loop; this change supplies the *primitive that loop and the bundled helper both call to send approval keys safely*.

## Goals / Non-Goals

**Goals:**

- A single, reusable **approval-send gate**: re-confirm a live permission-prompt footer is present in the target pane *immediately before* sending approval keystrokes; send nothing if the prompt has cleared.
- Forbid blind send-keys into the supervisor's own pane 0 (W15-13).
- Identity-keyed approval dedup — never dedup on prompt boilerplate (W15-19).
- Make the bundled `sweep.sh approve` and the in-binary `automatic-approval` send path both pass through the gate, so the helper the supervisor already invokes and the auto-approver are both race-safe.
- Reuse existing broker variants for the approval-trigger and escalation signals.

**Non-Goals:**

- **Per-CLI PreToolUse hooks that route the permission modal into the broker (path (a), the full "broker chat-with-options" approval).** This is explicitly deferred to v1.0.0 (per-CLI-hook-provider territory). v0.9.0 ships path (b): scrape-and-send gated on a re-confirmed live-prompt signal. No `BrokerMessage` variant or per-CLI hook is added here.
- **The in-tool `--unattended` poll loop itself**, completion detection, escalation policy, and the exit summary — owned by `unattended-drive-loop`. This change owns only the send gate that loop calls.
- **The safe/danger classification decision** — owned by `auto-approve-classifier` (#5) / `safe-command-classification`. The gate acts on a decision already made; it does not classify.
- **Clearing pane 0's *own* safe prompts** — a non-blind path (W15-3) owned by the drive loop. This change only forbids the *blind* send-keys path from touching pane 0.

## Decisions

### Decision 1: Re-confirm-before-send, not detect-then-send (path b)

The gate captures the target pane (`tmux capture-pane -p`) at the moment of sending and checks that a live permission-prompt footer is present in the **last ~4 non-blank lines** of the capture. Only on confirmation does it dispatch keys. If the footer is absent (the prompt cleared), the gate sends nothing and reports "prompt cleared, no keys sent".

- **Why the last ~4 non-blank lines:** the approval footer ("Do you want to proceed?" / "❯ 1. Yes" / "No") is always the bottommost interactive region. A match anywhere in scrollback would re-fire on a prompt the agent already answered (the footer scrolls up into history). Anchoring to the tail is what makes "live" mean live.
- **Why re-confirm rather than trust the detection capture:** detection and send are separated by classifier work, broker round-trips, and (in sweep.sh) a subprocess boundary plus `sleep`. The prompt can clear in that window. Re-confirming at send time is the only point where "is the modal still up?" is actually true.
- **Alternative considered — per-CLI hooks (path a):** route the modal to the broker so the approver answers structurally and the CLI never sees stray text. Strictly better, but requires a hook provider per CLI (PreToolUse for Claude, equivalents for others) — too large for v0.9.0. Deferred to v1.0.0 (Non-Goal).
- **Alternative considered — bracketed-paste / atomic single send-keys:** does not help; the problem is *whether to send at all*, not how the keys are framed.

### Decision 2: Pane 0 is excluded from blind send-keys

The gate refuses to send keys to pane index 0 (the supervisor's own pane; titled `supervisor` / `@paw_role = supervisor` per `supervisor-pane-affordances`). Typing into pane 0 blind would inject into the supervisor's own CLI input. Clearing pane 0's *own* prompt is a distinct action (W15-3) handled by the drive loop through a non-blind path; the blind send-keys gate SHALL never touch it.

- **Why index 0 specifically:** the session builder pins the supervisor to pane 0 and the dashboard to pane 1; coding agents are pane ≥ 2. sweep.sh's `discover_coding_panes` already excludes panes titled `supervisor`/`dashboard`. This change makes the *exclusion of pane 0 a normative property of the gate itself*, so any caller (not just sweep's discovery) is protected even if a caller passes an explicit pane argument of 0.

### Decision 3: Dedup on (agent/command) identity or wait-for-clear, never on prompt text

A pending-approval is deduped by `(agent_id or pane, command identity)` — or simply by waiting for the prompt to clear before re-acting. The gate SHALL NOT compute a dedup key from the captured prompt's trailing/boilerplate lines.

- **Why:** the footer is identical across all CLI permission modals, so a boilerplate key collapses distinct commands (W15-19). Command identity (the command text the modal is asking about, e.g. `cargo test` vs `git push`) distinguishes them; "wait for clear" (the natural consequence of Decision 1 — once a prompt is answered the footer disappears and the next distinct prompt re-confirms fresh) is the simplest correct dedup. This mirrors `feedback_inbox_dedup_unknown_classification`'s `(agent_id, shape)` keying and `stuck-prompt-detection`'s per-window dedup, neither of which keys on boilerplate.

### Decision 4: Reuse existing broker variants — no new message shape

The approval-trigger and escalation channel reuse what already exists:

- **Trigger:** `stuck-prompt-detection` already publishes a synthetic `agent.status` with `phase: "stuck-on-prompt"` carrying `detail.captured_prompt`. That is the signal that a pane is awaiting approval.
- **Escalation (unsafe/unknown):** `automatic-approval` already surfaces a non-auto-approvable prompt via `agent.question` (asking agent's slug), answered by the supervisor.
- **Supervisor→agent reply / feedback:** `agent.feedback` and `/tell` already exist.

`broker-messages` already defines all seven variants (`Status`, `Artifact`, `Blocked`, `Verified`, `Feedback`, `Question`, `Intent`). The gate needs no eighth. Adding one would be redundant and would widen the validated wire surface for no behavioural gain. (W15-11's `agent.question`→answer asymmetry is met operationally by `/tell`; the design notes it but does not add a variant.)

- **Alternative considered — a new `agent.approval` / chat-with-options variant:** that is path (a)'s wire format and belongs with the per-CLI hooks in v1.0.0. Specifying it now would be dead weight.

## Risks / Trade-offs

- **[Re-confirm window is not zero]** Even with re-confirm-at-send, a vanishingly small window exists between the final capture and the `send-keys` call. → Mitigation: the capture-then-send sequence is tight (no classifier work or broker round-trip between them, unlike detection→decision). The residual window is microseconds vs. the multi-second detection→send gap that caused F5; the race is reduced from "routinely observed" to "theoretically possible". Path (a) closes it fully in v1.0.0.
- **[Footer-pattern drift across CLIs]** A new CLI whose modal footer git-paw does not recognise would fail re-confirm and the gate would (safely) send nothing — the prompt stays up for a human. → Mitigation: this is fail-safe (no stray input). New footer patterns are added to the prompt-marker set shared with `permission-detection`/`stuck-prompt-detection`. A false negative costs a manual approval; a false positive (the old behaviour) costs context pollution.
- **[sweep.sh re-confirm adds a capture per approve]** `cmd_approve` now runs an extra `tmux capture-pane` before sending. → Mitigation: a single tail capture is cheap and `approve` is not on a hot loop (it fires per stuck prompt, deduped).
- **[Two callers, one rule]** The in-binary auto-approver and the shell `sweep.sh` implement the same gate in two languages, risking divergence. → Mitigation: both reference the same normative requirement (last ~4 non-blank lines, pane-0 exclusion); the `broker-mediated-approvals` capability is the single source of truth and each implementation has a scenario test asserting it.

## Migration Plan

Backward-compatible behaviour change, no config or wire-format change:

- Before: `sweep.sh approve <pane>` and the auto-approver send keys unconditionally.
- After: both re-confirm a live prompt first and refuse pane 0.
- A caller that approves a *still-live* prompt sees identical behaviour to today (keys are sent). The only behavioural difference is the previously-buggy case (prompt already cleared, or target is pane 0), where the new behaviour is to send nothing.
- No rollback hazard: the change can only *suppress* a send that would have been stray. Existing tests that approve a live prompt pass unchanged.

## Open Questions

- **None blocking.** The path (a) vs (b) decision is resolved (ship (b); see Decisions 1 & 4 and the Non-Goals). Whether the gate lives as a standalone `src/supervisor/approval_gate.rs` or a function on the existing approval path is an implementation detail left to tasks.md; the capability requirements are written against behaviour, not module layout.
