# approval-auto Specification

## Purpose
git-paw clears an unattended agent's safe permission prompts through three complementary layers that together approve routine prompts without ever landing a stray keystroke or granting more than intended.

1. **Keystroke approval gate.** Stall detection classifies a detected prompt; when it is safe AND live, the supervisor dispatches the agent CLI's "approve and remember" keystrokes via `tmux send-keys`, selecting the correct option index by prompt shape, restricting the permanent broad grant to allowlisted non-arbitrary-code verbs, pre-approving worktree-confined `git add`/`git commit`, escalating unknown prompts to the human as an `agent.question`, and logging every action to the broker. It never types into the supervisor's own pane 0.
2. **Broker-mediated send-gate re-confirm.** A single approval-send gate that every approver passes through re-captures the target pane immediately before send, dispatches keystrokes only when a live-prompt marker is still present, refuses pane 0, keys approval dedup on command/agent identity rather than shared footer text, and reuses the existing `BrokerMessage` variants for its trigger and escalation signals rather than introducing a new variant or per-CLI hook.
3. **File-edit auto-approval.** The classifier recognises Claude's write/edit/create/delete filesystem-prompt patterns and treats them as safe when the canonicalized target path resolves inside the agent's own worktree (guarding against symlink escape), gated by `approve_worktree_writes` (default true) and purely additive to the shell-command auto-approval.

These layers form one set: the keystroke gate decides WHETHER a prompt is safe to approve, the send-gate re-confirm decides whether it is STILL LIVE to dispatch at the last instant, and file-edit auto-approval extends the safe-classification set to worktree-confined filesystem operations.
## Requirements
### Requirement: Live-prompt gate

Prompt detection SHALL treat a prompt as present only when it is LIVE, and SHALL reliably detect both single- and multi-option permission prompts. A prompt is live when the pane capture contains the prompt's structural markers — the numbered Yes/No option glyphs and/or the `Do you want to proceed?` marker, together with the `Esc to cancel` footer — within a window anchored on the modal block itself. Detection SHALL search upward from the end of the capture rather than inspecting a fixed count of trailing lines, and SHALL tolerate a bounded region of non-prompt content between the modal and the end of the capture (up to 13 non-blank trailing lines below the footer, in the shipped implementation), so a CLI that renders a task list, status strip, or progress footer below its input area does not hide a live modal. The inspected window SHALL remain wide enough to span a full multi-option prompt block (`Do you want to proceed?` + N options + footer), so a prompt offering a "don't ask again" option is detected rather than missed. The tolerated trailing region SHALL be bounded, so a prompt that has genuinely scrolled out of view still fails the gate. The extended tolerance SHALL apply only when the modal's actual SHAPE is present, not a bare mention of its footer text: the footer marker (such as `Esc to cancel`) MUST anchor the START of a line (after decoration stripping, with an optional leading parenthesis), AND a numbered option line MUST be present within the wider block window above it. A confirmation-question line on its own, or the footer text merely appearing mid-line (narration describing the UI, or grep-style output echoing a matching line), SHALL be judged by the narrow tail alone and SHALL NOT extend the tolerance — because an answered prompt can leave its question in the transcript followed by a few lines of ordinary command output, and treating a bare substring mention as live would raise a phantom escalation or land an approval digit as stray chat input. Even when the footer-and-option shape is present, the extended tolerance SHALL NOT apply if an idle input-box line (the CLI's composer sigil, not an option line) appears anywhere between the matched footer and the capture's true end — a real live modal REPLACES the input box while it is up, so a quoted modal (e.g. a full permission prompt copied into tool output, such as the supervisor's own pane echoing another pane's capture) followed by the pane's own idle composer box is not a live prompt even though the footer and option shape both match. When these markers are absent, the system SHALL treat the capture as containing no live prompt: it SHALL NOT escalate a prompt for it and SHALL NOT send any approval keystrokes — preventing a supervisor that is merely narrating about a pane (or a scrolled-away earlier prompt) from tripping a phantom escalation or a phantom approval.

Before dispatching approval keystrokes, the sender SHALL re-confirm the prompt is still live on a FRESH capture taken immediately prior to sending. If the prompt has cleared in the interim (the agent moved on, or another approver won the race), the sender SHALL send nothing, so approval digits can never land as stray chat input.

The drive loop's prompt detector and the bundled `sweep.sh approve` helper SHALL use the same live-prompt markers and the same anchoring discipline so their detection agrees.

