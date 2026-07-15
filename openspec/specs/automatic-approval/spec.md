# automatic-approval Specification

## Purpose
Defines how the supervisor dispatches approval keystrokes for safe-classified permission prompts: firing only from stall detection, only when a live-prompt marker is present and re-confirmed immediately before send, never into the supervisor's own pane 0, and never taking a permanent broad grant for arbitrary-code runners. Unknown prompts escalate to the human as an `agent.question`, and every auto-approval is logged to the broker for audit.
## Requirements
### Requirement: Auto-approval keystroke sequence

When a detected prompt is classified safe, the system SHALL send the agent CLI's "approve and remember" keystroke sequence to the pane via `tmux send-keys`.

The keystroke send SHALL pass through the `broker-mediated-approvals` approval-send gate: immediately before dispatching the keystroke sequence, the system SHALL re-capture the target pane and confirm the prompt's live structural markers are present at the capture's tail, per the Live-prompt gate — a window wide enough to span a full multi-option prompt block, not a fixed ~4-line tail. When the re-confirm capture shows the prompt has cleared between classification and send, the system SHALL dispatch NO keystrokes (no stray input). The system SHALL also refuse to dispatch the sequence into pane index 0 (the supervisor's own pane) via this blind send-keys path.

#### Scenario: Default Claude approval sequence

- **GIVEN** a Claude pane displaying a permission prompt for an allowlisted curl command
- **WHEN** auto-approval fires AND the immediate-before-send re-confirm capture shows the prompt is still live (structural markers present at the tail)
- **THEN** the system SHALL send the resolved option digit followed by `Enter` (per "Option-index selection for Yes/No prompts") to the pane via `tmux send-keys -t <session>:<pane>`

#### Scenario: Each keystroke sent separately

- **GIVEN** auto-approval is firing against a re-confirmed live prompt
- **WHEN** the keystrokes are dispatched
- **THEN** the system SHALL invoke `tmux send-keys` once per logical key (the resolved option digit, then `Enter`) rather than as a single concatenated string

#### Scenario: Cleared prompt suppresses the keystroke sequence

- **GIVEN** a prompt that classified safe at detection time
- **WHEN** the immediate-before-send re-confirm capture no longer shows the prompt's live structural markers at the tail
- **THEN** the system SHALL dispatch NO keystrokes to the pane
- **AND** the agent's CLI input SHALL receive no stray characters

#### Scenario: Auto-approval never types into pane 0

- **GIVEN** a safe-classified prompt whose resolved target pane index is 0
- **WHEN** auto-approval would fire
- **THEN** the system SHALL dispatch NO keystrokes via the blind send-keys path
- **AND** the prompt SHALL be left for the non-blind supervisor-pane path

### Requirement: No auto-approval for unsafe or unknown classes

The auto-approver SHALL only fire when classification returns a known-safe class AND the prompt is LIVE (see the "Live-prompt gate" requirement below). When the prompt is not live, or the classification is `Unknown` (or any other non-safe class), auto-approval SHALL NOT fire.

When auto-approval declines to fire because the classification is `Unknown` (or any other non-safe class), the prompt SHALL be surfaced to the human via the supervisor pane — by publishing an `agent.question` whose `agent_id` is the originating coding-agent slug and whose `payload.question` describes the unclassified prompt. The supervisor agent (running inside its own tmux pane) consumes that question from its inbox and replies via `tmux send-keys` to the agent pane. The dashboard's role is observation only — the v0.4 "prompts inbox" panel that surfaced these questions inline was removed in this change (see the `dashboard` capability's "No prompt-inbox panel" requirement).

#### Scenario: Unknown class is left for human

- **GIVEN** a detected prompt classified `PermissionType::Unknown`
- **WHEN** the supervisor poll loop runs
- **THEN** auto-approval SHALL NOT fire
- **AND** the prompt SHALL be surfaced to the human via the supervisor pane (typically by publishing an `agent.question` to the broker, which the supervisor agent then handles in its own pane)

#### Scenario: Disabled config disables firing

- **GIVEN** `[supervisor.auto_approve] enabled = false`
- **WHEN** any detected prompt arrives
- **THEN** auto-approval SHALL NOT fire regardless of classification

#### Scenario: Safe class with non-live prompt does not fire

- **GIVEN** a capture whose command slice classifies safe but whose footer marker `Esc to cancel` is NOT in the last ~4 non-blank lines
- **WHEN** the supervisor poll loop runs
- **THEN** auto-approval SHALL NOT fire (the prompt is not live)

### Requirement: Auto-approval is logged

Every auto-approval action SHALL be recorded in the broker message log so the human can audit decisions after the session.

#### Scenario: Approval emits broker message

