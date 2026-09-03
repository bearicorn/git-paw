## MODIFIED Requirements

### Requirement: Supervisor skill — paste-buffer recovery in stall detection

The embedded `supervisor.md` skill SHALL include a paste-buffer recovery sub-case under its existing stall-detection section. The sub-case SHALL instruct the supervisor agent that when a peer agent's `last_seen` has not advanced (or, at launch time, before any heartbeat has arrived) AND a `tmux capture-pane` of that peer's pane shows a paste-buffer indicator or the intended text sitting unsubmitted on the input line, the supervisor SHALL recover by clearing the pane's input line (`tmux send-keys -t <target> C-u`), re-typing the intended text, then sending `Enter` — because a bare `Enter` / `C-m` against a stale input line is a no-op that does not submit the buffered or stale content.

The sub-case SHALL:

1. Identify itself as an additional stall-detection case alongside the existing "idle prompt → likely done" and "thinking/waiting → prompt to self-report" cases.
2. List at least one known paste-buffer indicator pattern. The list SHALL include Claude Code's `Pasted text #N` (where `N` is a number) and SHALL be presented as illustrative-not-exhaustive so the supervisor agent can apply judgment to indicators on other CLIs.
3. Specify the recovery action as clearing the stuck pane's input line (`tmux send-keys -t <pane> C-u`), re-typing the intended text, then `Enter` — and SHALL state that a lone `Enter` / `C-m` against a stale input line is a no-op that does not submit.
4. State that the recovery is safe-by-default — on a non-paste-aware CLI or a misclassified pane, clearing the line and re-typing either reproduces the intended input or leaves a benign prompt.
5. Frame indicator detection as lenient: if a pane shows long buffered or unsubmitted text in the input area without a follow-up response, the supervisor SHOULD attempt the recovery even if the literal indicator string is not on the listed-patterns list.
6. State that paste-buffer recovery SHALL also be applied **proactively at launch time** — the supervisor agent SHALL NOT wait for the `last_seen`-based stall threshold before inspecting agent panes for paste-buffer state. Coding-agent boot prompts are frequently long enough on paste-aware CLIs (e.g. Claude Code v2.1.x) to land in a paste buffer immediately, and waiting 30+ seconds for stall detection wastes the agents' productive time.

#### Scenario: Supervisor skill mentions paste-buffer recovery

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains a heading or sub-section identifying paste-buffer recovery (e.g. `paste-buffer`, `paste buffer`, or equivalent under stall detection)

#### Scenario: Supervisor skill names a known paste-buffer indicator

- **WHEN** the paste-buffer recovery sub-case is inspected
- **THEN** it mentions Claude Code's `Pasted text #N` indicator pattern (or substantively equivalent text)

#### Scenario: Supervisor skill specifies the corrected recovery action

- **WHEN** the paste-buffer recovery sub-case is inspected
- **THEN** it instructs the supervisor agent to use `tmux capture-pane` to inspect the suspected pane
- **AND** it instructs the supervisor agent to recover by clearing the input line (`C-u`), re-typing the intended text, then sending `Enter`
- **AND** it states that a lone `Enter` / `C-m` against a stale input line is a no-op, and that the clear-and-retype recovery is safe-by-default on non-paste-aware CLIs

#### Scenario: Supervisor skill frames indicator detection as lenient

- **WHEN** the paste-buffer recovery sub-case is inspected
- **THEN** it instructs the supervisor agent to apply judgment rather than match a closed list of indicator patterns
- **AND** it covers the heuristic case of long buffered text in the input area without a follow-up response

#### Scenario: Supervisor skill instructs proactive paste-buffer recovery at launch

- **WHEN** the paste-buffer recovery sub-case is inspected
- **THEN** it explicitly instructs the supervisor agent to perform a paste-buffer-recovery sweep proactively at launch (before any `last_seen`-based stall threshold elapses)
- **AND** it explains the rationale (long boot prompts land in paste buffers immediately on paste-aware CLIs)
