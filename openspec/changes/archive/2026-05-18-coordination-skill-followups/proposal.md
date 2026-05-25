## Why

Six small skill-content gaps surfaced during v0.5.0 dogfooding (MILESTONE drift items 34, 37, 54, 55, 56, 57) that are each too thin to justify their own OpenSpec change but, taken together, materially improve the agent-skills surface. None of them belong in an existing in-flight change:

- The `forward-coordination` change is already mid-implementation (its agent has published `agent.verified`); re-opening it to thread in extra subsections would invalidate the verification and force re-audit. The cheapest path is a standalone follow-up change that lands **after** `forward-coordination` archives.
- Drift 34 and 37 were initially folded mid-flight into `forward-coordination`'s tasks.md as augmented sections 9 + 10. The agent had already verified before seeing the augmentation, so the augmented sections sat unchecked. Moving them here keeps `forward-coordination` archivable as the agent actually completed it; the drift-34 + drift-37 work lands as part of this follow-up.
- The six items are all asset-and-spec-only edits to the same two markdown files (`assets/agent-skills/coordination.md` and `assets/agent-skills/supervisor.md`) plus their content-substring tests in `src/skills.rs::tests`. Bundling them avoids creating six micro-changes whose deltas would conflict with each other at merge time.

The drift items:

- **Drift 34 — Agents don't poll the broker inbox for `agent.feedback` answers.** During v0.5.0 dogfood, 5 agents published `agent.question` events; the supervisor published matching `agent.feedback` events back; the agents never received them because no agent CLI polls its inbox between events. Workaround: the supervisor SHOULD ALSO push the answer text via `tmux send-keys` to the agent's pane, so the answer arrives as direct input. Agent-side polling is deferred to v0.6.0 with MCP integration; this is the v0.5.0 mitigation.
- **Drift 37 — No agent working-heartbeat between lifecycle events.** Agents only publish on REGISTER / DONE / BLOCKED / QUESTION; `last_seen` stays stale unless the filesystem watcher catches file writes. Dashboard shows agents as "stuck" while they're actively working on read-only tasks (reads, grep, LLM-only deliberation, awaiting permission prompts). Fix: agents publish a lightweight `agent.status { status: "working" }` heartbeat every 5 tool uses. Reuses the existing `agent.status` shape; no new wire format.
- **Drift 54 — `agent_id` vs branch slugify mismatch.** Supervisors (human and LLM) repeatedly confused the dashed `agent_id` form (`feat-no-supervisor-flag`) with the slashed branch form (`feat/no-supervisor-flag`) when composing `agent.feedback` and `agent.question` payloads. `slugify_branch` is the canonical conversion. The skills do not document the two forms or where each is used.
- **Drift 55 — Supervisor's own commits invisible to agents.** When the supervisor (human or LLM) commits bug fixes or prep work to `main` while agents are running, no broker event surfaces the change. Agents working in feat branches don't know `main` has advanced and may produce incompatible commits. `forward-coordination` introduces `agent.intent`; the supervisor skill should now teach the supervisor to publish `agent.intent` from `agent_id = "supervisor"` whenever it touches repo files on `main`.
- **Drift 56 — `⏵⏵ accept edits` mode reduces audit visibility.** Once an agent enters Claude Code's accept-edits auto-mode, subsequent file edits silently apply without re-prompting. The supervisor loses real-time visibility into what's being edited. The supervisor skill needs an explicit "inspect each `agent.artifact { modified_files }` against the change's owned/expected files" step in merge orchestration, with the option to flag out-of-scope edits via `agent.feedback`.
- **Drift 57 — Stash-pop disaster pattern undocumented.** During a v0.5.0 dogfood session, `feat-supervisor-as-pane` lost in-flight work via a `git stash pop` of an unrelated stash (from another agent's worktree). The coordination skill has no guidance for stash hygiene in a multi-worktree environment.

## What Changes

Six new subsections across the two skill markdown files:

1. **`coordination.md` — `### Working heartbeat`** (new). The "every 5 tool uses" cadence plus the rationale (filesystem watcher misses read-only tools, permission waits, LLM-only deliberation). Covers drift 37.
2. **`coordination.md` — `### References & terminology`** (new). Documents the two forms of agent identifier (slashed branch name vs dashed `agent_id`) and names `slugify_branch` as the canonical conversion. Covers drift 54.
3. **`coordination.md` — `### Stash hygiene`** (new). The three rules: list before pop; inspect via `git stash show -p stash@{N}`; pop only entries you authored. Includes the dogfood narrative as the cautionary example. Covers drift 57.
4. **`supervisor.md` — `### Send the answer to the agent pane too`** (new). Supervisor SHALL ALSO push `agent.feedback` answer text via `tmux send-keys` to the agent's pane (because agents don't poll their inbox). Cross-references the existing paste-buffer recovery sub-case for long answers. Covers drift 34.
5. **`supervisor.md` — `### Supervisor publishes agent.intent for main-side work`** (new). When the supervisor commits to `main` (bug fixes, prep, etc.), publish `agent.intent { agent_id: "supervisor", payload: { files: [...], summary: ..., valid_for_seconds: ..., scope: "main" } }` so agents working in feat branches see the main-side advance. Cross-references `forward-coordination`'s `agent.intent` mechanism. Covers drift 55.
6. **`supervisor.md` — `### Verify accept-edits commits before merge`** (new, slotted into the merge-orchestration / Spec Audit area). For any agent that ran in `⏵⏵ accept edits` mode, inspect each `agent.artifact { modified_files }` against the change's owned-files / expected-files list; flag out-of-scope edits via `agent.feedback`. Covers drift 56.

