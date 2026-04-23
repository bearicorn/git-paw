## 1. Supervisor skill template

- [ ] 1.1 Create `assets/agent-skills/supervisor.md` with the supervisor instruction set:
  - Role: monitor and verify, do NOT write code
  - Context: `{{PROJECT_NAME}}`, `{{GIT_PAW_BROKER_URL}}`
  - Skills: curl commands for /status, /messages/supervisor, POST /publish (verified, feedback)
  - Skills: tmux capture-pane and send-keys with `paw-{{PROJECT_NAME}}`
  - Workflow: baseline capture → watch → test ({{TEST_COMMAND}}) → regression check → verify/feedback → merge order → summarize
  - Regression detection: compare test results against baseline, flag any previously-passing test that now fails
  - Conflict detection: check modified_files overlap across agents
  - Rules: don't write code, ask human for merges, escalate on ambiguity
- [ ] 1.2 Keep the template under ~80 lines to avoid context window pressure

## 2. Embed in skills module

- [ ] 2.1 Add `const SUPERVISOR_DEFAULT: &str = include_str!("../assets/agent-skills/supervisor.md");` to `src/skills.rs`
- [ ] 2.2 Add match arm in `embedded_default()`: `"supervisor" => Some(SUPERVISOR_DEFAULT)`
- [ ] 2.3 Verify `resolve("supervisor")` returns the embedded content

## 3. Render function update

- [ ] 3.1 Add `project: &str` parameter to `render()` function signature
- [ ] 3.2 Add `template.content.replace("{{PROJECT_NAME}}", project)` to the substitution chain
- [ ] 3.3 Update all call sites in `src/main.rs` to pass the project name
- [ ] 3.4 Update doc comment on `render()` to document the new parameter

## 4. Unit tests

- [ ] 4.1 Test: `resolve("supervisor")` returns `Ok` with `Source::Embedded` and non-empty content
- [ ] 4.2 Test: supervisor skill contains "do NOT write code" or equivalent
- [ ] 4.3 Test: supervisor skill contains `{{GIT_PAW_BROKER_URL}}/status`
- [ ] 4.4 Test: supervisor skill contains `agent.verified` and `agent.feedback`
- [ ] 4.5 Test: supervisor skill contains `tmux capture-pane` and `paw-{{PROJECT_NAME}}`
- [ ] 4.6 Test: `render()` substitutes `{{PROJECT_NAME}}` correctly
- [ ] 4.7 Test: `render()` substitutes both `{{BRANCH_ID}}` and `{{PROJECT_NAME}}` in same template
- [ ] 4.8 Test: user override for supervisor skill is preferred over embedded
- [ ] 4.9 Test: existing `resolve("coordination")` still works (no regression)
- [ ] 4.10 Test: existing `render()` calls with new parameter still produce correct output

## 5. Quality gates

- [ ] 5.1 `cargo fmt` clean
- [ ] 5.2 `cargo clippy --all-targets -- -D warnings` clean
- [ ] 5.3 `cargo test` — all tests pass
- [ ] 5.4 `just check` — full pipeline green

## 6. Handoff readiness

- [ ] 6.1 Confirm `assets/agent-skills/supervisor.md` exists and is tracked
- [ ] 6.2 Confirm `resolve("supervisor")` works
- [ ] 6.3 Confirm `render()` signature updated with `project` parameter
- [ ] 6.4 Confirm no changes outside `src/skills.rs`, `src/main.rs`, `assets/`, and test files
- [ ] 6.5 Commit with message: `feat(skills): add supervisor skill template`
