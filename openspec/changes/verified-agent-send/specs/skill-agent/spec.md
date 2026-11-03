## MODIFIED Requirements

### Requirement: Embedded supervisor skill

The embedded supervisor skill SHALL include: a Spec Audit procedure run after the test command and
before `agent.verified`; a `Watch peer intents` pointer; an `agent.question` answer step that
publishes the reply AND types it into the asking agent's input with `sweep.sh tell`, because agents
do not poll their inbox; a section on publishing `agent.intent` for main-side work through
`broker.sh`; and an accept-edits `modified_files` audit before merge.

#### Scenario: Supervisor skill contains spec audit procedure

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains the substring `Spec Audit`
- **AND** it contains instructions to read `openspec/changes/` spec files
- **AND** it contains instructions to grep for matching tests
- **AND** it contains instructions to verify field names match spec

#### Scenario: Spec audit runs after tests, before verified

- **WHEN** the embedded supervisor skill workflow is inspected
- **THEN** the spec audit step appears after the test command step
- **AND** the spec audit step appears before the `agent.verified` publish step

#### Scenario: Supervisor skill mentions agent.intent

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains the substring `agent.intent`
- **AND** it contains a heading or section titled `Watch peer intents` (or equivalent)
- **AND** it indicates that automatic conflict-warning logic is not part of this release

#### Scenario: Supervisor skill instructs tmux-send-keys alongside agent.feedback answers

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains explicit guidance that, when answering an `agent.question`, the supervisor MUST publish the reply AND ALSO deliver the answer to the asking agent's input via `sweep.sh tell`
- **AND** it contains the substring `sweep.sh tell` in the section covering `agent.question` answers
- **AND** it states the rationale (agents do not poll their inbox for `agent.feedback` responses)

#### Scenario: Supervisor skill cross-references paste-buffer recovery for long answers

- **WHEN** the supervisor skill's `agent.question` answer step is inspected
- **THEN** it instructs the supervisor to apply the paste-buffer recovery sub-case when the answer does not submit
- **AND** it says what to do when `sweep.sh tell` reports `wedged` or `refused`

#### Scenario: Supervisor skill instructs publishing agent.intent for main-side work

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains a heading or section indicating the supervisor publishes `agent.intent` for main-side work (e.g. `Supervisor publishes agent.intent for main-side work`, `Publish intent before editing main`, or equivalent)
- **AND** it contains a `.git-paw/scripts/broker.sh --agent supervisor intent` example
- **AND** the example supplies the summary, the file list and the TTL the helper sends as `summary`, `files` and `valid_for_seconds`
- **AND** the section explains the visibility gap (agents in feat branches do not see supervisor commits to `main` without an explicit broker event)

#### Scenario: Supervisor skill cross-references the agent-side intent flow

- **WHEN** the supervisor-publishes-intent section is inspected
- **THEN** it cross-references the agent-side `agent.intent` flow in `coordination.md` (e.g. names the `Before you start editing` section or substantively equivalent)
- **AND** it states that the supervisor reuses the same wire format defined for coding agents

#### Scenario: Supervisor skill instructs accept-edits modified_files audit

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains a heading or section instructing the supervisor to verify `accept edits`-mode commits before merge (e.g. `Verify accept-edits commits before merge` or equivalent)
- **AND** it instructs the supervisor to locate the change's expected file list from `proposal.md`
- **AND** it instructs the supervisor to diff `agent.artifact { modified_files }` against the expected list
- **AND** it instructs the supervisor to flag out-of-scope edits via `agent.feedback`

#### Scenario: Supervisor skill forbids silent auto-approval of out-of-scope accept-edits changes

- **WHEN** the supervisor accept-edits audit section is inspected
- **THEN** it states that out-of-scope edits SHALL NOT be silently auto-approved
- **AND** it instructs the supervisor to publish `agent.feedback` for substantive out-of-scope edits asking the agent to revert or justify

### Requirement: Supervisor skill — paste-buffer recovery in stall detection

The supervisor skill SHALL include a paste-buffer recovery sub-case under stall detection, applied
proactively at launch too: when a pane shows a paste-buffer indicator (such as Claude Code's
`Pasted text #N`, illustrative only) or unsubmitted text, the supervisor SHALL re-send the intended
text with `sweep.sh tell`, which re-types and verifies it, and the skill SHALL state that a lone
`Enter` against a stale input line is a no-op.

#### Scenario: Supervisor skill mentions paste-buffer recovery

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains a heading or sub-section identifying paste-buffer recovery (e.g. `paste-buffer`, `paste buffer`, or equivalent under stall detection)

#### Scenario: Supervisor skill names a known paste-buffer indicator

- **WHEN** the paste-buffer recovery sub-case is inspected
- **THEN** it mentions Claude Code's `Pasted text #N` indicator pattern (or substantively equivalent text)

#### Scenario: Supervisor skill specifies the corrected recovery action

- **WHEN** the paste-buffer recovery sub-case is inspected
- **THEN** it instructs the supervisor agent to inspect the suspected pane with `sweep.sh capture`
- **AND** it instructs the supervisor agent to recover by re-sending the intended text with `sweep.sh tell`, which clears (`C-u`) and re-types its own text and sends `Enter` until the input empties
- **AND** it states that a lone `Enter` / `C-m` against a stale input line is a no-op, and that the recovery is safe-by-default on non-paste-aware CLIs

#### Scenario: Supervisor skill frames indicator detection as lenient

- **WHEN** the paste-buffer recovery sub-case is inspected
- **THEN** it instructs the supervisor agent to apply judgment rather than match a closed list of indicator patterns
- **AND** it covers the heuristic case of long buffered text in the input area without a follow-up response

#### Scenario: Supervisor skill instructs proactive paste-buffer recovery at launch

- **WHEN** the paste-buffer recovery sub-case is inspected
- **THEN** it explicitly instructs the supervisor agent to perform a paste-buffer-recovery sweep proactively at launch (before any `last_seen`-based stall threshold elapses)
- **AND** it explains the rationale (long boot prompts land in paste buffers immediately on paste-aware CLIs)
