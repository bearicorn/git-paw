# Tasks

## 0. Drift 34 — Send the answer to the agent pane too

- [x] 0.1 Edit `assets/agent-skills/supervisor.md`: in the `### Publish feedback to a peer agent` section (or a new subsection beneath it), add an explicit instruction that when the supervisor publishes `agent.feedback` in response to an `agent.question`, it MUST ALSO send the answer text to the asking agent's pane via `tmux send-keys -t paw-{{PROJECT_NAME}}:0.<pane-index> "<answer>" Enter`. Include the rationale: "agents do not poll their inbox for `agent.feedback` responses; this workaround is transitional until MCP-mediated inbox access lands in v0.6.0."
- [x] 0.2 In the same supervisor skill section, cross-reference the existing paste-buffer recovery sub-case (under stall detection): if the answer text is long enough to trigger a paste-buffer indicator (e.g. `Pasted text #N` on Claude Code), the supervisor SHALL follow up with `tmux send-keys ... Enter` to submit the buffered content.
- [x] 0.3 Update `docs/src/user-guide/coordination.md` (or the supervisor-side user-guide chapter) to mirror the new instruction so the in-tree docs match the skill content.

## 0.5. Drift 37 — Working heartbeat in coordination skill

- [x] 0.5.1 Edit `assets/agent-skills/coordination.md`: insert a new `### Working heartbeat` subsection (placement: after the `### Automatic status publishing` section and before the new `### References & terminology` section from drift 54). The section MUST state the cadence as "every 5 tool uses" and explain why this is needed despite the filesystem watcher (the watcher cannot observe read-only tools, permission-prompt waits, or LLM-only deliberation).
- [x] 0.5.2 Include a `curl` example in the new section that publishes `agent.status` with `status: "working"` and `modified_files: []` (or current dirty list) using the existing `{{GIT_PAW_BROKER_URL}}` and `{{BRANCH_ID}}` placeholders. No new wire-format variant is introduced — the heartbeat reuses the existing `agent.status` shape.
- [x] 0.5.3 Update `docs/src/user-guide/coordination.md` to mirror the new heartbeat section.

## 1. Edit `assets/agent-skills/coordination.md`

- [x] 1.1 Append a new `### References & terminology` subsection after `### Messages you may receive`. Content per `design.md` D3: document the branch-name vs `agent_id` forms, name `slugify_branch` as the conversion, describe the slugify rule's effect (lowercase, non-`[a-z0-9_]` → `-`, collapse, fallback `agent`), and state which form to use in which context (`agent_id` in broker payload `target` fields; branch name in git operations).
- [x] 1.2 Append a new `### Stash hygiene` subsection after `### References & terminology`. Content per `design.md` D6: the three rules in order — list before pop (`git stash list`); inspect before pop (`git stash show -p stash@{N}`); pop only your own. State explicitly that `git stash pop` SHOULD NOT be run blindly. The cautionary v0.5.0 dogfood narrative is optional.
- [x] 1.3 Do NOT modify any existing subsection of `coordination.md`. All edits are additive new headings.
- [x] 1.4 Re-read the file end-to-end to confirm the two new subsections read coherently after the `forward-coordination` baseline content (which precedes them) and that no `forward-coordination` heading was accidentally duplicated.

## 2. Edit `assets/agent-skills/supervisor.md`