- **GIVEN** auto-approval fires for agent `feat-foo` for a `cargo test` prompt
- **WHEN** the keystrokes are sent
- **THEN** the broker SHALL receive an `agent.status` (or equivalent log) message tagged `auto_approved` containing the agent id and the matched whitelist entry

#### Scenario: Logged before keystrokes

- **GIVEN** auto-approval fires
- **WHEN** the action is recorded
- **THEN** the log entry SHALL be appended before the `tmux send-keys` call so a crash mid-action still leaves an audit trail

### Requirement: Integration with stall detection

Auto-approval SHALL be triggered from the stall-detection loop, not run as an independent timer, so it only fires when an agent is genuinely stuck.

#### Scenario: Healthy agent not approved

- **GIVEN** an agent that is publishing status updates within the stall threshold
- **WHEN** the supervisor poll loop runs
- **THEN** detection SHALL NOT capture its pane and auto-approval SHALL NOT fire

#### Scenario: Stalled agent approved

- **GIVEN** an agent whose `last_seen` is older than the configured stall threshold
- **WHEN** the supervisor poll loop runs
- **THEN** detection SHALL capture its pane and, if classification is safe, auto-approval SHALL fire

### Requirement: Live-prompt gate

The auto-approver SHALL act on a prompt only when it is LIVE, and SHALL reliably detect both single- and multi-option permission prompts. A prompt is live when the tail of the pane capture contains the prompt's structural markers — the numbered Yes/No option glyphs and/or the `Do you want to proceed?` marker, together with the `Esc to cancel` footer. The inspected window SHALL be wide enough to span a full multi-option prompt block (`Do you want to proceed?` + N options + footer), so a prompt offering a "don't ask again" option is detected rather than missed. When these markers are absent from the tail, the system SHALL treat the capture as containing no live prompt and SHALL NOT send any keystrokes, regardless of classification — preventing a supervisor that is merely narrating about a pane (or a scrolled-away earlier prompt) from tripping a phantom approval.

Before dispatching approval keystrokes, the sender SHALL re-confirm the prompt is still live on a FRESH capture taken immediately prior to sending. If the prompt has cleared in the interim (the agent moved on, or another approver won the race), the sender SHALL send nothing, so approval digits can never land as stray chat input.

The in-tool auto-approver and the bundled `sweep.sh approve` helper SHALL use the same live-prompt markers so their detection agrees.

#### Scenario: Live single-option prompt fires

- **GIVEN** a capture whose tail includes `Esc to cancel` and whose command slice classifies safe
- **WHEN** the poll loop runs
- **THEN** the live-prompt gate SHALL pass and auto-approval MAY fire

#### Scenario: Live multi-option prompt is detected

- **GIVEN** a capture whose tail is a multi-option permission prompt (`Do you want to proceed?`, a numbered option list including a "don't ask again" option, and `Esc to cancel`)
- **WHEN** the poll loop (or `sweep.sh approve`) inspects it
- **THEN** the prompt SHALL be detected as live — the detection SHALL NOT report "no live prompt" merely because `Do you want to proceed?` sits above a narrow tail window

#### Scenario: Footer absent does not fire

- **GIVEN** a capture that mentions a safe command in prose but whose tail does NOT contain the prompt's option glyphs or `Esc to cancel`
- **WHEN** the poll loop runs
- **THEN** the live-prompt gate SHALL fail and the system SHALL NOT send keystrokes

#### Scenario: Prompt cleared before send sends nothing

- **GIVEN** the gate classified a prompt live and decided to approve
- **WHEN** a fresh capture taken immediately before sending shows the prompt has cleared
- **THEN** the sender SHALL send no keystrokes, so no approval digit lands as stray chat input

#### Scenario: Detection agrees across auto-approver and sweep helper

- **WHEN** the same multi-option prompt capture is evaluated by the in-tool auto-approver and by `sweep.sh approve`
- **THEN** both SHALL agree it is a live prompt (shared marker set)

### Requirement: Worktree-confined git add and git commit pre-approval

The classifier SHALL pre-approve `git add` and `git commit` prompts when the agent's working directory resolves inside its worktree root, using the same canonicalize-then-`starts_with(worktree_root)` boundary check as worktree-confined file edits. This pre-approval SHALL allow an unattended agent to stage and commit its own work without stalling on the approval prompt. `git push` SHALL NOT be covered by this requirement — it is on the danger-list and SHALL always escalate.

#### Scenario: Worktree git commit auto-approves

- **GIVEN** an agent whose cwd canonicalizes inside its worktree root
- **AND** a live prompt whose command slice is `git commit -m "feat: x"`
- **WHEN** the classifier runs
- **THEN** the command SHALL classify safe-by-pattern and auto-approval SHALL dispatch the approval keystrokes

