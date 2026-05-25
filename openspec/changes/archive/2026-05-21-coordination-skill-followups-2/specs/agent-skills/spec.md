## ADDED Requirements

### Requirement: Coordination skill SHALL teach per-group commit cadence

The embedded `assets/agent-skills/coordination.md` skill SHALL contain a section (heading text approximately "Commit cadence" or "Per-group commit cadence") that instructs the coding agent to commit after completing each numbered task group (e.g. `## 1.`, `## 2.`) in the change's `tasks.md`. The section SHALL state:

1. The default unit of commit is the task GROUP, not the individual task. After all `- [ ]` items in a group are `- [x]`, the agent SHALL commit before starting the next group.
2. The agent SHALL NOT accumulate more than approximately ten uncommitted files at a time. If a single group's implementation produces more uncommitted files than that, the agent SHALL split into multiple commits using suffixes like `(part 1 of 2)`.
3. The commit message SHALL follow the project's conventional-commit pattern (e.g. `feat(scope): ...`, `test(scope): ...`, `docs(scope): ...`) — the scope is typically the change name's key word.
4. The rationale: per-group commits protect against agent crashes, conflict mediation, and `/clear` resets losing unbounded work; they also map cleanly to the post-commit hook's `agent.artifact{status:committed}` event sequence the supervisor uses for verification.

#### Scenario: Coordination skill names the per-group cadence

- **WHEN** the embedded `coordination.md` skill is inspected
- **THEN** the content SHALL contain a heading naming the commit-cadence concept (e.g. "Commit cadence", "Per-group commit cadence", or substantively equivalent)
- **AND** the section's body SHALL mention the GROUP grain (i.e. the substring "group" or "section" appears at least once)
- **AND** SHALL name the ~10-file soft cap on uncommitted work

#### Scenario: Coordination skill names conventional-commit types

- **WHEN** the commit-cadence section is inspected
- **THEN** it SHALL show at least one example commit message using a conventional-commit prefix (`feat(...)`, `fix(...)`, `docs(...)`, `test(...)`, or `chore(...)`)

### Requirement: Coordination skill SHALL forbid the coding agent from invoking `/opsx:verify` and `/opsx:archive`

The embedded `coordination.md` skill SHALL contain a section explaining that the coding agent's terminal action is `agent.artifact { status: "done" }` (or the implicit `committed` event auto-published by the post-commit hook). The section SHALL explicitly state that the coding agent SHALL NOT invoke `/opsx:verify <change-id>` or `/opsx:archive <change-id>`, naming both skill names so the rule is unambiguous.

The rationale SHALL be stated:

1. Verification is the supervisor's responsibility (the five-gate framework codified in `supervisor-as-pane-followups`).
2. Archiving happens during the supervisor's cherry-pick + merge flow on the release branch (per the AGENTS.md release procedure), NOT on the agent's feature branch.

#### Scenario: Coordination skill explicitly names `/opsx:verify` and `/opsx:archive` as off-limits

- **WHEN** the embedded `coordination.md` skill is inspected
- **THEN** the content SHALL contain the literal substring `/opsx:verify`
- **AND** the literal substring `/opsx:archive`
- **AND** prose stating both are NOT the coding agent's responsibility (e.g. "do not invoke", "off-limits", "supervisor's job", or substantively equivalent)

#### Scenario: Coordination skill names the terminal action

- **WHEN** the terminal-action section is inspected
- **THEN** the content SHALL mention `agent.artifact` with `status: "done"` OR `status: "committed"` as the coding agent's final wire-format publish

### Requirement: Supervisor skill SHALL teach `pane_current_path` for pane→agent resolution

The embedded `assets/agent-skills/supervisor.md` skill SHALL contain a section (heading text approximately "Resolve pane to agent" or "Pane→agent mapping") that teaches the supervisor agent the canonical resolution command:

```bash
tmux display-message -t paw-{{PROJECT_NAME}}:0.<pane> -p '#{pane_current_path}'
```

The section SHALL warn explicitly that:

1. Pane indices are NOT sorted alphabetically by `agent_id`.
2. Pane indices are NOT in the CLI-argument order from `git paw start --specs A B C`.
3. The mapping SHALL NOT be inferred from `git paw status` output (which is sorted alphabetically by the broker) or from the dashboard's row order (same).

The output's basename ends in `<project>-feat-<branch>`, giving the authoritative `agent_id`. The supervisor agent SHOULD cache the mapping once per session.

#### Scenario: Supervisor skill names the `pane_current_path` resolution command

- **WHEN** the embedded supervisor skill is inspected
- **THEN** the content SHALL contain the literal substring `tmux display-message` and `pane_current_path`
- **AND** the section SHALL warn against assuming pane order from `agent_id` alphabetical or from CLI-argument order

#### Scenario: Supervisor skill warns against `git paw status` ordering as a mapping source

- **WHEN** the pane-resolution section is inspected
- **THEN** the section SHALL contain prose explicitly noting that `git paw status` ordering or dashboard row order SHALL NOT be used as the pane→agent mapping source