- [x] 2.1 Append a new `### Supervisor publishes agent.intent for main-side work` subsection after the existing `### Conflict detection` section and before `### Rules`. Content per `design.md` D4: explain the visibility gap (supervisor commits to `main` aren't surfaced as broker events; agents in feat branches may produce incompatible commits without notification), include a `curl` example with `type: "agent.intent"`, `agent_id: "supervisor"`, payload containing `files`, `summary`, `valid_for_seconds` (and optional illustrative `scope: "main"`), and cross-reference the agent-side `Before you start editing` section in `coordination.md`.
- [x] 2.2 Append a new `### Verify accept-edits commits before merge` subsection as a sibling immediately after the `### Spec Audit Procedure` section. Content per `design.md` D5: explain the auto-accept-edits visibility gap; instruct the supervisor to locate the change's expected file list in `proposal.md`; instruct the supervisor to diff `agent.artifact { modified_files }` against that list; flag out-of-scope edits via `agent.feedback`; state that out-of-scope edits SHALL NOT be silently auto-approved.
- [x] 2.3 Do NOT modify any existing subsection of `supervisor.md`. All edits are additive new headings.
- [x] 2.4 Re-read the file end-to-end to confirm the two new subsections sit cleanly after the `forward-coordination` baseline content and that the cross-reference to `coordination.md`'s `Before you start editing` heading resolves correctly to the post-`forward-coordination` heading text.

## 3. Skill-content tests in `src/skills.rs::tests`

All tests SHALL be behavioural substring assertions on the embedded skill content (consistent with the existing `coordination_skill_*` / `supervisor_skill_*` tests in the same module). No production code changes outside this test module.

- [x] 3.1 Add `coordination_skill_documents_slugify_terminology` — assert the embedded `coordination.md` content contains both `agent_id` and `slugify_branch` as substrings, AND that a heading naming references/terminology is present.
- [x] 3.2 Add `coordination_skill_documents_stash_hygiene` — assert the embedded `coordination.md` content contains `git stash list`, `git stash show -p`, and a heading naming stash hygiene. Optionally assert language indicating "pop only your own" or substantively equivalent.
- [x] 3.3 Add `supervisor_skill_documents_main_side_intent` — assert the embedded `supervisor.md` content contains `agent.intent`, the literal `"supervisor"` `agent_id` value (or substantively equivalent within a curl example), AND a heading indicating supervisor-side intent publishing.
- [x] 3.3a Add `supervisor_skill_documents_tmux_send_keys_alongside_feedback` (drift 34) — assert the embedded `supervisor.md` content contains the substrings `tmux send-keys` AND `agent.feedback` within the same section, AND mentions the rationale "agents do not poll".
- [x] 3.3b Add `coordination_skill_documents_working_heartbeat` (drift 37) — assert the embedded `coordination.md` content contains a heading naming working heartbeat, the literal `every 5 tool uses` (or equivalent naming `5`), the substring `agent.status` (the heartbeat reuses this type), and rationale text mentioning the filesystem watcher.
- [x] 3.4 Add `supervisor_skill_documents_accept_edits_audit` — assert the embedded `supervisor.md` content contains `accept edits` and `modified_files` substrings AND a heading naming the audit step. Optionally assert language indicating out-of-scope edits MUST NOT be silently auto-approved.
- [x] 3.5 (Optional) Add `coordination_skill_describes_slugify_rule` — assert the slugify rule description includes the keyword sequence covering lowercase, non-allowed-char replacement, and the `agent` fallback, so future rewordings that drop the rule are caught.
- [x] 3.6 (Optional) Add `supervisor_skill_cross_references_agent_intent_flow` — assert the supervisor-publishes-intent section names the `Before you start editing` heading (or substantively equivalent cross-reference) so the cross-reference doesn't bit-rot silently.

## 4. Quality gates

- [ ] 4.1 Run `just lint` (fmt + clippy). Skill-content edits don't touch Rust code, but the new test additions must pass clippy pedantic.
- [ ] 4.2 Run `just check` (fmt + clippy + all tests). All new content tests SHALL pass.
- [ ] 4.3 Run `just deny`. No license/advisory/duplicate-dep regressions expected (no dependency changes).
- [ ] 4.4 Verify no `unwrap()` / `expect()` introduced in non-test code (this change adds no non-test code; this is a sanity check).

## 5. Documentation

- [x] 5.1 Update mdBook chapter `docs/src/user-guide/coordination.md` to mirror the two new `coordination.md` subsections (References & terminology; Stash hygiene). Match prose closely to keep the user-guide chapter and the embedded skill in sync.
- [x] 5.2 If a `docs/src/user-guide/supervisor.md` (or equivalent) chapter exists at archive time, mirror the two new `supervisor.md` subsections (Supervisor publishes agent.intent for main-side work; Verify accept-edits commits before merge) there. If no such chapter exists, skip this task (the skill content itself is the user-facing surface). _(N/A — no `docs/src/user-guide/supervisor.md` chapter exists; skipped per task condition.)_
- [ ] 5.3 Run `mdbook build docs/` — SHALL succeed with no warnings.

## 6. No production code changes

- [ ] 6.1 Confirm `git diff` against the merge base shows changes only in:
  - `assets/agent-skills/coordination.md`
  - `assets/agent-skills/supervisor.md`
  - `src/skills.rs` (test module only — verify the diff stays inside `#[cfg(test)] mod tests {}`)
  - `docs/src/user-guide/coordination.md` (and optionally `supervisor.md`)
  - `openspec/changes/coordination-skill-followups/**`
- [ ] 6.2 Confirm no edits to `src/broker/`, `src/main.rs`, `src/skills.rs` outside the test module, or any other production source path.

## 7. Order-of-operations note

- [x] 7.1 Before starting implementation, confirm that `forward-coordination` has archived. If it has not, **wait** — do not implement on a base that lacks `forward-coordination`'s rewrites of `coordination.md` and `supervisor.md`, because the new headings this change appends are positioned relative to headings `forward-coordination` introduces. _(Verified: `5fba167 chore(specs): archive forward-coordination; sync deltas to main specs` is in this branch's history.)_
- [x] 7.2 Once `forward-coordination` archives, rebase this change's branch on `main`, re-read both skill files, and confirm the heading anchors named in tasks 1 and 2 are present before appending the new subsections. _(Branch already includes `forward-coordination`'s archive commit; the anchors `### Messages you may receive`, `### Spec Audit Procedure`, `### Conflict detection` / `### Watch peer intents and broker-side conflict detection`, and `### Rules` were all verified present before editing.)_
