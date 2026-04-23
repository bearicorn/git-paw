## 1. Spec audit section in supervisor.md

- [ ] 1.1 Add a `### Spec Audit Procedure` section to `assets/agent-skills/supervisor.md`
- [ ] 1.2 Write step 1: locate spec files at `openspec/changes/<change-name>/specs/`
- [ ] 1.3 Write step 2: read each spec file and extract `#### Scenario:` blocks
- [ ] 1.4 Write step 3: for each scenario, extract the THEN assertion and grep test files for a matching test
- [ ] 1.5 Write step 4: for each `### Requirement:` block, read SHALL/MUST statements and verify implementation matches (field names, signatures, types)
- [ ] 1.6 Write step 5: compile gap list — untested scenarios, field mismatches, missing implementations
- [ ] 1.7 Write step 6: if gaps → publish `agent.feedback` with errors list; if clean → include "spec audit clean" in `agent.verified`
- [ ] 1.8 Position the spec audit section after the test command section and before the merge ordering section in the workflow

## 2. Workflow ordering

- [ ] 2.1 Verify the supervisor.md workflow sections are ordered: baseline → watch → test → spec audit → verify/feedback → merge → summarize
- [ ] 2.2 Add a note that spec audit is skipped if the test command fails (no point auditing if tests don't pass)

## 3. Tests

- [ ] 3.1 Test: supervisor skill contains `Spec Audit` substring
- [ ] 3.2 Test: supervisor skill contains instructions to read `openspec/changes/`
- [ ] 3.3 Test: supervisor skill contains instructions to grep for tests
- [ ] 3.4 Test: supervisor skill contains instructions to verify field names
- [ ] 3.5 Test: spec audit section appears after test command section in the template
- [ ] 3.6 Test: spec audit section appears before verified/feedback publish section

## 4. Quality gates

- [ ] 4.1 `cargo fmt` clean
- [ ] 4.2 `cargo test` — all tests pass
- [ ] 4.3 `just check` — full pipeline green

## 5. Handoff readiness

- [ ] 5.1 Confirm only `assets/agent-skills/supervisor.md` is modified (plus test assertions)
- [ ] 5.2 Confirm no Rust code changes
- [ ] 5.3 Commit with message: `feat(skills): add spec audit procedure to supervisor template`
