# Tasks — standards-skill-integration

## 1. Supervisor gate consult (exported)
- [x] Edit `assets/agent-skills/supervisor.md`: add a review-gate step to consult the project's `.agents/skills/` standards skills (e.g. `test-strategy`, `code-standards`) when present and verify conformance — project-agnostic wording, no git-paw stack specifics
- [x] Confirm the step sits inside the appropriate (region-gated if applicable) section so non-OpenSpec/other consumers still parse cleanly

## 2. Worker consult (exported)
- [x] Edit `assets/agent-skills/coordination.md` (and/or the boot-block guidance) to direct the implementing agent to consult the project's `.agents/skills/` standards skills during implementation — project-agnostic wording
- [x] Ensure absent standards → no-op (backward compatible)

## 3. Ripple + sync (scope up front)
- [x] Grep `src/skills.rs` + `tests/*_skill_content.rs` (`coordination_region_skill_content.rs`, `supervisor_routing_skill_content.rs`, …) for the pinned literals; update those tests in lockstep with the prose edits
- [x] Sync the tracked `.git-paw/scripts`/skill copies from `assets/` — N/A: this change edits only the exported skill prose; tests read `assets/` directly via `include_str!`, and the script-drift hazard (sweep.sh/broker.sh) is untouched
- [x] Extend the `lang-agnostic-skills` no-language-leak audit to cover the new consult wording — added `standards_consult_step_passes_no_leak_audit_across_backends` (renders supervisor + coordination across every backend, asserts the consult step present + no stack/language token). Repo-internal guard on the EXPORTED skills, so consumers still get stack-agnostic assets

## 4. Verification (five gates)
- [ ] Gate 1/2 — `skills.rs` + `*_skill_content.rs` updated and green; full suite green vs merge-base
- [ ] Gate 3 — every `standards-skill-integration` scenario maps to a test (gate-step present, no-baked-standard, worker-step present, absent→no-op)
- [ ] Gate 4 — mdBook/user-guide notes the supervisor consults project standards at the gate (if the supervisor chapter documents gates)
- [ ] Gate 5 — security/agnosticism: exported assets carry only generic "consult the project's standards" wording; no git-paw specifics leaked
- [ ] `just check` green; `cargo fmt` before commit
- [ ] `openspec validate standards-skill-integration --strict` passes

## Notes
- Consumes `.agents/skills/` standards (git-paw's own `test-strategy` + `code-standards` for dogfood;
  consumers supply their own). Additive + backward-compatible.