#### Scenario: Live single-option prompt is detected

- **GIVEN** a capture whose tail includes `Esc to cancel`
- **WHEN** the drive loop evaluates it
- **THEN** the live-prompt gate SHALL pass and the prompt SHALL be treated as live

#### Scenario: Live multi-option prompt is detected

- **GIVEN** a capture whose tail is a multi-option permission prompt (`Do you want to proceed?`, a numbered option list including a "don't ask again" option, and `Esc to cancel`)
- **WHEN** the drive loop (or `sweep.sh approve`) inspects it
- **THEN** the prompt SHALL be detected as live — the detection SHALL NOT report "no live prompt" merely because `Do you want to proceed?` sits above a narrow tail window

#### Scenario: Trailing task region does not hide a live modal

- **GIVEN** two captures of the same live multi-option prompt, identical except that one appends a six-line task-list region below the `Esc to cancel` footer
- **WHEN** the live-prompt gate evaluates each
- **THEN** both SHALL be detected as live
- **AND** `sweep.sh approve` SHALL dispatch keystrokes for both rather than reporting that the prompt cleared

#### Scenario: The effective trailing tolerance holds at its exact boundary

- **GIVEN** two captures of the same live multi-option prompt with an `Esc to cancel` footer, one followed by exactly 13 non-blank trailing lines and the other by exactly 14
- **WHEN** the live-prompt gate evaluates each
- **THEN** the 13-trailing-line capture SHALL be detected as live
- **AND** the 14-trailing-line capture SHALL fail the gate

#### Scenario: An answered prompt followed by command output is not live

- **GIVEN** a capture whose question line `Do you want to proceed?` is followed by four lines of ordinary command output and a shell prompt, with no option list and no `Esc to cancel` footer after it
- **WHEN** the live-prompt gate evaluates it
- **THEN** the gate SHALL fail, and the system SHALL neither escalate it nor send keystrokes, even though the question line lies within the bounded trailing region

#### Scenario: Footer absent is not live

- **GIVEN** a capture that mentions a command in prose but whose tail does NOT contain the prompt's option glyphs or `Esc to cancel`
- **WHEN** the drive loop evaluates it
- **THEN** the live-prompt gate SHALL fail and the system SHALL neither escalate it nor send keystrokes

#### Scenario: Scrolled-away prompt still fails the gate

- **GIVEN** a capture in which a permission prompt appears only in scrolled-away history, separated from the end of the capture by more content than the bounded trailing region allows
- **WHEN** the live-prompt gate evaluates it
- **THEN** the gate SHALL fail and the system SHALL NOT send keystrokes

#### Scenario: A footer with no option list anywhere is not a live modal

- **GIVEN** a capture whose tail includes `Esc to cancel` but contains no numbered option list anywhere, regardless of how close the footer sits to the end of the capture
- **WHEN** the live-prompt gate evaluates it
- **THEN** the gate SHALL fail and the system SHALL NOT send keystrokes — a footer string alone is not a live modal

#### Scenario: Narration mentioning the footer text is not live

- **GIVEN** a capture whose only occurrence of the footer text is inside a sentence (e.g. narration describing what the pane shows), more than 4 non-blank lines from the end, with no numbered option list anywhere
- **WHEN** the live-prompt gate evaluates it
- **THEN** the gate SHALL fail and the system SHALL NOT send keystrokes — a bare substring mention is not the modal's footer

#### Scenario: Grep-style output echoing the footer text is not live

- **GIVEN** a capture whose only occurrence of the footer text is inside a line of tool output (e.g. a grep match prefixed by file and line context), more than 4 non-blank lines from the end, with no numbered option list anywhere
- **WHEN** the live-prompt gate evaluates it
- **THEN** the gate SHALL fail and the system SHALL NOT send keystrokes

#### Scenario: A captured modal in tool output with an idle input box below it is not live

