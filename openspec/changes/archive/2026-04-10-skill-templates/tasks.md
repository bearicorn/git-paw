## 1. Asset directory and default skill file

- [ ] 1.1 Create directory `assets/agent-skills/`
- [ ] 1.2 Create `assets/agent-skills/coordination.md` with the coordination skill content matching the broker wire format from `message-types` — four operations (status, poll, artifact, blocked) using `{{BRANCH_ID}}` and `${GIT_PAW_BROKER_URL}` placeholders
- [ ] 1.3 Add `"assets/**/*"` to the `include` list in `Cargo.toml` so `cargo publish` ships the skill files
- [ ] 1.4 Verify `cargo package --list` includes `assets/agent-skills/coordination.md`

## 2. Module scaffolding

- [ ] 2.1 Create `src/skills.rs` with module-level doc comment explaining the purpose of the skill system, the two-level resolution order, and the substitution rules
- [ ] 2.2 Add `mod skills;` declaration in `src/main.rs`
- [ ] 2.3 Confirm `cargo build` succeeds with the empty module

## 3. SkillTemplate type and Source enum

- [ ] 3.1 Define `pub enum Source { Embedded, User }` with derives `Debug, Clone, Copy, PartialEq, Eq`
- [ ] 3.2 Define `pub struct SkillTemplate { pub name: String, pub content: String, pub source: Source }` with derives `Debug, Clone`
- [ ] 3.3 Add doc comments on the type and each field

## 4. SkillError type

- [ ] 4.1 Define `pub enum SkillError` via `thiserror` with variants:
  - `UnknownSkill { name: String }` — no embedded or user override found
  - `UserOverrideRead { path: PathBuf, source: std::io::Error }` — file exists but cannot be read
- [ ] 4.2 Wire `SkillError` into `PawError` in `src/error.rs` as a wrapped variant
- [ ] 4.3 Add doc comments on the error type and each variant

## 5. Embedded default lookup

- [ ] 5.1 Add `const COORDINATION_DEFAULT: &str = include_str!("../assets/agent-skills/coordination.md");`
- [ ] 5.2 Implement `fn embedded_default(skill_name: &str) -> Option<&'static str>` that matches `"coordination"` to `COORDINATION_DEFAULT` and returns `None` for any other name
- [ ] 5.3 Add doc comment explaining that new embedded skills are added by adding a new `include_str!` constant and a new match arm

## 6. User override lookup

- [ ] 6.1 Implement `fn try_load_user_override(skill_name: &str) -> Result<Option<String>, SkillError>`:
  - Call `dirs::config_dir()`; if `None`, return `Ok(None)`
  - Build path `<config_dir>/git-paw/agent-skills/<skill_name>.md`
  - `std::fs::read_to_string`: `NotFound` → `Ok(None)`, other errors → `Err(SkillError::UserOverrideRead { .. })`
- [ ] 6.2 Add doc comment explaining the strict error handling contract (missing = normal, unreadable = hard error)

## 7. resolve function

- [ ] 7.1 Implement `pub fn resolve(skill_name: &str) -> Result<SkillTemplate, SkillError>`:
  - Call `try_load_user_override(skill_name)?`; if `Some(content)` → return `Ok(SkillTemplate { name, content, source: Source::User })`
  - Call `embedded_default(skill_name)`; if `Some(content)` → return `Ok(SkillTemplate { name, content: content.to_string(), source: Source::Embedded })`
  - Otherwise → `Err(SkillError::UnknownSkill { name })`
- [ ] 7.2 Make `resolve` the primary public entry point; mark `try_load_user_override` and `embedded_default` as private

## 8. render function

- [ ] 8.1 Implement `pub fn render(template: &SkillTemplate, branch: &str, broker_url: &str) -> String`:
  - Call `crate::broker::messages::slugify_branch(branch)` to get the `branch_id`
  - Replace all occurrences of `{{BRANCH_ID}}` in `template.content` with `branch_id`
  - Replace all occurrences of `{{GIT_PAW_BROKER_URL}}` in `template.content` with `broker_url`
- [ ] 8.2 After substitution, scan the rendered output for any remaining `{{...}}` patterns using a simple regex or string search
- [ ] 8.3 If unknown `{{...}}` placeholders remain, write a warning to stderr via `eprintln!` identifying each unknown placeholder
- [ ] 8.4 Add doc comment explaining the `broker_url` parameter is used to substitute `{{GIT_PAW_BROKER_URL}}` at render time

