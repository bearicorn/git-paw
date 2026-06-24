# Tasks — coding-agent-commit-discipline

## 1. Edit the bundled coordination skill (`assets/agent-skills/coordination.md`)

- [ ] 1.1 Add a positive **stand-by-after-commit** protocol. Strengthen the existing
      `### Terminal action: commit then publish, never archive` section (and/or the
      tail of the `opsx-role-gating` block) into an explicit "STAND BY after your final
      commit" instruction: after committing, rely on the auto `agent.artifact` publish
      (or a manual `status: "done"` for code-less work), then **wait** — do not run
      `/opsx:verify` or `/opsx:archive`. Cross-reference the role-gating block instead
      of restating the forbidden-commands list. State what the agent waits for:
      `agent.verified`, `agent.feedback`, or further `agent.intent`. Edit prose
      *outside* the `<!-- opsx-role-gating:begin/end -->` sentinels.
- [ ] 1.2 Add **releasable-unit + amend-fixups** guidance to the `### Commit cadence`
      section: each commit MUST build/pass its own gates (a releasable unit); fold a
      small follow-up to the just-made commit in with `git commit --amend` rather than a
      separate micro-commit; do NOT `--amend` an already-verified commit or an earlier
      group's commit; tie the rationale to per-commit verification + supervisor-curated
      changelog hygiene (the prior-cycle squash cost).
- [ ] 1.3 **De-opinionate the commit-format prose**: replace the "Use the project's
      conventional-commit prefix per group … `feat(<scope>):` …" prescription in
      `### Commit cadence` with "follow the project's commit-message conventions (see the
      project's `AGENTS.md`)". A Conventional-Commits prefix MAY remain as an illustrative
      example but SHALL NOT be stated as mandatory. Keep the per-group cadence and
      ~10-file ceiling unchanged.

## 2. Cross-reference the supervisor skill (`assets/agent-skills/supervisor.md`)

- [ ] 2.1 In the supervisor skill's commit-cadence / verification guidance, add a note
      that the verify-then-archive workflow depends on coding agents standing by after
      their final commit: the supervisor (not the agent) runs `/opsx:verify` and
      `/opsx:archive` once the post-commit `agent.artifact` arrives. Cross-reference the
      agent-side stand-by protocol in `coordination.md`.

## 3. Skill-content tests

- [ ] 3.1 Add a skill-content test (new `tests/commit_discipline_skill_content.rs`, or
      extend an existing `tests/*_skill_content.rs`) asserting the **stand-by** prose:
      the coordination skill instructs standing by after the final commit, waits for
      `agent.verified` / `agent.feedback` / `agent.intent`, and forbids self
      `/opsx:verify` / `/opsx:archive` while cross-referencing role-gating. (Maps:
      "Coordination skill — stand-by after final commit" scenarios.)
- [ ] 3.2 Add skill-content tests asserting the **releasable-unit + amend** prose: each
      commit is a releasable unit; amend the just-made commit for small fixups; do NOT
      amend an already-verified / earlier commit. (Maps: "releasable-unit commit
      discipline with amend fixups" scenarios.)
- [ ] 3.3 Add a skill-content test asserting the **commit-format deferral**: the
      "Commit cadence" section references the project's `AGENTS.md` for message format
      and does NOT mandate a specific format. (Maps: "Coordination skill defers
      commit-message format to the project AGENTS.md" scenario.)
- [ ] 3.4 Add a skill-content test asserting the **supervisor cross-reference**: the
      supervisor skill states the supervisor verifies/archives post-commit and
      cross-references the coordination stand-by protocol. (Maps:
      supervisor-skill-discipline "supervisor relies on agents standing by" scenario.)
- [ ] 3.5 Audit existing skill-content / lang-agnostic-audit tests
      (`tests/coordination_*_skill_content.rs`, `tests/lang_agnostic_skill_audit.rs`,
      `tests/supervisor_*_skill_content.rs`) for any assertion that pins the removed
      Conventional-Commits-mandatory prose; update them to match the de-opinionated text
      so the suite stays green.

## 4. Docs

- [ ] 4.1 Update the skill/agent docs that describe the post-commit protocol and commit
      cadence (mdBook chapter under `docs/src/` covering agent skills / coordination, and
      any `--help`/README surface that summarises the commit cadence) so they reflect the
      stand-by protocol, the releasable-unit/amend guidance, and the deferral of
      commit-message format to the project `AGENTS.md`. Run `mdbook build docs/`.

## 5. Quality gates

- [ ] 5.1 `just check` (fmt + clippy + all tests) passes.
- [ ] 5.2 `just deny` passes (no dependency change expected — skill-prose-only).
- [ ] 5.3 `openspec validate coding-agent-commit-discipline --strict` passes.
- [ ] 5.4 Confirm backward compatibility: no code-path or config change; the skill renders
      and injects through the existing `skills.rs` machinery unchanged.
