## ADDED Requirements

### Requirement: Approval keystrokes reach only the prompt that was classified

An approver that classifies a prompt from one capture and sends the approval keystrokes after a later capture SHALL send them only when the prompt live in the pre-send capture is the prompt it classified. This governs both in-binary approvers: the dashboard's auto-approve poll loop and the unattended drive loop.

The system SHALL derive a prompt's **identity** from a capture using the same extractors the classifier reads. The identity has three parts:

1. the prompted command text exactly as captured, which is the command slice the escalation dedup shape keys on (when a prompt has neither a command header nor a file-operation target, this part SHALL be the capture text the classifier evaluated in its place);
2. the file-operation target path;
3. the prompt's numbered option list, with decoration and the selection caret removed.

The expected identity SHALL be derived from the same capture the verdict and option index were computed from. Immediately before sending, the approver SHALL derive the identity of the live prompt from the same fresh capture that re-confirms a live prompt, taken while holding the pane's claim. It SHALL dispatch the keystrokes only when that identity equals the expected identity. The footer and the position of the selection caret SHALL NOT be part of the identity, so two captures of one unchanged prompt yield equal identities.

When the identities differ, the approver:
- SHALL dispatch NO keystrokes and SHALL release the pane's claim;
- SHALL report the attempt as not approved;
- SHALL discard the verdict. The next sweep SHALL capture and classify the pane afresh, and no verdict, option index or matched rule SHALL be carried from one prompt to another.

Both approvers SHALL make the comparison through one shared, side-effect-free function, so they reach the same decision for the same pair of captures. Only a safe verdict SHALL reach the send step. A danger or unknown verdict SHALL be escalated and SHALL NOT be sent, whatever a later capture of the pane shows. The bundled `sweep.sh approve` helper carries no earlier verdict and resolves its option from its own capture, so this requirement does not govern it.

#### Scenario: An unchanged prompt is approved

- **GIVEN** pane 2 shows a live 2-option prompt for `ls -la src` that the approver classified safe with option 1
- **WHEN** the approver's pre-send capture shows the same prompt (the same command and the same option list)
- **THEN** it SHALL send `1` then `Enter` to pane 2

#### Scenario: A different command that replaced the classified one receives no keystrokes

- **GIVEN** the approver classified a live prompt for `ls -la src` on pane 2 as safe
- **AND** before the send that prompt was answered and a live prompt for `rm -rf ~/projects` appeared on pane 2
- **WHEN** the approver takes its pre-send capture
- **THEN** it SHALL dispatch NO keystrokes to pane 2
- **AND** it SHALL report the attempt as not approved

#### Scenario: The replacement prompt is classified afresh on the next sweep

- **GIVEN** a send was aborted because a live `rm -rf ~/projects` prompt replaced the classified `ls -la src` prompt on pane 2
- **WHEN** the next sweep captures pane 2 while the `rm -rf ~/projects` prompt is still live
- **THEN** the approver SHALL classify that prompt from the new capture and escalate it as danger
- **AND** it SHALL NOT approve it with the earlier safe verdict, option index or matched rule

#### Scenario: A cleared prompt receives no keystrokes

- **GIVEN** the approver classified a live prompt on pane 2 as safe
- **WHEN** its pre-send capture shows no live prompt on pane 2
- **THEN** it SHALL dispatch NO keystrokes to pane 2

#### Scenario: A file-operation prompt whose target changed receives no keystrokes

- **GIVEN** the approver classified a live prompt to edit `src/lib.rs` inside the agent's worktree as a safe worktree file operation
- **AND** before the send it was replaced by a live prompt to edit `~/.zshrc`
- **WHEN** the approver takes its pre-send capture
- **THEN** it SHALL dispatch NO keystrokes to that pane

#### Scenario: A changed option list receives no keystrokes

