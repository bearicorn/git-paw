## ADDED Requirements

### Requirement: Supervisor skill — interactive user input

The embedded `supervisor.md` skill SHALL include a "When the user types in your pane" section instructing the supervisor agent how to handle user input that arrives while the autonomous monitoring loop is running. The section SHALL:

1. State that the supervisor pane is interactive — the user can type questions or directives at any point during the session.
2. Distinguish three cases of user input and map each to existing mechanisms:
   - **Status question** ("how's X going?", "what are the agents working on?") — answer conversationally using `curl /status`, `curl /messages/supervisor`, and `tmux capture-pane`. Do NOT publish; just respond.
   - **Directive** ("ask X to use bcrypt", "tell Y to skip the migration") — publish `agent.feedback` to the named agent (or use `tmux send-keys` for low-stakes nudges) AND confirm the action conversationally to the user.
   - **Judgment-call ask** ("should we merge feat-a before feat-b?") — apply the supervisor's normal escalation rules; only publish `agent.question` to the dashboard if the call is genuinely ambiguous beyond what the user just provided. Otherwise answer conversationally with the supervisor's reasoning.
3. State that the autonomous loop continues alongside user input — if the supervisor is mid-spec-audit when the user asks something, finish the current step, then respond, then resume the loop.
4. The mechanisms (curl, tmux capture-pane, agent.feedback, tmux send-keys, agent.question) are unchanged from the existing supervisor skill; the addition is *when to use which* in response to user input.

#### Scenario: Supervisor skill mentions interactive user input

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains a heading or section identifying user input handling (e.g. `When the user types in your pane`)

#### Scenario: Supervisor skill names the three input cases

- **WHEN** the embedded supervisor skill's user-input section is inspected
- **THEN** it identifies the status-question case and maps it to `/status` / `/messages/supervisor` / `tmux capture-pane`
- **AND** it identifies the directive case and maps it to `agent.feedback` / `tmux send-keys`
- **AND** it identifies the judgment-call case and maps it to the supervisor's normal escalation (with `agent.question` only when ambiguous)

#### Scenario: Supervisor skill states the autonomous loop continues alongside user input

- **WHEN** the embedded supervisor skill's user-input section is inspected
- **THEN** it explicitly states that the autonomous monitoring loop continues alongside user input
- **AND** it instructs the supervisor to finish the current step before responding

### Requirement: Supervisor skill — Merge orchestration

The embedded `supervisor.md` skill SHALL include a "Merge orchestration" section that replaces the v0.4 Rust `run_merge_loop` function. The section SHALL instruct the supervisor agent to perform merge orchestration via existing mechanisms (curl, shell, git, the configured `test_command`) and SHALL cover:

1. **When to merge** — once all expected agents have published `agent.verified` (or after the user explicitly asks the supervisor to merge).
2. **Compute merge order from `agent.blocked` events** — read `curl /messages/supervisor` (or the broker's message log via the dashboard's view) to find `agent.blocked` events. For each `agent.blocked` from X with `payload.from = Y`, treat as edge "X depends on Y". Topologically sort the dependency graph; agents with no incoming edges merge first.
3. **Per-branch merge loop**:
   - Checkout main: `git checkout main`.
   - Fast-forward merge: `git merge --ff-only feat/<branch>`. Never create merge commits.
   - On non-FF / conflict: SKIP the merge for this branch; publish `agent.feedback` to the branch's agent listing the conflict and asking them to rebase or resolve. Continue with the next branch.
   - On FF success: run the configured `test_command` (from `[supervisor].test_command`). On failure: `git reset --hard <previous-HEAD>` to revert; publish `agent.feedback` listing the regression. On success: continue to the next branch.
4. **Cycle handling** — if the dependency graph has cycles, publish `agent.question` to the dashboard surfacing the cycle and asking the user how to proceed. Do NOT merge any branch in the cycle until the user resolves it.
5. **Final summary** — when all eligible branches are merged (or skipped), publish a final `agent.status` from `agent_id = "supervisor"` summarising what was merged, what was skipped, and any regressions encountered.

#### Scenario: Supervisor skill mentions merge orchestration

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains a heading or section identifying merge orchestration (e.g. `Merge orchestration`)

#### Scenario: Supervisor skill names the trigger condition

- **WHEN** the embedded supervisor skill's merge-orchestration section is inspected
- **THEN** it states that merge orchestration runs when all expected agents have published `agent.verified` (or on user request)

#### Scenario: Supervisor skill describes the topological-order computation

- **WHEN** the merge-orchestration section is inspected
- **THEN** it instructs the supervisor to read `agent.blocked` messages
- **AND** it states that an `agent.blocked` from X with `payload.from = Y` defines a dependency edge (X depends on Y)
- **AND** it instructs the supervisor to topologically sort the resulting dependency graph

#### Scenario: Supervisor skill describes the per-branch merge + test loop

- **WHEN** the merge-orchestration section is inspected
- **THEN** it instructs the supervisor to use `git merge --ff-only`
- **AND** it instructs the supervisor to run the configured `test_command` after each successful merge
- **AND** it instructs the supervisor to revert (`git reset --hard`) and publish `agent.feedback` on test failure
- **AND** it instructs the supervisor to skip and publish `agent.feedback` on merge conflict / non-FF

#### Scenario: Supervisor skill describes cycle handling

- **WHEN** the merge-orchestration section is inspected
- **THEN** it instructs the supervisor to escalate via `agent.question` when the dependency graph has cycles
- **AND** it instructs the supervisor NOT to merge cycle members until the user resolves the cycle

#### Scenario: Supervisor skill describes the final summary

- **WHEN** the merge-orchestration section is inspected
- **THEN** it instructs the supervisor to publish a final `agent.status` summary after merge orchestration completes
- **AND** the summary covers what was merged, what was skipped, and any regressions