## 9. Unit tests

- [ ] 9.1 Add `#[cfg(test)] mod tests` block at the bottom of `src/skills.rs`
- [ ] 9.2 Test: embedded coordination skill is reachable without any user files — `resolve("coordination")` returns `Ok` with `Source::Embedded` and non-empty content
- [ ] 9.3 Test: embedded coordination skill contains all four operations — assert content contains `agent.status`, `agent.artifact`, `agent.blocked`, `{{GIT_PAW_BROKER_URL}}/messages/{{BRANCH_ID}}`
- [ ] 9.4 Test: user override is preferred — create a `tempdir` as config_dir, write `coordination.md` to it, call `try_load_user_override` and assert it returns the custom content (use a test helper that temporarily overrides the config dir path)
- [ ] 9.5 Test: missing user config directory falls through — `try_load_user_override` returns `Ok(None)` when config_dir is a nonexistent path
- [ ] 9.6 Test: missing agent-skills subdirectory falls through — `try_load_user_override` returns `Ok(None)`
- [ ] 9.7 Test: missing skill file falls through — `try_load_user_override` returns `Ok(None)` when directory exists but file does not
- [ ] 9.8 Test: unreadable user override returns hard error — create file with `0o000` permissions, assert `try_load_user_override` returns `Err(SkillError::UserOverrideRead { .. })`
- [ ] 9.9 Test: unknown skill name returns error — `resolve("nonexistent")` returns `Err(SkillError::UnknownSkill { .. })`
- [ ] 9.10 Test: `{{BRANCH_ID}}` is substituted — render a template containing `{{BRANCH_ID}}` with branch `"feat/http-broker"`, assert output contains `feat-http-broker` and no `{{BRANCH_ID}}`
- [ ] 9.11 Test: `{{GIT_PAW_BROKER_URL}}` is substituted at render time — render a template containing `{{GIT_PAW_BROKER_URL}}`, assert the literal URL is present and no placeholder remains
- [ ] 9.12 Test: slug substitution matches `slugify_branch` — render with branch `"Feature/HTTP_Broker"` and assert the substitution equals `slugify_branch("Feature/HTTP_Broker")`
- [ ] 9.13 Test: render is deterministic — call render twice with the same inputs, assert outputs are identical
- [ ] 9.14 Test: render performs no I/O — resolve a template, then (simulated) confirm render succeeds without any filesystem access
- [ ] 9.15 Test: unknown placeholder triggers a warning — render a template containing `{{UNKNOWN_THING}}` and assert it survives in the output (testing the warning itself requires capturing stderr, which may be impractical in unit tests; document as a manual verification or integration test)
- [ ] 9.16 Test: no warning when only known placeholders are present — render the embedded coordination template and assert no `{{...}}` remains in the output after substitution
- [ ] 9.17 Test: `SkillTemplate` is cloneable — resolve, clone, assert fields match

## 10. Testability consideration

- [ ] 10.1 Consider making the config directory path injectable in `try_load_user_override` (e.g. accept an `Option<&Path>` override parameter, default to `dirs::config_dir()`) so tests can point at a tempdir without mocking `dirs`. If this makes the API cleaner, expose the override internally; keep the public `resolve` signature unchanged.

## 11. Quality gates

- [ ] 11.1 `cargo fmt` clean
- [ ] 11.2 `cargo clippy --all-targets -- -D warnings` clean (all public items documented, no `unwrap`/`expect` outside tests)
- [ ] 11.3 `cargo test` — all new tests pass
- [ ] 11.4 `cargo doc --no-deps` builds without warnings for the new module
- [ ] 11.5 `just check` — full pipeline green

## 12. Handoff readiness

- [ ] 12.1 Confirm `src/skills.rs` exposes `resolve`, `render`, `SkillTemplate`, `Source`, `SkillError` as public API
- [ ] 12.2 Confirm `assets/agent-skills/coordination.md` is a tracked file that renders correctly in a markdown viewer
- [ ] 12.3 Confirm no changes to `src/agents.rs` (that belongs to `skill-injection` in Wave 2)
- [ ] 12.4 Confirm no changes outside `src/skills.rs`, `src/main.rs` (mod declaration), `src/error.rs` (SkillError integration), `Cargo.toml` (include list), and `assets/`
- [ ] 12.5 Commit with message: `feat(skills): add skill template loading and rendering`
