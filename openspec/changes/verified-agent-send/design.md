## Context

`send_nudge` in `supervisor/drive.rs` is the only code that types free text into a worker and checks
it was submitted. Its callers are all inside the loop (`hand_to_orchestrator`, `send_reengagement`,
the correction loop). v0.17.0 `idle-pane-nudge-submission` fixes its verification (whole input box,
wrapped rows, `paste_buffer_pattern`) and gives the idle-pane path the same recovery.

The driving agent has no equivalent. The bundled supervisor skill tells it to deliver `/tell`
prompts by `tmux send-keys` in `send-keys` mode, and to recover a paste-buffer stall by hand with
`C-u`, re-type, `Enter`. `sweep.sh` already brokers the driving agent's other pane actions (`capture`,
`approve`) and the hidden-subcommand pattern (`__prompt-profile`, `__authority`) is how it reaches
Rust logic without duplicating it.

## Goals / Non-Goals

**Goals:**

- One reliable, verified way for the driving agent to type a message into a worker.
- The same submission logic as the drive loop's nudges, per CLI.
- No new way to answer a permission prompt or race `sweep.sh approve` or the drive loop's nudges.
- No new pre-approved keystroke path: `setup-cli` never grants the verb.

**Non-Goals:**

- An MCP tool for sending (a natural follow-on to v0.18.0's root-only MCP tools; not specced here).
- Changing `/tell`'s delivery-mode selection or the broker `feedback` mode.
- Sending to anything but a coding agent.

## Decisions

**D1 — Reuse the actuator, do not copy it.** Factor `send_nudge`'s body into a function over the
terminal port and a resolved `CliProfile`, with the loop's existing callers unchanged. `__send`
resolves the session, the agent's surface from the roster and its profile from the launch record,
then calls it. A shell implementation in `sweep.sh` was rejected: it would duplicate the
verification rules that took two releases to get right.

**D2 — Guard order.** Refuse when run from a linked worktree (the same main-working-tree check
`__authority` makes: only the orchestrator at the repository root types into agents, never a coding
agent into a peer), resolve the target (unknown agent or non-agent surface → `refused`), take the
per-pane claim (busy → `refused: pane busy`), capture and refuse on a live permission prompt (using
the same live-prompt gate `sweep.sh approve` re-confirms with), then send. The claim is held for the
whole send-and-verify sequence.

**D3 — Text on stdin.** `sweep.sh tell <agent> <message…>` joins its arguments and pipes them into
`git paw __send --agent <agent>`, so the message is never part of `__send`'s argv. The 600-character
cap the loop applies to injected text (`INJECTED_TEXT_MAX_CHARS`) applies here too; longer messages
are refused with a pointer to the broker `feedback` mode, which has no size limit.

**D4 — Exit codes.** `0` delivered, `2` wedged, `3` refused, so the driving agent can branch without
parsing prose; the one-line result is printed either way.

**D5 — Never granted by `setup-cli`.** v0.17.0 `cli-setup` requires every `sweep.sh` subcommand to be
granted or listed as never granted. `tell` types into another agent's input, so it joins `approve`:
listed as never granted, with `Bash(.git-paw/scripts/sweep.sh tell *)` in `permissions.ask` so even a
broader user rule cannot let it run silently. Granting it was rejected: a pre-approved verb that
types free text into a peer's pane is a prompt-injection relay. A repository set up before this
change shows as out of date in `doctor`; git-paw does not rewrite the grants.

## Risks / Trade-offs

- **The claim blocks a concurrent `sweep.sh approve` for a few seconds** → the same window the loop's
  own nudges hold; `approve` skips a held pane rather than waiting, and the human or orchestrator
  re-runs it.
- **Refusing on a live prompt can strand a message** → the result says why; the driving agent relays
  the prompt to the human first and runs `sweep.sh approve` only on the human's word. Typing into a
  live prompt is the worse failure.
- **Every `tell` prompts** → by design (D5); the orchestrator's CLI asks once per call.
- **Depends on three unmerged changes** → lands last in the cycle, after `cli-profile-threading`.
