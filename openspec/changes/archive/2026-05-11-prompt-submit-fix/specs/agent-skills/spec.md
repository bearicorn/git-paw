## ADDED Requirements

### Requirement: Supervisor skill — paste-buffer recovery in stall detection

The embedded `supervisor.md` skill SHALL include a paste-buffer recovery sub-case under its existing stall-detection section. The sub-case SHALL instruct the supervisor agent that when a peer agent's `last_seen` has not advanced (or, at launch time, before any heartbeat has arrived) AND a `tmux capture-pane` of that peer's pane shows a paste-buffer indicator, the supervisor SHALL send a recovery `tmux send-keys -t <target> Enter` to submit the buffered content.

The sub-case SHALL:

1. Identify itself as an additional stall-detection case alongside the existing "idle prompt → likely done" and "thinking/waiting → prompt to self-report" cases.
2. List at least one known paste-buffer indicator pattern. The list SHALL include Claude Code's `Pasted text #N` (where `N` is a number) and SHALL be presented as illustrative-not-exhaustive so the supervisor agent can apply judgment to indicators on other CLIs.
3. Specify the recovery action as `tmux send-keys -t <pane> Enter` (a single Enter keystroke to the stuck pane).
4. State that the recovery action is safe-by-default — on a non-paste-aware CLI or a misclassified pane, the extra Enter either produces a benign blank prompt or is ignored.
5. Frame indicator detection as lenient: if a pane shows long buffered text in the input area without a follow-up response, the supervisor SHOULD attempt the recovery even if the literal indicator string is not on the listed-patterns list.
6. State that paste-buffer recovery SHALL also be applied **proactively at launch time** — the supervisor agent SHALL NOT wait for the `last_seen`-based stall threshold before inspecting agent panes for paste-buffer state. Coding-agent boot prompts are frequently long enough on paste-aware CLIs (e.g. Claude Code v2.1.x) to land in a paste buffer immediately, and waiting 30+ seconds for stall detection wastes the agents' productive time.

#### Scenario: Supervisor skill mentions paste-buffer recovery

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains a heading or sub-section identifying paste-buffer recovery (e.g. `paste-buffer`, `paste buffer`, or equivalent under stall detection)

#### Scenario: Supervisor skill names a known paste-buffer indicator

- **WHEN** the paste-buffer recovery sub-case is inspected
- **THEN** it mentions Claude Code's `Pasted text #N` indicator pattern (or substantively equivalent text)

#### Scenario: Supervisor skill specifies the recovery action

- **WHEN** the paste-buffer recovery sub-case is inspected
- **THEN** it instructs the supervisor agent to use `tmux capture-pane` to inspect the suspected pane
- **AND** it instructs the supervisor agent to send `tmux send-keys -t <pane> Enter` to recover
- **AND** it states that the Enter keystroke is safe-by-default (no-op or benign blank prompt on non-paste-aware CLIs)

#### Scenario: Supervisor skill frames indicator detection as lenient

- **WHEN** the paste-buffer recovery sub-case is inspected
- **THEN** it instructs the supervisor agent to apply judgment rather than match a closed list of indicator patterns
- **AND** it covers the heuristic case of long buffered text in the input area without a follow-up response

#### Scenario: Supervisor skill instructs proactive paste-buffer recovery at launch

- **WHEN** the paste-buffer recovery sub-case is inspected
- **THEN** it explicitly instructs the supervisor agent to perform a paste-buffer-recovery sweep proactively at launch (before any `last_seen`-based stall threshold elapses)
- **AND** it explains the rationale (long boot prompts land in paste buffers immediately on paste-aware CLIs)

### Requirement: Supervisor skill — proactive permission-prompt handling at launch

The embedded `supervisor.md` skill SHALL include explicit guidance for the supervisor agent's initial launch-time monitoring sweep. The guidance SHALL:

1. State that immediately after attaching to the session, the supervisor SHALL inspect every coding-agent pane via `tmux capture-pane`.
2. Classify what each pane is showing into the categories:
   - `paste-buffer state` → apply the paste-buffer recovery sub-case (above).
   - `permission prompt` (e.g. `This command requires approval`, `Do you want to proceed?`) → classify the pending command and act per the safe-command policy (below).
   - `working / esc to interrupt` → leave alone, agent is doing its thing.
   - `idle / ? for shortcuts` → agent has finished or never started; investigate.
3. For panes showing a permission prompt, classify the pending command:
   - **Safe-by-pattern**: matches the existing auto-approve safe-command whitelist (`curl http://127.0.0.1:<broker_port>/...`, `cargo fmt|clippy|test|build`, `git commit`, `git push`, plus any `safe_commands` from `[supervisor.auto_approve]` in config). The supervisor SHALL select the "Yes, and don't ask again" option (typically `Down` + `Enter`) so the pattern is permanently allowed for that agent.
   - **Confined-to-worktree**: file edits / reads / `git -C <worktree>` operations within the agent's own worktree are safe; select "Yes, allow all edits" or equivalent (`Down` + `Enter`).
   - **Unknown / wider scope**: anything else SHALL escalate via `agent.question` to the dashboard, not be auto-approved.
4. State that this proactive sweep complements (does NOT replace) the existing `[supervisor.auto_approve]` background poll thread; the proactive sweep handles permissions that appear within the first few seconds of launch when the poll thread's stall threshold has not yet elapsed.

#### Scenario: Supervisor skill mentions proactive launch-time permission sweep

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains a heading or section instructing the supervisor to inspect every pane immediately after attaching (e.g. "launch-time sweep", "initial pane inspection", or equivalent)

#### Scenario: Supervisor skill enumerates the four pane categories

- **WHEN** the launch-time sweep section is inspected
- **THEN** it identifies the four pane categories: paste-buffer state, permission prompt, working, and idle
- **AND** it maps each category to a default action

#### Scenario: Supervisor skill describes the safe-command auto-approve heuristic

- **WHEN** the launch-time sweep section is inspected
- **THEN** it instructs the supervisor to recognise `curl http://127.0.0.1:` as a broker call and approve via "Yes, and don't ask again"
- **AND** it states that the "don't ask again" option is preferred so future broker calls auto-allow without further intervention
- **AND** it mentions that file edits and `git -C <worktree>` operations confined to the agent's own worktree are also safe-by-default

#### Scenario: Supervisor skill escalates unknown permission prompts

- **WHEN** the launch-time sweep section is inspected
- **THEN** it instructs the supervisor to escalate via `agent.question` (rather than auto-approve) for any permission prompt that does not match the safe-command or confined-to-worktree patterns

#### Scenario: Supervisor skill says proactive sweep complements the auto-approve thread

- **WHEN** the launch-time sweep section is inspected
- **THEN** it explicitly states that the proactive sweep complements (does NOT replace) the existing `[supervisor.auto_approve]` background poll thread
- **AND** it explains the rationale (proactive sweep covers the first-few-seconds window before the poll thread's stall threshold elapses)
