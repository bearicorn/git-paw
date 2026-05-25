## ADDED Requirements

### Requirement: Coordination skill — Spec Kit consolidated worktree behaviour

The embedded `coordination.md` skill SHALL include a "When working in a Spec Kit consolidated worktree" sub-section that activates when the agent's worktree branch begins with `phase/`. The sub-section SHALL instruct the agent to:

1. Read the ordered list of tasks provided in the boot prompt and treat them as a sequential to-do list (no parallelism within the consolidated worktree — the non-`[P]` marker in Spec Kit means tasks share files or context).
2. Work through tasks in the order given. After completing each task, flip its `- [ ]` checkbox to `- [x]` in the worktree's local `tasks.md`. The agent MAY commit the writeback alongside the task's code change or as a separate commit; the choice is the agent's.
3. Publish `agent.intent` covering the union of files for the next 1–2 tasks rather than re-publishing for every task — `valid_for_seconds` SHALL be set generously (e.g. equal to expected runtime for the remaining tasks) since the agent owns the consolidated set.
4. Publish `agent.done` (the existing `agent.artifact` with terminal status) only after every task in the listed set shows `- [x]` in `tasks.md`.

The sub-section SHALL state that for `[P]` (single-task) worktrees this guidance does not apply — `[P]` worktrees are scoped to one task and follow the standard "before/while editing" coordination pattern.

#### Scenario: Coordination skill mentions Spec Kit consolidated behaviour

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains a heading or section referring to Spec Kit consolidated worktrees (or `phase/...` branches)
- **AND** it instructs the agent to work through listed tasks sequentially

#### Scenario: Coordination skill mentions tasks.md writeback

- **WHEN** the embedded coordination skill is inspected
- **THEN** it instructs the agent to flip `- [x]` in `tasks.md` per task as it completes
- **AND** it states that the writeback can be committed alongside the task's code or as a separate commit

#### Scenario: Coordination skill states agent.done timing for consolidated worktrees

- **WHEN** the embedded coordination skill is inspected
- **THEN** it instructs the agent to publish `agent.done` only after all listed tasks show `- [x]`

#### Scenario: Coordination skill clarifies that [P] worktrees follow standard pattern

- **WHEN** the embedded coordination skill is inspected
- **THEN** it clarifies that `[P]` (single-task) worktrees do not require sequential-list handling
- **AND** the standard "before/while editing" pattern applies to `[P]` worktrees

