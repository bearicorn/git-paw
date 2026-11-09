## MODIFIED Requirements

### Requirement: Supervisor skill — Merge orchestration

The embedded orchestrator skill (`assets/agent-skills/orchestrator.md`, formerly `supervisor.md`) SHALL include a "Merge orchestration" section that replaces the v0.4 Rust `run_merge_loop` function. The section SHALL instruct the orchestrator to perform merge orchestration via existing mechanisms (the bundled helpers or the MCP tools, shell, git, the configured `test_command` — never a raw `curl`) and SHALL cover:

1. **When to merge** — once all expected agents have published `agent.verified` (or after the user explicitly asks the orchestrator to merge). In an unattended run (the session context carries the drive-loop directive) the orchestrator SHALL merge as soon as this trigger holds, fast-forward only, without first asking the human, and SHALL hold a branch back and escalate only on a conflict (a non-fast-forward or conflicting branch, or a dependency cycle among the branches), an exhausted correction cycle, or a regression: a conflicting branch is skipped and a regressing merge reverted, each with `agent.feedback` to the branch's agent (item 3); a dependency cycle (item 4) and a branch whose correction budget is exhausted are escalated with `agent.question`; every held-back branch is named in the final summary (item 5). In an attended run the orchestrator MAY confirm the merge order with the human before merging. The skill SHALL NOT carry an unconditional rule to ask the human before every merge.
2. **Compute merge order from `agent.blocked` events** — read the broker's message log through `sweep.sh inbox` / `sweep.sh events` or the MCP read tools to find `agent.blocked` events. For each `agent.blocked` from X with `payload.from = Y`, treat as edge "X depends on Y". Topologically sort the dependency graph; agents with no incoming edges merge first.
3. **Per-branch merge loop**:
   - Check out the merge target (the integration branch the session merges into, resolved at run time, never a hardcoded name).
   - Fast-forward merge the agent's resolved branch: `git merge --ff-only <agent-branch>`. Never create merge commits.
   - Confirm the merge landed: `git merge-base --is-ancestor <agent-branch-tip> HEAD`, per "Orchestrator skill confirms a merge landed before archiving". A failed check means the merge did not land.
   - On non-FF / conflict: SKIP the merge for this branch; publish `agent.feedback` to the branch's agent listing the conflict and asking them to rebase or resolve. Continue with the next branch.
   - On FF success: run the configured `test_command` (from `[supervisor].test_command`). On failure: `git reset --hard <previous-HEAD>` to revert; publish `agent.feedback` listing the regression. On success: continue to the next branch.
4. **Cycle handling** — if the dependency graph has cycles, publish `agent.question` (through `broker.sh --agent supervisor question`) surfacing the cycle and asking the user how to proceed. Do NOT merge any branch in the cycle until the user resolves it.
5. **Final summary** — when all eligible branches are merged (or skipped), publish a final `agent.status` from `agent_id = "supervisor"` summarising what was merged, what was skipped, and any regressions encountered.

#### Scenario: Supervisor skill mentions merge orchestration

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains a heading or section identifying merge orchestration (e.g. `Merge orchestration`)

#### Scenario: Supervisor skill names the trigger condition

- **WHEN** the embedded supervisor skill's merge-orchestration section is inspected
- **THEN** it states that merge orchestration runs when all expected agents have published `agent.verified` (or on user request)

#### Scenario: An unattended merge follows the trigger without asking first

- **WHEN** the merge-orchestration section is inspected
- **THEN** it SHALL state that, in an unattended run, merging proceeds once every expected agent is verified, fast-forward only, without asking the human first
- **AND** it SHALL name the only escalations: a conflict (non-fast-forward or dependency cycle), an exhausted correction cycle, and a regression
- **AND** the skill SHALL NOT contain an unconditional "ask the human before merging" rule

#### Scenario: An attended merge may confirm with the human

- **WHEN** the merge-orchestration section is inspected
- **THEN** it SHALL state that in an attended run the orchestrator may confirm the merge order with the human before merging

#### Scenario: Supervisor skill describes the topological-order computation

- **WHEN** the merge-orchestration section is inspected
- **THEN** it instructs the supervisor to read `agent.blocked` messages
- **AND** it states that an `agent.blocked` from X with `payload.from = Y` defines a dependency edge (X depends on Y)
- **AND** it instructs the supervisor to topologically sort the resulting dependency graph

#### Scenario: Supervisor skill describes the per-branch merge + test loop

- **WHEN** the merge-orchestration section is inspected
- **THEN** it instructs the supervisor to use `git merge --ff-only`
- **AND** it instructs the supervisor to confirm each merge with `git merge-base --is-ancestor` before archiving or publishing
- **AND** it instructs the supervisor to run the configured `test_command` after each successful merge
- **AND** it instructs the supervisor to revert (`git reset --hard`) and publish `agent.feedback` on test failure
- **AND** it instructs the supervisor to skip and publish `agent.feedback` on merge conflict / non-FF
- **AND** it SHALL NOT contain `git checkout main` or `git merge --ff-only feat/<branch>`

#### Scenario: Supervisor skill describes cycle handling

- **WHEN** the merge-orchestration section is inspected
- **THEN** it instructs the supervisor to escalate via `agent.question` when the dependency graph has cycles
- **AND** it instructs the supervisor NOT to merge cycle members until the user resolves the cycle

#### Scenario: Supervisor skill describes the final summary

- **WHEN** the merge-orchestration section is inspected
- **THEN** it instructs the supervisor to publish a final `agent.status` summary after merge orchestration completes
- **AND** the summary covers what was merged, what was skipped, and any regressions