- **GIVEN** a capture whose tail shows a full permission prompt (footer and option list) quoted verbatim inside tool output (e.g. the supervisor's own pane running `sweep.sh capture` on another pane), followed by that pane's own idle input box
- **WHEN** the live-prompt gate evaluates it
- **THEN** the gate SHALL fail and the system SHALL NOT send keystrokes, even though the footer and option shape both match — a live modal would have replaced the idle input box, not appeared above it

#### Scenario: Prompt cleared before send sends nothing

- **GIVEN** `sweep.sh approve` found a prompt live and is about to send the approval
- **WHEN** a fresh capture taken immediately before sending shows the prompt has cleared
- **THEN** the sender SHALL send no keystrokes, so no approval digit lands as stray chat input

#### Scenario: Detection agrees across the drive loop and the sweep helper

- **WHEN** the same multi-option prompt capture is evaluated by the drive loop's prompt detector and by `sweep.sh approve`
- **THEN** both SHALL agree it is a live prompt (shared marker set and shared anchoring discipline)

#### Scenario: Parity holds for captures carrying a trailing region

- **WHEN** a capture carrying a trailing task region is evaluated by the drive loop's prompt detector and by the bundled `sweep.sh` mirror
- **THEN** both SHALL return the same liveness verdict

### Requirement: Approval keystrokes require a re-confirmed live prompt

Any approver that clears an agent CLI permission prompt by sending keystrokes via `tmux send-keys` SHALL pass through a single approval-send gate. Immediately before dispatching the approval keystrokes, the gate SHALL capture the target pane (e.g. `tmux capture-pane -p -t <session>:<pane>`) and SHALL confirm that a live prompt anchors the capture per the **Live-prompt gate**: at the capture's true end, a textual marker within the last 4 non-blank lines, OR a numbered option line anchoring that tail while a textual marker sits within the wider block window above it (so a multi-option prompt whose `Do you want to proceed?` header sits above the last 4 lines is still detected rather than missed); OR, at a bounded offset above the true end (up to 13 non-blank trailing lines below the footer), the modal's actual shape — the footer marker anchoring the START of a line together with a numbered option line within the wider block window above it, AND no idle input-box line (the CLI's composer sigil) anywhere between that footer and the capture's true end — so a modal sitting under a few lines of CLI chrome is detected, while a modal copied verbatim into tool output above the pane's own idle input box is not. Only when a live prompt is re-confirmed SHALL the gate dispatch the keystrokes.

If the re-confirm capture does NOT show a live prompt per the Live-prompt gate (including the shape requirement at a bounded offset — a bare mention of the footer text with no option list nearby does NOT count, nor does a footer and option list with an idle input-box line below them), the gate SHALL dispatch NO keystrokes to the pane. The capture used at the detection/decision stage SHALL NOT substitute for this re-confirm capture; the re-confirm capture SHALL be taken immediately before the send, with no classification work or broker round-trip between the re-confirm and the send.

A permission-prompt marker matched only in scrollback above the Live-prompt gate's inspected tail/block window, matched only as a bare substring mention with no accompanying option list, or matched above an idle input-box line, SHALL NOT count as a live prompt.

#### Scenario: Approval keys sent only when a live prompt is re-confirmed

- **GIVEN** an approval decision has been made for a pane and approval keystrokes are about to be sent
- **WHEN** the gate's immediate-before-send capture of the pane shows a permission-prompt marker anchoring the tail (per the Live-prompt gate's tail/block window)
- **THEN** the gate SHALL dispatch the approval keystrokes to that pane via `tmux send-keys`

#### Scenario: Cleared prompt receives no stray keys

- **GIVEN** an approval decision was made for a pane while it showed a permission prompt
- **WHEN** the gate's immediate-before-send capture of the pane no longer shows a permission-prompt marker anchoring the tail (the prompt has cleared)
- **THEN** the gate SHALL dispatch NO keystrokes to that pane
- **AND** the agent's CLI input SHALL receive no stray text

#### Scenario: A stale marker only in scrollback is not treated as live

- **GIVEN** a pane whose capture contains a permission-prompt marker only in scrollback above the Live-prompt gate's inspected tail/block window (a prompt the agent already answered, scrolled up into history)
- **WHEN** the gate evaluates the re-confirm capture
- **THEN** the gate SHALL treat the prompt as cleared
- **AND** SHALL dispatch NO keystrokes

#### Scenario: A footer mentioned without its modal shape is not treated as live

- **GIVEN** a pane whose capture contains the footer text only as a bare substring (narration or tool output) more than 4 non-blank lines from the end, with no numbered option list anywhere
- **WHEN** the gate evaluates the re-confirm capture
- **THEN** the gate SHALL treat the prompt as cleared
- **AND** SHALL dispatch NO keystrokes — preventing an approval digit from landing as a phantom chat message

### Requirement: Blind send-keys excludes the supervisor pane 0

The approval-send gate SHALL refuse to dispatch approval keystrokes to pane index 0 (the supervisor's own pane). When the resolved target pane index is 0, the gate SHALL send no keystrokes and SHALL report that pane 0 is excluded from blind send-keys.

No git-paw component SHALL send approval keystrokes to pane 0 by any other path. A permission prompt on the supervisor's own pane is answered by the human at that pane; the drive loop only escalates it (see `supervisor-unattended-operation`, *The drive loop covers the supervisor's own pane 0*).

#### Scenario: Pane 0 is never sent blind keystrokes

- **GIVEN** an approval target whose resolved pane index is 0
- **WHEN** the approval-send gate runs, even with a live prompt re-confirmed in pane 0
- **THEN** the gate SHALL dispatch NO keystrokes to pane 0
- **AND** SHALL report that pane 0 is excluded from blind send-keys

#### Scenario: Coding agent panes are still approvable

- **GIVEN** an approval target whose resolved pane index is 2 (a coding-agent pane) with a live prompt re-confirmed
- **WHEN** the approval-send gate runs
- **THEN** the gate SHALL dispatch the approval keystrokes to pane 2

#### Scenario: The drive loop sends no approval keystrokes to pane 0

- **GIVEN** an `--unattended` session whose supervisor pane (pane 0) shows a live permission prompt
- **WHEN** the drive loop sweeps pane 0
- **THEN** no keystroke answering the prompt SHALL be sent to pane 0

### Requirement: Approval gate reuses existing broker variants

Escalating a live permission prompt, and the signals around it, SHALL reuse the existing `BrokerMessage` variants defined in `broker-messages`. The trigger that a pane is waiting on a prompt SHALL be the synthetic `agent.status` with `phase: "stuck-on-prompt"` (per `stuck-prompt-detection`). The escalation of a live permission prompt SHALL reuse `agent.question`, and its question text SHALL name the originating agent and the prompted command. Supervisor→agent replies SHALL reuse `agent.feedback` or the supervisor `/tell` path. The system SHALL NOT define a `BrokerMessage` variant or a per-CLI permission hook dedicated to permission prompts.

#### Scenario: Escalation of a live prompt reuses agent.question

- **GIVEN** a live permission prompt on agent `feat-a`'s pane in an `--unattended` session
- **WHEN** the drive loop escalates it
- **THEN** the broker SHALL receive an `agent.question` whose question text names `feat-a` and the prompted command

#### Scenario: No permission-prompt message variant exists

- **WHEN** the broker message envelope is inspected
- **THEN** no `BrokerMessage` variant SHALL be dedicated to permission prompts or approvals
- **AND** the stuck-on-prompt trigger SHALL be expressed as an `agent.status`

### Requirement: Approval keystrokes are sent only by an explicit approve action

Approval keystrokes SHALL reach an agent pane only through the bundled `sweep.sh approve <pane>` action, run explicitly by a human or by the orchestrator. No git-paw process SHALL send approval keystrokes on its own initiative. The `--unattended` drive loop, the `__dashboard` process and every other git-paw subcommand SHALL detect and escalate permission prompts but SHALL NOT answer them. The explicit approve action SHALL always select the one-time "Yes" option and SHALL never select a durable "don't ask again" grant (see `broker-agent-helper`, *sweep.sh approve re-confirms a live prompt before sending keys*).

#### Scenario: An unattended run sends no approval keystrokes

- **GIVEN** an `--unattended` session whose coding-agent pane shows a live permission prompt
- **WHEN** the drive loop sweeps that pane on several consecutive ticks
- **THEN** no keystroke answering the prompt SHALL be sent to that pane

#### Scenario: An attended session runs no auto-approver

- **GIVEN** an attended supervisor session with the broker enabled and a configuration that still declares `[supervisor.auto_approve] enabled = true`
- **WHEN** a coding-agent pane shows a live permission prompt
- **THEN** no git-paw process SHALL send a keystroke to that pane
- **AND** the `__dashboard` process SHALL NOT run an auto-approve thread

#### Scenario: The explicit approve action answers once

- **GIVEN** a coding-agent pane showing a live permission prompt
- **WHEN** a human runs `sweep.sh approve <pane>` for it
- **THEN** the helper SHALL send `1` followed by `Enter`
- **AND** SHALL NOT select any durable "don't ask again" option

