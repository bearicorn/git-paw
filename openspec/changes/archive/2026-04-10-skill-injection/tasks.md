## 1. Extend WorktreeAssignment

- [ ] 1.1 Add `pub skill_content: Option<String>` field to `WorktreeAssignment` in `src/agents.rs`
- [ ] 1.2 Update all existing call sites that construct `WorktreeAssignment` to include `skill_content: None` so they compile unchanged

## 2. Extend generate_worktree_section

- [ ] 2.1 In `generate_worktree_section`, after the file ownership block and before pushing `END_MARKER`, add:
  ```rust
  if let Some(ref skill) = assignment.skill_content {
      section.push('\n');
      section.push_str(skill);
      if !skill.ends_with('\n') {
          section.push('\n');
      }
  }
  ```
- [ ] 2.2 Verify the skill content appears inside the markers (after ownership, before `<!-- git-paw:end -->`)

## 3. Launch flow integration

- [ ] 3.1 In the `start` flow in `src/main.rs`, when `config.broker.enabled`:
  - Call `skills::resolve("coordination")?` once to get the `SkillTemplate`
  - Compute `broker_url` from `config.broker.url()`
- [ ] 3.2 For each worktree's `WorktreeAssignment`, call `skills::render(&template, &branch, &broker_url)` and set `skill_content = Some(rendered)`
- [ ] 3.3 When `config.broker.enabled` is false, leave `skill_content = None` for all assignments

## 4. Unit tests

- [ ] 4.1 Add test: `generate_worktree_section` with all fields including `skill_content` — assert the output contains the skill text inside markers, after file ownership
- [ ] 4.2 Add test: `generate_worktree_section` with `skill_content` but no spec or files — assert skill text present after assignment
- [ ] 4.3 Add test: `generate_worktree_section` with `skill_content = None` — assert output is identical to the existing v0.2.0 tests (regression check)
- [ ] 4.4 Add test: rendered skill content contains slugified branch (e.g. `feat-http-broker`) and does not contain literal `{{BRANCH_ID}}`
- [ ] 4.5 Add test: rendered skill content contains literal `${GIT_PAW_BROKER_URL}` (not substituted)
- [ ] 4.6 Verify all existing `agents.rs` tests still pass (they construct `WorktreeAssignment` without `skill_content`, which is now `None`)

## 5. Quality gates

- [ ] 5.1 `cargo fmt` clean
- [ ] 5.2 `cargo clippy --all-targets -- -D warnings` clean
- [ ] 5.3 `cargo test` — all unit tests pass (new + existing)
- [ ] 5.4 `cargo doc --no-deps` builds without warnings
- [ ] 5.5 `just check` — full pipeline green

## 6. Handoff readiness

- [ ] 6.1 Confirm changes are limited to `src/agents.rs` and `src/main.rs`
- [ ] 6.2 Confirm `skill_content: None` produces byte-identical output to v0.2.0 for all existing test cases
- [ ] 6.3 Confirm no changes to `src/skills.rs` (that module is owned by `skill-templates`)
- [ ] 6.4 Commit with message: `feat(agents): inject coordination skill into worktree AGENTS.md`
