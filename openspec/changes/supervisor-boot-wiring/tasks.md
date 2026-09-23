## 1. Failing tests first

- [ ] 1.1 `tests/e2e_supervisor_launch.rs`: launch `git paw start --supervisor` (stub `cli = "echo"`, as the existing test does) with `[governance] dod = "docs/dod.md"`; read the repo-root `AGENTS.md` and assert a start-of-line `\n## Governance documents\n`, the bullet `- dod: docs/dod.md`, and that the heading follows `## Supervisor Skills` — fails before the fix (boot-block: *A launched supervisor session carries the governance section*)
- [ ] 1.2 Same file: no `[governance]` paths → the repo-root `AGENTS.md` has no start-of-line `## Governance documents` heading (*…without governance paths is unchanged*)
- [ ] 1.3 `tests/supervisor_routing_skill_content.rs`: the pane-resolution section names `.git-paw/sessions/`, `agents`, `worktree_path` and `branch_id`; contains no `<project>-feat-` rule and no project-prefixed example basename (e.g. `myproj-`); states re-resolution after `git paw add` / `git paw remove` — fails before the fix (skill-agent scenarios 3 and 4)
- [ ] 1.4 `src/session.rs` tests: `spec_id = Some(..)` round-trips; JSON without `spec_id` loads as `None`; `spec_id = None` serializes with no `spec_id` key (session-state, all three scenarios)
- [ ] 1.5 `src/broker/learnings.rs` tests: `seed_from_session` with X and Y both recording `003-user-list`, then a `[conflict-detector] forward conflict` message → `forward-conflict-intra-spec` with that spec id; X recorded / Y unrecorded → cross-spec, rendered bullet names `003-user-list` and no empty spec (supervisor-learnings scenarios 4 and 5) — fails before `seed_from_session` exists

## 2. Governance in the supervisor boot context (DS-1)

- [ ] 2.1 `src/skills.rs`: add `pub fn supervisor_boot_context(supervisor_md: &str, unattended: bool, governance: &GovernanceConfig) -> String` — `with_drive_loop_directive`, then (when non-empty) newline-terminate, one blank line, `governance_section_paths(adr, test_strategy, security, dod, constitution)`; treat an empty path as unset (design D1); doc comment
- [ ] 2.2 `src/commands/supervisor.rs` `cmd_supervisor`: set the supervisor assignment's `skill_content` to `Some(supervisor_boot_context(&supervisor_md, unattended, &config.governance))`
- [ ] 2.3 `tests/governance_supervisor_boot.rs`: replace the hand-written `assemble_supervisor_boot_prompt` concatenation with a call to `supervisor_boot_context` (build a `GovernanceConfig`); keep the four existing assertions; update the module doc comment
- [ ] 2.4 Unit test in `src/skills.rs`: `unattended = true` with a governance path → drive-loop directive present AND the governance heading follows it; empty `constitution` path renders no `- constitution:` bullet

## 3. Supervisor skill pane resolution (DS-2)

- [ ] 3.1 `assets/agent-skills/supervisor.md` *Resolve pane to agent via pane_current_path*: replace the basename paragraph and `myproj-feat-auth` example with matching against the `agents` roster's `worktree_path` in `.git-paw/sessions/<session>.json` (that entry's `branch_id` is the agent id; the roster's `pane_index` is launch-time and not used for routing); say the directory name depends on worktree placement; re-resolve after `git paw add` / `remove`. Keep the existing warnings and the `tmux display-message` block verbatim; no project-specific names
- [ ] 3.2 Run the existing skill-content tests (`src/skills.rs` `supervisor_skill_documents_pane_current_path_resolution`, the `git paw status` warning test, `tests/lang_agnostic_skill_audit.rs`, `tests/agent_skills_conform.rs`) and fix any assertion pinned to the removed text

## 4. Persist and consume the spec id (DS-3)

- [ ] 4.1 `src/session.rs`: add `spec_id: Option<String>` to `WorktreeEntry` with `#[serde(default, skip_serializing_if = "Option::is_none")]` and a doc comment
- [ ] 4.2 Add `spec_id` to every `WorktreeEntry { .. }` literal (`rtk grep "WorktreeEntry {" src tests`): `None` except the launch sites below
- [ ] 4.3 `src/main.rs` `attach_agent`: `spec_id: spec_entry.map(|s| s.id.clone())`; `src/commands/start.rs` `launch_spec_session`: `spec_id` from `spec_by_branch.get(branch)`; bare `cmd_start`: `None`
- [ ] 4.4 Test: a spec-driven launch's saved session carries each agent's `spec_id` (extend the closest existing spec-launch integration test, e.g. `tests/from_specs_launch_fixes_integration.rs`, reading the saved session JSON) (session-state scenario 1)
- [ ] 4.5 `src/broker/learnings.rs`: add `pub fn seed_from_session(&mut self, session: &Session)` — `register_agent(slugify_branch(&wt.branch))` per entry plus `set_spec_id` when `spec_id` is `Some`
- [ ] 4.6 `src/main.rs` dashboard learnings wiring: replace the `register_agent` loop over `watch_targets` with `seed_from_session` on the loaded session

## 5. Docs

- [ ] 5.1 `docs/src/user-guide/supervisor.md` *Resolve Pane to Agent*: replace "re-resolution only happens when the supervisor notices an inconsistency" with re-resolution after `git paw add` / `remove` too; state that the session file's `pane_index` is not used for routing
- [ ] 5.2 `docs/src/user-guide/governance.md`: one sentence saying the configured docs appear as a `## Governance documents` section after the supervisor skill in the supervisor's `AGENTS.md` block, and that `constitution = ""` adds no bullet
- [ ] 5.3 `docs/src/user-guide/learnings.md` *Conflict events*: forward conflicts are labelled intra-spec / cross-spec from each agent's launch spec; agents started without a spec classify cross-spec
- [ ] 5.4 Verify every scenario in this change's four delta specs maps to a test from sections 1–4

## 6. Gates

- [ ] 6.1 `cargo fmt`, then `just lint`
- [ ] 6.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast > target/test.log 2>&1; echo $?` — exit code must be 0 (never trust a piped status)
- [ ] 6.3 `mdbook build docs/`
- [ ] 6.4 `openspec validate supervisor-boot-wiring --strict`