- **GIVEN** the approver classified a live 3-option prompt for `cargo test` as safe and selected option 2, whose text grants `cargo test` only
- **AND** before the send it was replaced by a prompt for `cargo test` whose option 2 instead grants similar commands across the whole worktree
- **WHEN** the approver takes its pre-send capture
- **THEN** it SHALL dispatch NO keystrokes to that pane

#### Scenario: Separate captures of one unchanged prompt match

- **GIVEN** two captures of the same live prompt that differ only in the selection caret's position and in output above the prompt's header
- **WHEN** their identities are derived
- **THEN** the identities SHALL be equal, so an approval whose pre-send capture is the second capture is sent

#### Scenario: Danger and unknown verdicts never reach the send step

- **GIVEN** a live prompt the classifier returns as danger or unknown
- **WHEN** the approver processes the pane
- **THEN** it SHALL escalate the prompt
- **AND** it SHALL dispatch no approval keystrokes, whatever a later capture of the pane shows

#### Scenario: Both approvers reach the same decision for the same captures

- **GIVEN** a classified capture and a pre-send capture of one pane
- **WHEN** the drive loop and the dashboard poll loop each decide whether to send for that pair
- **THEN** both SHALL reach the same send or no-send decision

#### Scenario: The claim is released after an aborted send

- **GIVEN** an approver aborted a send on pane 2 because the live prompt had changed
- **WHEN** another approver then tries to take pane 2's claim
- **THEN** it SHALL acquire the claim

## MODIFIED Requirements

### Requirement: Approval keystrokes require a re-confirmed live prompt

Any approver that clears an agent CLI permission prompt by sending keystrokes via `tmux send-keys` SHALL pass through a single approval-send gate. Immediately before dispatching the approval keystrokes, the gate SHALL capture the target pane (e.g. `tmux capture-pane -p -t <session>:<pane>`) and SHALL confirm that a live prompt anchors the capture per the **Live-prompt gate**: at the capture's true end, a textual marker within the last 4 non-blank lines, OR a numbered option line anchoring that tail while a textual marker sits within the wider block window above it (so a multi-option prompt whose `Do you want to proceed?` header sits above the last 4 lines is still detected rather than missed); OR, at a bounded offset above the true end (up to 13 non-blank trailing lines below the footer), the modal's actual shape — the footer marker anchoring the START of a line together with a numbered option line within the wider block window above it, AND no idle input-box line (the CLI's composer sigil) anywhere between that footer and the capture's true end — so a modal sitting under a few lines of CLI chrome is detected, while a modal copied verbatim into tool output above the pane's own idle input box is not. Only when a live prompt is re-confirmed, and that same capture shows the prompt that was classified (per "Approval keystrokes reach only the prompt that was classified"), SHALL the gate dispatch the keystrokes.

If the re-confirm capture does NOT show a live prompt per the Live-prompt gate (including the shape requirement at a bounded offset — a bare mention of the footer text with no option list nearby does NOT count, nor does a footer and option list with an idle input-box line below them), the gate SHALL dispatch NO keystrokes to the pane. The capture used at the detection/decision stage SHALL NOT substitute for this re-confirm capture; the re-confirm capture SHALL be taken immediately before the send, with no classification work or broker round-trip between the re-confirm and the send. The prompt-identity comparison SHALL read this same re-confirm capture, so liveness and identity are confirmed on one observation.

A permission-prompt marker matched only in scrollback above the Live-prompt gate's inspected tail/block window, matched only as a bare substring mention with no accompanying option list, or matched above an idle input-box line, SHALL NOT count as a live prompt.

#### Scenario: Approval keys sent only when a live prompt is re-confirmed

- **GIVEN** an approval decision has been made for a pane and approval keystrokes are about to be sent
- **WHEN** the gate's immediate-before-send capture of the pane shows a permission-prompt marker anchoring the tail (per the Live-prompt gate's tail/block window)
- **AND** the prompt identity derived from that capture equals the identity of the prompt that was classified
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
