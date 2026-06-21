## Why

Today an approver — `sweep.sh approve`, the supervisor auto-approver, or a human — sends approval keystrokes (`Down`/`Enter`, `1`+`Enter`, `BTab Down Enter`) to an agent pane *blind*: it does not re-check that a permission prompt is still on screen at the instant it sends. If the prompt already cleared (the agent moved on, or another approver won the race), the digits/keys land as a literal chat message — polluting the agent's context and leaving stray, unsubmitted commands (e.g. a dangling `/opsx:archive`) in the input box. This is the F5 blind-send-keys race surfaced repeatedly during v0.6.0–v0.8.0 dogfood (findings W15-13/W15-19). The current `sweep.sh approve <pane>` (assets/scripts/sweep.sh) and the `automatic-approval` keystroke path both send unconditionally.

This change eliminates the race by making *every* send-keys approval pass through a single re-confirm-before-send gate, and by forbidding blind send-keys into the supervisor's own pane (pane 0).

## What Changes

- Introduce a single, reusable **approval-send gate** that callers MUST pass through before dispatching approval keystrokes to a pane. The gate (1) captures the target pane immediately before sending, (2) confirms a live permission-prompt footer is present in the last ~4 non-blank lines, and (3) only then sends the keys; if the prompt has cleared, it sends NOTHING (no stray input). This is path (b) from the v0.9.0 design decision — a re-confirmed scrape-and-send gate, NOT per-CLI PreToolUse hooks.
- The gate MUST refuse to blindly send keys into the supervisor's own pane 0 (W15-13). Clearing pane 0's own prompt is a separate concern owned by the unattended drive loop; the blind send-keys path SHALL exclude pane 0.
- Approval dedup MUST key on command/agent identity (or wait-for-clear), NEVER on prompt boilerplate text (W15-19). Keying on the trailing prompt lines deduped every distinct command after the first and stalled the operator 3+ minutes.
- Update the bundled `assets/scripts/sweep.sh` `approve` subcommand to re-confirm a live prompt immediately before sending keys and to refuse pane 0, so the helper the supervisor skill already invokes is race-safe.
- Gate the `automatic-approval` keystroke path on the same re-confirm-before-send signal so the supervisor auto-approver cannot leave stray input.
- Reuse the existing broker primitives (`agent.status` with the `stuck-on-prompt` phase from `stuck-prompt-detection`, `agent.question`/`agent.blocked`, `agent.feedback`) as the approval-trigger and escalation channel. **No new broker message variant is introduced.**

**Non-Goal (explicit, deferred to v1.0.0):** routing the CLI permission modal to the broker via per-CLI PreToolUse hooks (the "broker chat-with-options" path (a)). v0.9.0 ships path (b) only. See design.md.

## Capabilities

### New Capabilities
- `broker-mediated-approvals`: the reusable approval-send gate — re-confirm a live permission-prompt footer in the target pane immediately before sending approval keystrokes; send nothing if the prompt has cleared; exclude the supervisor's pane 0 from blind send-keys; dedup pending approvals on command/agent identity (never on prompt boilerplate); and the rule that the gate's approval-trigger and escalation channel reuse existing broker variants rather than adding a new one.

### Modified Capabilities
- `automatic-approval`: the keystroke-send path is now gated on a re-confirm-live-prompt check immediately before `tmux send-keys`; when the prompt has cleared between detection and send, no keystrokes are dispatched.
- `stuck-prompt-detection`: the bundled `sweep.sh approve <pane>` subcommand re-confirms a live prompt immediately before sending keys and refuses to send into pane 0.

## Impact

- New module (e.g. `src/supervisor/approval_gate.rs` or a function in the existing approval path) implementing the re-confirm-before-send gate and the identity-keyed dedup, reused by the auto-approver and (via the unattended drive loop) the in-tool sweep.
- `assets/scripts/sweep.sh` — `cmd_approve` gains a pre-send `tmux capture-pane` re-confirm and a pane-0 guard; the `approve` usage line documents the live-prompt requirement.
- `assets/agent-skills/supervisor.md` — the approval guidance documents that approvals only fire on a re-confirmed live prompt and never target pane 0 via blind send-keys.
- Reuses existing tmux capture/send-keys, the `safe-command-classification` whitelist, the `stuck-prompt-detection` pane-keyed capture, and existing broker variants. No new third-party dependencies and no new broker message shape.
- Pairs with `unattended-drive-loop` (v0.9.0 #7), which consumes this gate from the in-tool poll loop, and `auto-approve-classifier` (#5), which supplies the safe/danger decision the gate acts on.
