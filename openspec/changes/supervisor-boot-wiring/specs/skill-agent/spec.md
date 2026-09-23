## MODIFIED Requirements

### Requirement: Supervisor skill SHALL teach `pane_current_path` for pane→agent resolution

The embedded `assets/agent-skills/supervisor.md` skill SHALL contain a section (heading text approximately "Resolve pane to agent" or "Pane→agent mapping") that teaches the supervisor agent the canonical resolution command:

```bash
tmux display-message -t paw-{{PROJECT_NAME}}:0.<pane> -p '#{pane_current_path}'
```

The section SHALL warn explicitly that:

1. Pane indices are NOT sorted alphabetically by `agent_id`.
2. Pane indices are NOT in the CLI-argument order from `git paw start --specs A B C`.
3. The mapping SHALL NOT be inferred from `git paw status` output (which is sorted alphabetically by the broker) or from the dashboard's row order (same).

The section SHALL teach that the output is resolved to an agent by matching it against the worktree paths the session records: the `worktree_path` entries of the `agents` roster in the per-repo session file (`.git-paw/sessions/<session>.json`), where the matching entry's `branch_id` is the authoritative `agent_id`. The section SHALL NOT teach deriving the `agent_id` from the directory's basename or from any project-prefixed naming shape, because the worktree directory name depends on the configured worktree placement (a child worktree carries no project prefix, and its slug need not equal the `agent_id`). The skill text SHALL stay project-agnostic (no project-specific directory names). The supervisor agent SHOULD cache the mapping once per session and SHALL re-resolve it after an agent is added or removed, since tmux renumbers panes on layout changes.

#### Scenario: Supervisor skill names the `pane_current_path` resolution command

- **WHEN** the embedded supervisor skill is inspected
- **THEN** the content SHALL contain the literal substring `tmux display-message` and `pane_current_path`
- **AND** the section SHALL warn against assuming pane order from `agent_id` alphabetical or from CLI-argument order

#### Scenario: Supervisor skill warns against `git paw status` ordering as a mapping source

- **WHEN** the pane-resolution section is inspected
- **THEN** the section SHALL contain prose explicitly noting that `git paw status` ordering or dashboard row order SHALL NOT be used as the pane→agent mapping source

#### Scenario: Supervisor skill resolves panes against the session's recorded worktree paths

- **WHEN** the pane-resolution section is inspected
- **THEN** it SHALL name the per-repo session file `.git-paw/sessions/` and its `agents` roster's `worktree_path` and `branch_id` fields as the mapping source
- **AND** it SHALL NOT contain a basename-shape rule such as `<project>-feat-<branch>` or an example basename carrying a project prefix

#### Scenario: Supervisor skill re-resolves after the roster changes

- **WHEN** the pane-resolution section is inspected
- **THEN** it SHALL state that the cached mapping is re-resolved after `git paw add` or `git paw remove`
