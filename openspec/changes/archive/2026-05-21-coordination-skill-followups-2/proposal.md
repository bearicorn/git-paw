## Why

The 2026-05-18 to 2026-05-20 v0.5.0 Batch-2 dogfood surfaced three skill-content drifts that didn't fit any of the in-flight changes. They are doctrine and instruction updates — no new wire-format variants, no new CLI surface, no new config keys. Folding them into one follow-up change keeps the surface coherent.

### The three drifts

1. **Drift A — Per-section commit cadence is missing from `/opsx:apply` and from the coordination skill.** OpenSpec agents using `/opsx:apply` walk through tasks and mark them `- [x]` in `tasks.md`, but they do NOT commit between tasks or task groups unless explicitly told to. Three of four Batch-2 agents accumulated 14-20 uncommitted files at 9-12 tasks done before committing anything. The bundled coordination skill SHALL teach the per-group commit cadence so agents commit per Section/Group as a default behaviour, not as a supervisor-driven nudge. Captured 2026-05-19 in MEMORY.md `feedback_per_section_commit_cadence.md`.

2. **Drift B — Pane→agent mapping resolution via `pane_current_path` is not documented.** Pane indices are NOT sorted alphabetically by `agent_id` and NOT in the CLI invocation order — the launcher assigns by internal scan order. The supervisor skill currently tells the supervisor agent to write `tmux capture-pane -t paw-{{PROJECT_NAME}}:0.<pane-index> -p` but does not teach how to resolve `<pane-index>` for a given agent. The correct resolution path is `tmux display-message -t <session>:0.<pane> -p '#{pane_current_path}'`, whose output ends in `<project>-feat-<branch>` — the authoritative agent identity. Captured 2026-05-19 in MEMORY.md `feedback_verify_pane_to_agent_mapping.md`.

3. **Drift C — `/opsx:verify` and `/opsx:archive` SHALL be explicitly off-limits for the coding agent.** Both Batch-2 coding agents (pane 3 supervisor-as-pane-followups and pane 2 test-coverage-v0-5-0) attempted to invoke `/opsx:verify <change-id>` and `/opsx:archive <change-id>` themselves. Verification is the supervisor's job (the five-gate framework codified in supervisor-as-pane-followups); archiving is the supervisor's job per the AGENTS.md release procedure. The coordination skill SHALL state explicitly that the coding agent's terminal action is `agent.artifact { status: "done" }` (or `committed` via the post-commit hook); the agent SHALL NOT invoke `/opsx:verify` or `/opsx:archive`.

## What Changes

### 1. `coordination.md` — `### Commit cadence` subsection

Add a new subsection to `assets/agent-skills/coordination.md` (after the existing `### Working heartbeat` from coordination-skill-followups, before `### Stash hygiene`). Content:

- State the per-group commit cadence: after completing a numbered task group (e.g. `## 1.`, `## 2.`, etc.), the agent SHALL commit before starting the next group. Each task group becomes one commit by construction.
- Bound: an agent SHALL NOT accumulate more than ~10 uncommitted files at a time. If a group is large enough to exceed that threshold mid-implementation, split the commit ("group 2 (part 1 of 2)" etc.).
- Conventional-commit type per group: typically `feat(<scope>):`, `fix(<scope>):`, `docs(<scope>):`, `test(<scope>):`, `chore(<scope>):` per the project's existing convention. The scope is the change name's key word (e.g. `coverage`, `dashboard`, `broker`).
- Rationale: protects against agent crashes, conflict mediation, and `/clear` resets losing unbounded work; matches the post-commit hook's `agent.artifact{status:committed}` event cadence the supervisor uses for verification.

### 2. `supervisor.md` — `### Resolve pane to agent via `pane_current_path`` subsection

Add a new subsection to `assets/agent-skills/supervisor.md` (near the existing `### Observe and drive a peer pane via tmux` section). Content:

- Show the canonical resolution command:
  ```bash
  tmux display-message -t paw-{{PROJECT_NAME}}:0.<pane> -p '#{pane_current_path}'
  ```
- Explain that the output ends in `<project>-feat-<branch>`, giving the authoritative `agent_id` (with the slash form `feat/<branch>`).
- Warn explicitly: pane indices are NOT alphabetical by `agent_id`, NOT in CLI-argument order. Do NOT infer the mapping from `git paw status`'s output ordering (sorted alphabetically by the broker) or from the dashboard's row order (same).
- Recommend caching the mapping once per session and re-resolving only when the supervisor notices an inconsistency.

### 3. `coordination.md` — `### Terminal action: commit then publish, never archive` subsection

Add a new subsection to `assets/agent-skills/coordination.md`. Content:

- Coding agent's terminal action SHALL be: (1) commit (auto-published by post-commit hook as `agent.artifact{status:committed}`), and (2) optionally publish `agent.artifact{status:done}` to signal "no more work coming". The agent SHALL NOT invoke `/opsx:verify <change-id>` or `/opsx:archive <change-id>` itself.
- Rationale: verification is the supervisor's responsibility (five-gate framework from drift 66 / supervisor-as-pane-followups). Archive happens during the supervisor's cherry-pick + merge flow on the release branch, NOT on the agent's feature branch.
- Explicitly call out the two skill names: `/opsx:verify` and `/opsx:archive` are off-limits for the coding agent. List them by name to make the rule unambiguous.

## Capabilities

### New Capabilities

*(none — extends existing capabilities)*

### Modified Capabilities

- `agent-skills` — coordination skill gains commit-cadence and terminal-action subsections; supervisor skill gains the pane-to-agent resolution subsection.

## Impact

**Code:**

- `assets/agent-skills/coordination.md` — append two new `###` subsections.
- `assets/agent-skills/supervisor.md` — append one new `###` subsection.
- `src/skills.rs::tests` — skill-content substring assertions for the new sections (3-4 new tests).

**Tests:**

- Skill-content tests asserting each new subsection's key substrings.

**Docs:**

- `docs/src/user-guide/coordination.md` (mdBook mirror) — append matching prose for the two coordination-skill additions.
- `docs/src/user-guide/supervisor.md` (mdBook mirror) — append matching prose for the supervisor-skill addition.

**Backward compatibility:** fully additive. Existing skill content remains; new subsections are appended at well-defined heading anchors.
