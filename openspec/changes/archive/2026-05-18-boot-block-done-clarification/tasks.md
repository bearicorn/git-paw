## 1. Rewrite the boot-block DONE section

- [ ] 1.1 In `assets/boot-block-template.md`, locate section `### 2. DONE: Task completion reporting`.
- [ ] 1.2 Replace the section body so the first paragraph instructs the agent to commit its work via `git commit` and explains that the git-paw post-commit hook auto-publishes `agent.artifact { status: "committed" }` with the committed files attached. State that the agent SHALL NOT publish anything manually for tasks that produce code changes.
- [ ] 1.3 Add an emphasised (bold or uppercase) warning immediately after the commit-first paragraph: the agent SHALL NOT publish manual `done` while the worktree has uncommitted changes; it SHALL commit instead.
- [ ] 1.4 Add a fallback paragraph describing manual `agent.artifact { status: "done" }` as the path for code-less tasks. Enumerate representative code-less task types (docs-only updates handled outside this worktree, planning notes, exploration tasks where the artifact is information reported to the broker).
- [ ] 1.5 Retain the existing manual `agent.artifact { status: "done" }` curl verbatim (same JSON shape: type, agent_id, payload with status/exports/modified_files) under the fallback paragraph so code-less agents have a copy-pasteable command.
- [ ] 1.6 Do NOT change the section heading text — it SHALL still read `### 2. DONE: Task completion reporting` so the existing "Standard boot block format" four-section structure is preserved.
- [ ] 1.7 Do NOT change sections 1 (REGISTER), 3 (BLOCKED), 4 (QUESTION), or the PASTE HANDLING block.

## 2. Skill-content tests

- [ ] 2.1 Add `boot_block_done_section_leads_with_commit_instruction` to `src/skills.rs::tests`: call `build_boot_block("feat/test", "http://127.0.0.1:9119")`, locate the DONE section body, assert it contains a substring matching the commit-first instruction (e.g. `"commit your work"` or `"git commit"`), and assert that substring's byte index is less than the byte index of the manual `agent.artifact { status: "done" }` curl in the same rendered string.
- [ ] 2.2 Add `boot_block_done_section_names_committed_status_published_by_hook` to `src/skills.rs::tests`: assert the rendered boot block contains the substring `status: "committed"` (or `status:\"committed\"`) and mentions the post-commit hook in the DONE section body.
- [ ] 2.3 Add `boot_block_done_section_scopes_manual_done_to_code_less_tasks` to `src/skills.rs::tests`: assert the rendered boot block's DONE section enumerates code-less task examples (substrings for at least two of: `docs-only`, `planning`, `exploration`).
- [ ] 2.4 Add `boot_block_done_section_warns_against_manual_done_with_uncommitted_changes` to `src/skills.rs::tests`: assert the rendered boot block's DONE section contains an emphasised warning (bold via `**...**` or uppercase) against publishing manual `done` when uncommitted changes exist. The assertion SHALL check both presence of the warning phrase and the emphasis markers.
- [ ] 2.5 Add `boot_block_done_section_retains_manual_done_curl` to `src/skills.rs::tests`: assert the rendered boot block still contains a complete curl publishing `agent.artifact` with `status: "done"` to the broker URL (regression guard against accidentally deleting the fallback curl when rewriting the section).
- [ ] 2.6 Verify the existing `boot_block_contains_all_four_essential_events` test continues to pass without modification (the four section headings are unchanged).

## 3. No code changes elsewhere

- [ ] 3.1 No changes to `src/skills.rs::build_boot_block` — the substitution logic is untouched.
- [ ] 3.2 No changes to `src/agents.rs::build_post_commit_dispatcher_hook` — the hook's payload already matches the spec (`status: "committed"`, `modified_files` from `git diff HEAD~1 --name-only`).
- [ ] 3.3 No changes to the broker delivery layer (`src/broker/delivery.rs`) — both `done` and `committed` remain terminal statuses.
- [ ] 3.4 No changes to the supervisor skill (`assets/agent-skills/supervisor.md`) in this change. Any supervisor-side wording follow-up is tracked separately.
- [ ] 3.5 No changes to `assets/boot-block-template.md` sections 1, 3, 4, or PASTE HANDLING.

## 4. Quality gates

- [ ] 4.1 `just check` (fmt + clippy + tests) passes on the change branch.
- [ ] 4.2 `just deny` passes (no new dependencies).
- [ ] 4.3 `cargo build` succeeds — verifies `include_str!("../assets/boot-block-template.md")` still resolves.
- [ ] 4.4 No `unwrap()` / `expect()` introduced in test code beyond the existing `src/skills.rs::tests` conventions.
- [ ] 4.5 All five new tests (`boot_block_done_section_*`) pass; all existing boot-block tests continue to pass.

## 5. Docs

- [ ] 5.1 No `--help` text changes — there is no new CLI flag or behaviour.
- [ ] 5.2 No README changes required — the README does not quote the DONE wording verbatim.
- [ ] 5.3 If `docs/src/architecture/boot-block.md` (or equivalent mdBook chapter) exists and discusses the DONE event, update it to describe the commit-first convention and the code-less fallback. Otherwise no doc change is required.
- [ ] 5.4 `mdbook build docs/` succeeds (run as part of `just check` or independently).

## 6. Dogfood verification (optional, post-merge)

- [ ] 6.1 During the next supervisor session, capture an agent's pane shortly after attach and confirm the rendered boot block's DONE section reflects the new wording.
- [ ] 6.2 Confirm a representative code-bearing agent reaches completion via `git commit` → post-commit hook → `agent.artifact { status: "committed" }` and does NOT publish a manual `done` event.

## 7. Follow-ups (out of scope, captured)

- [ ] 7.1 If supervisor-skill wording (`assets/agent-skills/supervisor.md`) also conflates `done` and `committed` as verification triggers, schedule a separate change to prefer `committed` events.
- [ ] 7.2 Consider broker-side diagnostic: when `agent.artifact { status: "done" }` arrives with non-empty `modified_files`, log a warning that the agent may have skipped the commit step. v0.6.0 broker-hardening candidate.
