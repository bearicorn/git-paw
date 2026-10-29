## MODIFIED Requirements

### Requirement: Supervisor skill — Merge orchestration

The embedded orchestrator skill (`assets/agent-skills/orchestrator.md`, formerly `supervisor.md`) SHALL include a "Merge orchestration" section that replaces the v0.4 Rust `run_merge_loop` function. The section SHALL instruct the orchestrator to perform merge orchestration through `git paw finish` run from the repository root, plus the broker for ordering and questions (through the bundled helpers or the MCP tools — never a raw `curl`), and SHALL cover:

1. **When to merge** — once all expected agents have published `agent.verified` (or after the user explicitly asks the orchestrator to merge). In an unattended run (the session context carries the drive-loop directive) the orchestrator SHALL integrate as soon as this trigger holds, through the queue (item 3), without first asking the human, and SHALL hold a branch back and escalate only on a conflict (a branch the queue skips, or a dependency cycle among the branches), an exhausted correction cycle, or a regression (a rebased branch whose re-test fails): a branch the queue skips, a regressing one included, gets the queue's one rebase directive (item 3); a dependency cycle (item 4) and a branch whose correction budget is exhausted are escalated with `agent.question`; every held-back branch is named in the final summary (item 5). In an attended run the orchestrator MAY confirm the merge order with the human before merging. The skill SHALL NOT carry an unconditional rule to ask the human before every merge.
2. **Compute merge order from `agent.blocked` events** — read the broker's message log through `sweep.sh inbox` / `sweep.sh events` or the MCP read tools to find `agent.blocked` events. For each `agent.blocked` from X with `payload.from = Y`, treat as edge "X depends on Y". Topologically sort the dependency graph; agents with no incoming edges merge first.
3. **Integrate the ordered batch** — run `git paw finish --rebase <branch> <branch> …` with the branches in topological order (or `git paw finish --verified --rebase` when no ordering constraint exists). The command fast-forwards each branch, rebases and re-tests provably disjoint branches itself, never creates merge commits unless the project configured them, tears down each integrated agent, and sends one rebase directive to every skipped agent. Exit `0` means every branch integrated; exit `3` means some were skipped (read the per-branch report); exit `1` is an error to surface to the user. The skill SHALL NOT instruct raw `git checkout`, `git merge` or `git reset --hard` on the target branch.
4. **Cycle handling** — if the dependency graph has cycles, publish `agent.question` (through `broker.sh --agent supervisor question`) surfacing the cycle and asking the user how to proceed. Do NOT merge any branch in the cycle until the user resolves it.
5. **Final summary** — when the batch is done, publish a final `agent.status` from `agent_id = "supervisor"` summarising what was integrated, what was skipped and why, taken from the command's report.

#### Scenario: Supervisor skill mentions merge orchestration

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains a heading or section identifying merge orchestration (e.g. `Merge orchestration`)

#### Scenario: Supervisor skill names the trigger condition

- **WHEN** the embedded supervisor skill's merge-orchestration section is inspected
- **THEN** it states that merge orchestration runs when all expected agents have published `agent.verified` (or on user request)

#### Scenario: An unattended merge follows the trigger without asking first

- **WHEN** the merge-orchestration section is inspected
- **THEN** it SHALL state that, in an unattended run, integration through `git paw finish` proceeds once every expected agent is verified, without asking the human first
- **AND** it SHALL name the only escalations: a conflict (a branch the queue skips, or a dependency cycle), an exhausted correction cycle, and a regression
- **AND** the skill SHALL NOT contain an unconditional "ask the human before merging" rule

#### Scenario: An attended merge may confirm with the human

- **WHEN** the merge-orchestration section is inspected
- **THEN** it SHALL state that in an attended run the orchestrator may confirm the merge order with the human before merging

#### Scenario: Supervisor skill describes the topological-order computation

- **WHEN** the merge-orchestration section is inspected
- **THEN** it instructs the orchestrator to read `agent.blocked` messages
- **AND** it states that an `agent.blocked` from X with `payload.from = Y` defines a dependency edge (X depends on Y)
- **AND** it instructs the orchestrator to topologically sort the resulting dependency graph

#### Scenario: Supervisor skill integrates through the finish queue

- **WHEN** the merge-orchestration section is inspected
- **THEN** it instructs the orchestrator to run `git paw finish --rebase` with the ordered branches from the repository root
- **AND** it explains exit codes `0`, `3` and `1`
- **AND** it does not instruct `git checkout`, `git merge --ff-only` or `git reset --hard` against the target branch

#### Scenario: Supervisor skill describes cycle handling

- **WHEN** the merge-orchestration section is inspected
- **THEN** it instructs the orchestrator to escalate via `agent.question` when the dependency graph has cycles
- **AND** it instructs the orchestrator NOT to merge cycle members until the user resolves the cycle

#### Scenario: Supervisor skill describes the final summary

- **WHEN** the merge-orchestration section is inspected
- **THEN** it instructs the orchestrator to publish a final `agent.status` summary after merge orchestration completes
- **AND** the summary covers what was integrated, what was skipped, and why
