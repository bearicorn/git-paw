## Why

Three supervisor-session behaviours are specified, tested in isolation, and never wired
into the real launch path, so a shipped session silently lacks them. Each was verified
against the code:

**DS-1 — governance docs never reach the supervisor.** `skills::governance_section_paths`
(`src/skills.rs`) is called only from unit tests and from
`tests/governance_supervisor_boot.rs`, which assembles the boot prompt *by hand*.
`cmd_supervisor` (`src/commands/supervisor.rs`) builds the supervisor's skill content as
`with_drive_loop_directive(&supervisor_md, unattended)` and never appends the section, so a
configured `[governance]` table has no effect. *Boot prompt includes governance documents
section* and *Governance section follows the supervisor skill content* (`boot-block`) are
unmet; the test suite passes because it never exercises the production assembly.

**DS-2 — the exported supervisor skill teaches a basename rule that is false by default.**
`assets/agent-skills/supervisor.md` (section *Resolve pane to agent via
pane_current_path*) says a worktree's basename "ends in `<project>-feat-<branch>`". That
is the sibling layout only (`git::worktree_dir_name`). The child layout —
`.git-paw/worktrees/<branch-slug>`, no project prefix (`git::branch_slug`) — is what
`git paw init` backfills (`worktree_placement = "child"`), and its slug can differ from the
broker `agent_id` for branches with stripped characters. A supervisor following the skill
mis-attributes or fails to attribute panes. The skill should match the pane's path against
the worktree paths the session records, which is what *Inventory sourcing*
(`supervisor-skill-discipline`) already requires.

**DS-3 — intra-spec conflict labelling can never happen.** `LearningsAggregator::set_spec_id`
(`src/broker/learnings.rs`) is called only from tests. The dashboard (`src/main.rs`) calls
only `register_agent`, so `classify_forward` always falls through to
`forward-conflict-cross-spec` with empty spec ids. *Conflict-event signal*
(`supervisor-learnings`) is unmet for the intra-spec case. The spec id is known at
spec-driven launch but is not persisted anywhere the dashboard can read it.

## What Changes

- `cmd_supervisor` appends the governance section (from the resolved `[governance]` paths)
  after the supervisor skill content in the supervisor's boot context, through one helper
  that the integration test also calls — no more hand-assembled test prompt.
- The supervisor skill's pane-resolution section tells the supervisor to match
  `pane_current_path` against the `worktree_path` entries of the `agents` roster in the
  per-repo session file (`.git-paw/sessions/<session>.json`) and take that entry's
  `branch_id`, and never to infer the agent from a basename shape. Project-agnostic text.
- `WorktreeEntry` gains `spec_id: Option<String>` (`#[serde(default,
  skip_serializing_if = "Option::is_none")]`), set on every spec-driven launch path; the
  dashboard seeds the learnings aggregator's `agent_id → spec_id` map from it.
- No CLI, config, or wire-format change. Old session files load unchanged.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `boot-block`: *Governance section follows the supervisor skill content* gains a scenario
  pinning the real supervisor launch path (the defect was that only a hand-assembled prompt
  was tested).
- `skill-agent`: *Supervisor skill SHALL teach `pane_current_path` for pane→agent
  resolution* replaces the basename rule with matching against the session's recorded
  worktree paths.
- `session-state`: ADDED *Worktree entry records the spec it was launched from*.
- `supervisor-learnings`: *Conflict-event signal* states where the agent → spec mapping
  comes from (the persisted session) and what an agent without a recorded spec classifies
  as.

## Impact

- `src/commands/supervisor.rs` — `cmd_supervisor`: supervisor `skill_content` assembly.
- `src/skills.rs` — new `pub fn supervisor_boot_context(supervisor_md, unattended,
  &GovernanceConfig) -> String` wrapping `with_drive_loop_directive` +
  `governance_section_paths`.
- `tests/governance_supervisor_boot.rs` — call the production helper instead of
  hand-assembling; new e2e in `tests/e2e_supervisor_launch.rs` reading the repo-root
  `AGENTS.md` after a real launch.
- `assets/agent-skills/supervisor.md` — *Resolve pane to agent via pane_current_path*.
  `tests/supervisor_routing_skill_content.rs` gains assertions.
- `src/session.rs` — `WorktreeEntry.spec_id`; every `WorktreeEntry { .. }` literal (23 sites
  across `src/` and `tests/`) gains `spec_id: None` or the real value.
- `src/main.rs` — `attach_agent` (shared by the supervisor launch and `git paw add
  --from-spec`) sets `spec_id` from `spec_entry`; the dashboard's learnings wiring seeds
  `set_spec_id`. `src/commands/start.rs` — `launch_spec_session` sets `spec_id`; bare
  `cmd_start` sets `None`.
- `src/broker/learnings.rs` — new `LearningsAggregator::seed_from_session(&Session)`
  (register + spec id) so the dashboard wiring is testable.
- **Delta coordination** (grepped every pending change): `start-spec-launch-parity` and
  `shipped-text-corrections` touch other `boot-block` requirements;
  `bundled-helper-discovery` modifies *Find session by repository path* in `session-state`
  (a different requirement — this change ADDs one); no pending change touches
  `skill-agent` or `supervisor-learnings`. No collision.
- **Consistency with `bundled-helper-discovery`**: the skill text names the per-repo
  discovery file and its `agents` roster (`branch_id` / `worktree_path`), the shape that
  change makes the helpers read; it does not mention the global session-state shape.
- **Sequencing**: `start-spec-launch-parity` also edits `launch_spec_session`, and
  `cli-launch-identity` edits `attach_agent`; the `spec_id` line is a one-field addition to
  each `WorktreeEntry` literal and merges cleanly either way. `cli-launch-identity` ADDs a
  different `boot-block` requirement and `retire-stale-requirements` modifies other
  `supervisor-learnings` requirements — no collision.
