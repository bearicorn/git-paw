## 1. Extract `build_task_prompt` helper

- [x] 1.1 In `src/main.rs`, add a pure helper `pub(crate) fn build_task_prompt(spec_entry: Option<&git_paw::specs::SpecEntry>) -> String` at module scope (next to the other supervisor-launch helpers like `resolve_dispatch_target`).
- [x] 1.2 When `spec_entry` is `Some(s)`, return a string that mentions `AGENTS.md`, mentions `openspec/changes/<id>/` (using `s.id`), and instructs the agent to read AGENTS.md plus the sibling artifacts (proposal, design, specs, tasks) before starting. Don't include any portion of `s.prompt` body in the returned string.
- [x] 1.3 When `spec_entry` is `None`, return the existing default fallback string `"Begin your assigned task as described in AGENTS.md."` verbatim.
- [x] 1.4 Add a doc comment on the helper noting (a) AGENTS.md is the source of truth for the spec body, written separately by `setup_worktree_agents_md`, and (b) callers SHALL ensure `setup_worktree_agents_md` runs before the boot prompt is injected.

## 2. Wire the helper into `cmd_supervisor`

- [x] 2.1 In `cmd_supervisor`'s pane-construction loop (around `src/main.rs:817`), replace the inline `spec_entry.map(|s| s.prompt.lines().next()...)` chain with a call to `build_task_prompt(spec_entry)`.
- [x] 2.2 Remove the now-unused inline `.filter(|p| !p.is_empty())` chain and the inline default-fallback `.unwrap_or_else(|| ...)`.
- [x] 2.3 Verify the `full_prompt = format!("{boot_block}\n\n{task_prompt}")` line below still works against the helper's return type (`String`).

## 3. Tests

- [x] 3.1 Unit test `task_prompt_with_spec_points_at_agents_md_and_includes_id` in `src/main.rs::tests`: construct a `SpecEntry { id: "my-change", branch: "feat/my-change", cli: None, prompt: "## 1. First section\n\nbody body body".to_string(), owned_files: None }`, call `build_task_prompt(Some(&entry))`, assert the result contains `AGENTS.md`, contains `openspec/changes/my-change`, does NOT contain `## 1. First section` in raw form, and does NOT contain `body body body`.
- [x] 3.2 Unit test `task_prompt_without_spec_uses_default_agents_md_fallback` in `src/main.rs::tests`: call `build_task_prompt(None)`, assert the result equals `"Begin your assigned task as described in AGENTS.md."` exactly.
- [x] 3.3 Unit test `task_prompt_does_not_include_spec_body_first_line`: a regression test for the original bug. Construct a `SpecEntry` whose `prompt` starts with `## 1. Code fix in cmd_supervisor`, call the helper, assert the returned string does NOT start with `## 1. Code fix` and does NOT contain that heading anywhere.

## 4. Quality gates

- [x] 4.1 `just check` (fmt + clippy + tests) passes on the change branch.
- [x] 4.2 `just deny` passes (no new dependencies).
- [x] 4.3 No `unwrap()`/`expect()` introduced in the helper or its call site.
- [x] 4.4 The helper has a doc comment explaining the AGENTS.md prerequisite.

## 5. Docs

- [x] 5.1 No `--help` text changes — there is no new flag.
- [x] 5.2 No README changes — this is internal behaviour.
- [x] 5.3 If `docs/src/user-guide/spec-driven-launch.md` mentions the boot-prompt content, update it to reflect that the boot prompt points at AGENTS.md rather than excerpting the spec.

## 6. Dogfood verification

- [ ] 6.1 Build the binary on the change branch.
- [ ] 6.2 Resume the v0.5.0 dogfood session (or relaunch fresh) and confirm agents that get the new boot prompt read AGENTS.md and proceed without publishing the "task description truncated" `agent.question` event that drift item 29 originally surfaced.

## 7. Follow-ups (out of scope, captured for v0.5.0 cleanup or later)

- [ ] 7.1 Boot block template (`assets/boot-block-template.md`) does not currently reference AGENTS.md. Schedule a small follow-up that adds a one-liner before the four coordination instructions so the AGENTS.md pointer is reinforced regardless of the task-prompt content.
- [ ] 7.2 When the `spec-kit-format` change ships, extend `build_task_prompt` to use the Spec Kit directory layout (`.specify/...`) instead of `openspec/changes/<id>/` for branches whose spec_entry came from the Spec Kit backend.
