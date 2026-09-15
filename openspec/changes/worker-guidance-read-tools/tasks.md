## 1. Author the guidance

- [ ] 1.1 Add a section to `assets/agent-skills/coordination.md` instructing the agent to prefer its CLI's own file-read and search facilities over shell text-extraction pipelines when inspecting source
- [ ] 1.2 State both costs explicitly: the approval stall, and the durable grant keyed on an exact command that will never match again
- [ ] 1.3 Add the scratch-path rule: reuse one path across iterations rather than writing numbered files, because a new filename costs a fresh approval
- [ ] 1.4 Keep the section short — it is read at boot by every agent and competes for context

## 2. Enforce export-agnosticism

- [ ] 2.1 Verify the wording names no AI CLI, no vendor, and no tool belonging to one CLI; express everything as capability classes
- [ ] 2.2 Review against the `export-agnosticism` dev skill's checklist for bundled skills
- [ ] 2.3 Confirm the section does not assume a particular toolchain, shell, or operating system

## 3. Tests

- [ ] 3.1 Add skill-content assertions pinning the required section, following the existing pattern used for the other coordination-skill requirements
- [ ] 3.2 Add an assertion that the section contains no specific CLI or tool name — this is the guard against a future edit reintroducing a vendor name for readability
- [ ] 3.3 Verify every scenario in `specs/skill-agent/spec.md` maps to at least one test

## 4. Docs and gates

- [ ] 4.1 Check whether the user-guide chapter that documents the bundled coordination skill enumerates its sections; update it if so
- [ ] 4.2 `cargo fmt`, then `just lint`
- [ ] 4.3 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, verifying the real exit code rather than piped output
- [ ] 4.4 `openspec validate worker-guidance-read-tools --strict`