#### Scenario: Worktree git add auto-approves

- **GIVEN** the same in-worktree agent
- **AND** a live prompt whose command slice is `git add -A`
- **WHEN** the classifier runs
- **THEN** the command SHALL classify safe-by-pattern and auto-approval SHALL fire

#### Scenario: git push still escalates despite worktree confinement

- **GIVEN** the same in-worktree agent
- **AND** a live prompt whose command slice is `git push`
- **WHEN** the classifier runs
- **THEN** the danger-list SHALL win and the classifier SHALL escalate; auto-approval SHALL NOT fire

### Requirement: Broad grant restricted to allowlisted non-arbitrary-code verbs

When a prompt offers the permanent broad grant option ("Yes, and don't ask again for: X"), the auto-approver SHALL select that option ONLY when X's leading verb is in the read-mostly allowlist (per `safe-command-classification`) AND X is NOT an arbitrary-code runner. Arbitrary-code runners SHALL include `python`, `bash -c`, `sh -c`, `eval`, `node`, and any command containing a bare ` -c ` code-string flag. For an arbitrary-code runner the auto-approver SHALL select the one-time "Yes" option and SHALL NEVER select the permanent broad grant.

#### Scenario: Allowlisted verb takes the broad grant

- **GIVEN** a live 3-option prompt for `git status` offering "Yes, and don't ask again for: git status"
- **WHEN** the auto-approver fires
- **THEN** it SHALL select the broad-grant option (option 2)

#### Scenario: python -c never gets a permanent broad grant

- **GIVEN** a live 3-option prompt for `python3 -c "import os; os.remove('x')"`
- **WHEN** the auto-approver fires
- **THEN** it SHALL select the one-time "Yes" option (option 1)
- **AND** it SHALL NOT select the permanent broad-grant option

#### Scenario: bash -c never gets a permanent broad grant

- **GIVEN** a live 3-option prompt for `bash -c "do-thing"`
- **WHEN** the auto-approver fires
- **THEN** it SHALL select the one-time "Yes" option and SHALL NOT take the broad grant

### Requirement: Option-index selection for Yes/No prompts

When dispatching approval keystrokes, the auto-approver SHALL select the option index according to the prompt shape. For a 2-option Yes/No prompt, option 1 SHALL be "Yes" and SHALL be selected. For a 3-option Yes / Yes-and-don't-ask-again / No prompt, the auto-approver SHALL select option 2 (the broad grant) only when the broad-grant rule permits (per "Broad grant restricted to allowlisted non-arbitrary-code verbs"), and otherwise SHALL select option 1 (one-time Yes).

The bundled `sweep.sh approve` helper SHALL follow the same option-index selection: it SHALL parse the numbered option list from the fresh pre-send capture and dispatch the selected option's digit followed by `Enter`. It SHALL NOT dispatch a blind cursor-movement sequence (e.g. `Down` + `Enter`) whose landing option depends on the prompt shape — on a 2-option Yes/No prompt such a sequence selects "No", and on a 3-option prompt it takes the permanent broad grant without consulting the broad-grant rule.

#### Scenario: Two-option prompt selects Yes

- **GIVEN** a live 2-option Yes/No prompt for a safe command
- **WHEN** the auto-approver fires
- **THEN** it SHALL select option 1 (Yes)

#### Scenario: Three-option prompt with allowlisted verb selects the broad grant

- **GIVEN** a live 3-option prompt for an allowlisted non-arbitrary-code command
- **WHEN** the auto-approver fires
- **THEN** it SHALL select option 2 (Yes, and don't ask again)

#### Scenario: Three-option prompt with arbitrary-code runner selects one-time Yes

- **GIVEN** a live 3-option prompt for `python3 -c "..."`
- **WHEN** the auto-approver fires
- **THEN** it SHALL select option 1 (one-time Yes), not option 2

#### Scenario: Sweep helper approves a 2-option prompt affirmatively

- **GIVEN** a live 2-option Yes/No prompt on an agent pane
- **WHEN** the operator runs `sweep.sh approve <pane>`
- **THEN** the helper SHALL send the digit `1` then `Enter` (selecting Yes)
- **AND** SHALL NOT send `Down` + `Enter`

#### Scenario: Sweep helper respects the broad-grant rule on 3-option prompts

- **GIVEN** a live 3-option prompt for an arbitrary-code runner (e.g. `bash -c "..."`)
- **WHEN** the operator runs `sweep.sh approve <pane>`
- **THEN** the helper SHALL select option 1 (one-time Yes), agreeing with the in-tool auto-approver's option resolution