Plus skill-content tests in `src/skills.rs::tests` asserting each new subsection's key substrings.

**Not in scope:**
- No new wire-format variants. `agent.intent` is owned by `forward-coordination`; this change only teaches the supervisor a new caller pattern.
- No new CLI surface, no new config keys, no new endpoints.
- No production-code edits outside the skill-content test module — these are spec-and-asset additions.
- No changes to existing `coordination.md` / `supervisor.md` sections; only new subsections appended at well-defined headings so merge with `forward-coordination` is additive.

## Capabilities

### New Capabilities
*(none — this change extends an existing capability)*

### Modified Capabilities
- `agent-skills` — extend "Embedded coordination skill" with new requirements for the references-terminology and stash-hygiene subsections; extend "Embedded supervisor skill" with new requirements for the supervisor-publishes-intent and accept-edits-audit subsections. Skill-content scenarios assert the new substrings.

## Impact

**Code**:
- `assets/agent-skills/coordination.md` — append two new subsections (`### References & terminology`, `### Stash hygiene`).
- `assets/agent-skills/supervisor.md` — append two new subsections (`### Supervisor publishes agent.intent for main-side work`, `### Verify accept-edits commits before merge`).
- `src/skills.rs::tests` — add 4-6 content-substring assertions.

**Tests**: 4-6 unit tests in `src/skills.rs::tests` covering each new subsection's expected substrings.

**Docs**: mdBook chapter `docs/src/user-guide/coordination.md` mirrors the skill content (already touched by `forward-coordination`; this change appends matching sections).

**Backward compatibility**: fully additive. Existing scenarios in `agent-skills/spec.md` remain valid; user overrides at `<config_dir>/git-paw/agent-skills/{coordination,supervisor}.md` continue to win via the resolution order.

**Dependencies**: none added.

**Order relative to `forward-coordination`**: this change applies **after** `forward-coordination` archives. The two changes touch the same files but at non-overlapping headings (see `design.md` D2 for the exact layout). If `forward-coordination` is still mid-implementation when this change is implemented, the implementing agent SHALL rebase on `main` after `forward-coordination` archives, not parallelise.
