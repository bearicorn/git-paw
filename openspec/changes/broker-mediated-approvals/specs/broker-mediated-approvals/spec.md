## ADDED Requirements

### Requirement: Approval keystrokes require a re-confirmed live prompt

Any approver that clears an agent CLI permission prompt by sending keystrokes via `tmux send-keys` SHALL pass through a single approval-send gate. Immediately before dispatching the approval keystrokes, the gate SHALL capture the target pane (e.g. `tmux capture-pane -p -t <session>:<pane>`) and SHALL confirm that a live permission-prompt marker is present within the last 4 non-blank lines of the capture. Only when a live prompt is re-confirmed SHALL the gate dispatch the keystrokes.

If the re-confirm capture does NOT contain a live permission-prompt marker in the last 4 non-blank lines, the gate SHALL dispatch NO keystrokes to the pane. The capture used at the detection/decision stage SHALL NOT substitute for this re-confirm capture; the re-confirm capture SHALL be taken immediately before the send, with no classification work or broker round-trip between the re-confirm and the send.

A permission-prompt marker matched anywhere outside the last 4 non-blank lines (i.e. only in scrollback above the tail) SHALL NOT count as a live prompt.

#### Scenario: Approval keys sent only when a live prompt is re-confirmed

- **GIVEN** an approval decision has been made for a pane and approval keystrokes are about to be sent
- **WHEN** the gate's immediate-before-send capture of the pane shows a permission-prompt marker within the last 4 non-blank lines
- **THEN** the gate SHALL dispatch the approval keystrokes to that pane via `tmux send-keys`

#### Scenario: Cleared prompt receives no stray keys

- **GIVEN** an approval decision was made for a pane while it showed a permission prompt
- **WHEN** the gate's immediate-before-send capture of the pane no longer shows a permission-prompt marker in the last 4 non-blank lines (the prompt has cleared)
- **THEN** the gate SHALL dispatch NO keystrokes to that pane
- **AND** the agent's CLI input SHALL receive no stray text

#### Scenario: A stale marker only in scrollback is not treated as live

- **GIVEN** a pane whose capture contains a permission-prompt marker only in lines above the last 4 non-blank lines (a prompt the agent already answered, scrolled up into history)
- **WHEN** the gate evaluates the re-confirm capture
- **THEN** the gate SHALL treat the prompt as cleared
- **AND** SHALL dispatch NO keystrokes

### Requirement: Blind send-keys excludes the supervisor pane 0

The approval-send gate SHALL refuse to dispatch approval keystrokes to pane index 0 (the supervisor's own pane). When the resolved target pane index is 0, the gate SHALL send no keystrokes and SHALL report that pane 0 is excluded from blind send-keys.

Clearing the supervisor pane's own prompt is a distinct action handled by a non-blind path (the unattended drive loop) and is outside this gate; the blind send-keys gate SHALL NOT type into pane 0 under any classification.

#### Scenario: Pane 0 is never sent blind keystrokes

- **GIVEN** an approval target whose resolved pane index is 0
- **WHEN** the approval-send gate runs, even with a live prompt re-confirmed in pane 0
- **THEN** the gate SHALL dispatch NO keystrokes to pane 0
- **AND** SHALL report that pane 0 is excluded from blind send-keys

#### Scenario: Coding agent panes are still approvable

- **GIVEN** an approval target whose resolved pane index is 2 (a coding-agent pane) with a live prompt re-confirmed
- **WHEN** the approval-send gate runs
- **THEN** the gate SHALL dispatch the approval keystrokes to pane 2

### Requirement: Approval dedup keys on command or agent identity, not prompt text

When the system deduplicates pending or repeated approvals so a single awaiting prompt is not acted on multiple times, it SHALL key the dedup on the command identity (the command text the modal is asking about) and/or the agent/pane identity, or SHALL rely on wait-for-clear (the prompt's disappearance after it is answered). The system SHALL NOT compute the dedup key from the captured prompt's boilerplate/footer text (e.g. the shared "Do you want to proceed?" footer).

#### Scenario: Distinct commands are not collapsed by shared footer

- **GIVEN** two successive permission prompts on the same agent — one for `cargo test` and one for `git push` — that share the identical permission-prompt footer text
- **WHEN** the dedup logic evaluates them
- **THEN** the two prompts SHALL be treated as distinct approval events (keyed on their differing command identity)
- **AND** the second prompt SHALL NOT be suppressed as a duplicate of the first

#### Scenario: Repeated capture of the same unanswered prompt is deduped

- **GIVEN** the same agent remains on the same unanswered permission prompt across consecutive sweep iterations
- **WHEN** the dedup logic evaluates the repeated detection
- **THEN** the repeated detection SHALL be treated as the same approval event (keyed on command/agent identity, or recognised as not-yet-cleared)
- **AND** the system SHALL NOT generate a duplicate approval action for the unchanged prompt

### Requirement: Approval gate reuses existing broker variants

The approval-send gate's approval-trigger signal and its escalation channel SHALL reuse the existing `BrokerMessage` variants defined in `broker-messages`. The trigger that a pane is awaiting approval SHALL be the synthetic `agent.status` with `phase: "stuck-on-prompt"` (per `stuck-prompt-detection`); escalation of an unsafe or unclassifiable prompt SHALL reuse `agent.question` (per `automatic-approval`); supervisor→agent replies SHALL reuse `agent.feedback` or the supervisor `/tell` path. This change SHALL NOT introduce a new `BrokerMessage` variant or a per-CLI permission hook.

#### Scenario: No new broker message variant is introduced

- **WHEN** the broker message envelope is inspected after this change
- **THEN** the set of `BrokerMessage` variants SHALL be unchanged (the seven variants `Status`, `Artifact`, `Blocked`, `Verified`, `Feedback`, `Question`, `Intent`)
- **AND** the approval-trigger and escalation signals SHALL be expressed using those existing variants

#### Scenario: Escalation of an unsafe prompt reuses agent.question

- **GIVEN** a re-confirmed live prompt that the classifier deems unsafe or unknown
- **WHEN** the gate escalates the prompt for human review
- **THEN** the escalation SHALL be published as an `agent.question` whose `agent_id` is the originating agent's slug
- **AND** no new message type SHALL be sent
