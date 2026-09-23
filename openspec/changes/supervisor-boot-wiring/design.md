## Context

Three already-specified behaviours are implemented as library pieces but never called from
the production launch path (see proposal). v0.18.0 removes the supervisor pane; these are
v0.17.0 bug fixes and must be correct now, so each fix is the smallest wiring that makes
the existing requirement true, plus a test that exercises the real path.

## Goals / Non-Goals

**Goals:**
- A configured `[governance]` table reaches the launched supervisor's boot context.
- The exported supervisor skill teaches a pane→agent rule that holds for both worktree
  placements and stays project-agnostic.
- Intra-spec forward conflicts are labelled intra-spec in a real session.

**Non-Goals:**
- Refreshing the learnings aggregator's roster after `git paw add` mid-session. The
  dashboard seeds it once at start (as it already does for `register_agent`); a mid-session
  agent's conflicts still classify (cross-spec). Same staleness class as GP-45, handled
  there for the approval path only.
- Changing `coordination::inventory::match_pane` (it already accepts both placements).
- Any change to the governance section's text, the header asset, or the `[governance]`
  schema.

## Decisions

### D1 — One assembly helper for the supervisor boot context (DS-1)

Add `skills::supervisor_boot_context(supervisor_md: &str, unattended: bool, governance:
&GovernanceConfig) -> String`: `with_drive_loop_directive(supervisor_md, unattended)`, then —
when `governance_section_paths(...)` is non-empty — a newline-terminated body, one blank
line, and the section (exactly the separator `tests/governance_supervisor_boot.rs` already
pins). `cmd_supervisor` passes its result as the supervisor assignment's `skill_content`.

The section goes into the supervisor's managed `AGENTS.md` block, not the pane-injected
`{boot_block}\n\n{framing}` text: the requirement positions it *after the supervisor skill
content*, and the skill content lives in that block. The drive-loop directive already uses
the same vehicle ("boot context").

*Alternative rejected:* inlining the concatenation in `cmd_supervisor`. The integration
test would then still have to re-implement it — the exact defect being fixed.

*Empty path:* the configuration chapter documents `constitution = ""` as the way to disable
Spec Kit auto-wiring. The helper treats an empty path as unset so that opt-out does not
render a blank `- constitution: ` bullet.

### D2 — Skill text resolves through the session file, not a basename (DS-2)

Rewrite the paragraph after the `tmux display-message` block: match the output against the
`worktree_path` entries of the `agents` roster in `.git-paw/sessions/<session>.json`; that
entry's `branch_id` is the agent id. The roster's `pane_index` is a launch-time value and is
not used for routing — the live `pane_current_path` match is. State that the worktree
directory name depends on placement, so no basename rule applies; re-resolve after `git paw
add` / `remove`. Remove the `myproj-feat-auth` example. This is the document shape
`bundled-helper-discovery` makes the helpers read, so the skill and `sweep.sh` agree.

### D3 — Persist `spec_id` on the worktree entry; seed the aggregator from the session (DS-3)

`WorktreeEntry.spec_id: Option<String>` with `#[serde(default, skip_serializing_if =
"Option::is_none")]`, mirroring `cli_session_id`. Set it in `attach_agent` from
`spec_entry.map(|s| s.id.clone())` (covers the supervisor launch and `git paw add
--from-spec`) and in `launch_spec_session` from `spec_by_branch`. Bare launches set `None`.

Add `LearningsAggregator::seed_from_session(&mut self, session: &Session)`: for every
worktree entry, `register_agent(slugify_branch(&wt.branch))` and, when `spec_id` is `Some`,
`set_spec_id`. The dashboard calls it in place of its `register_agent` loop. Keying by
`slugify_branch` matches how the dashboard already derives `WatchTarget.agent_id`.

*Alternative rejected:* re-scanning specs at dashboard start to rebuild the mapping. Specs
can be archived or renamed mid-session; the launch-time id is the fact the requirement
asks for, and the session file is where launch facts already live.

## Risks / Trade-offs

- [23 `WorktreeEntry { .. }` literals across `src/` and `tests/` must gain the field] →
  mechanical; the compiler finds every one. `spec_id: None` everywhere except the three
  launch sites.
- [Governance text now enters a real session's context] → it is the specified behaviour;
  with no paths configured the boot context stays byte-identical (pinned by a test).
- [Skill wording change ripples into skill-content tests] → grep `src/skills.rs` tests and
  `tests/*_skill_content.rs` for `pane_current_path` / `feat-` assertions before editing.

## Migration Plan

None. Old session files load with `spec_id = None` (cross-spec fallback, today's
behaviour). Rollback is a plain revert.
