## MODIFIED Requirements

### Requirement: Auto-approve classifier-safe prompts

When a swept pane shows a live prompt the drive loop SHALL classify it via the `auto-approve-classifier`. When the classification is safe, the loop SHALL approve the prompt by sending the agent CLI's documented "approve and remember" keystroke sequence per the `automatic-approval` capability (each logical key sent as a separate `tmux send-keys`). For a 2-option Yes/No prompt the loop SHALL send `"1"` then `Enter` directly rather than `Down`+`Enter`, because on a 2-option prompt `Down`+`Enter` selects the wrong option.

Every auto-approval SHALL be recorded in the broker message log (per `automatic-approval`) before the keystrokes are sent.

Immediately before sending, on a fresh capture taken while holding the pane's claim, the loop SHALL confirm that the live prompt is the one it classified, per `approval-auto`'s "Approval keystrokes reach only the prompt that was classified". This applies to every pane the loop approves. When the prompt has changed, the loop SHALL send nothing to that pane, SHALL NOT treat the attempt as a dispatched approval, and SHALL leave the new prompt to the next sweep's fresh classification. It SHALL keep sweeping the other panes without blocking.

#### Scenario: Classifier-safe prompt is auto-approved

- **GIVEN** a coding-agent pane showing a live permission prompt for a classifier-safe command (e.g. `cargo test`)
- **WHEN** the drive loop sweeps the pane and its pre-send capture still shows that same prompt
- **THEN** the loop SHALL send the documented approve-and-remember keystroke sequence to that pane
- **AND** the wave SHALL continue without blocking

#### Scenario: Two-option Yes/No prompt is approved with "1" then Enter

- **GIVEN** a live 2-option Yes/No permission prompt classified safe
- **WHEN** the drive loop approves it
- **THEN** the loop SHALL send `"1"` followed by a separate `Enter`
- **AND** SHALL NOT send `Down` then `Enter` for the 2-option prompt

#### Scenario: Approval is logged before keystrokes

- **GIVEN** the drive loop is about to auto-approve a safe prompt
- **WHEN** the action is recorded
- **THEN** the broker log entry SHALL be appended before the `tmux send-keys` call so a crash mid-action still leaves an audit trail

#### Scenario: A prompt replaced between classification and send is not approved

- **GIVEN** the drive loop classified a pane's live `cargo test` prompt as safe
- **AND** before the send that prompt was answered and a live `git push --force origin main` prompt appeared on the same pane
- **WHEN** the loop takes its pre-send capture
- **THEN** it SHALL send no keystrokes to that pane
- **AND** it SHALL NOT treat the attempt as a dispatched approval
- **AND** it SHALL continue sweeping the remaining panes

#### Scenario: The replacement prompt is escalated on the next sweep

- **GIVEN** a send was aborted because a live `git push --force origin main` prompt replaced the classified `cargo test` prompt
- **WHEN** the next sweep finds the `git push --force origin main` prompt still live
- **THEN** the loop SHALL classify it from that sweep's capture as danger and record it as an escalation
- **AND** it SHALL NOT approve it
