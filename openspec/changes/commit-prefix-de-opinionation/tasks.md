## 1. Spec alignment

- [ ] 1.1 Apply the MODIFIED `agent-skills` delta: de-opinionate the "Coordination skill SHALL teach per-group commit cadence" requirement (item 3) — commit-message format defers ENTIRELY to the project's `AGENTS.md`; no Conventional-Commits mandate, default, or recommendation; examples are format-neutral (delta at `specs/agent-skills/spec.md`).
- [ ] 1.2 Apply the ADDED `lang-agnostic-skills` delta: bundled skills are convention-agnostic; the bundled-skill leak audit also flags a hardcoded commit-convention mandate (delta at `specs/lang-agnostic-skills/spec.md`).
- [ ] 1.3 Run `openspec validate commit-prefix-de-opinionation --strict` and confirm it passes.

## 2. Genericise the exported skill (remove git-paw's convention from the export)

- [ ] 2.1 In `assets/agent-skills/coordination.md` "Commit cadence" section, replace the `feat(coverage): …` example commits (the `(part N of M)` illustration) with FORMAT-NEUTRAL example subjects (no convention-specific prefix).
- [ ] 2.2 Remove the Conventional-Commits illustration prose ("Many projects use a Conventional-Commits prefix such as `feat(<scope>):` …") so the section only defers to `AGENTS.md`; keep the deferral sentence.
- [ ] 2.3 Confirm the per-group cadence (commit per task group, ~10-file soft cap, `(part N of M)` split) and the releasable-unit / `git commit --amend` discipline remain present and unchanged.
- [ ] 2.4 Confirm git-paw's OWN `AGENTS.md` / `CLAUDE.md` Conventional-Commits convention is NOT touched (it is the repo-specific side of the separation).

## 3. Tests

- [ ] 3.1 Reframe the stale assertion in `src/skills.rs::coordination_skill_documents_commit_cadence` (the `has_conventional_prefix` block ~line 4880) so it no longer requires a Conventional-Commits prefix example; instead assert the section references `AGENTS.md` for message conventions and contains NO Conventional-Commits prefix (`feat(`/`fix(`) as a mandate/default/example.
- [ ] 3.2 Extend the bundled-skill leak audit (`src/skills.rs`, the no-language-leak audit) with a commit-convention check: the rendered supervisor + coordination skills SHALL NOT contain a Conventional-Commits prefix presented as a mandate/default/recommendation.
- [ ] 3.3 Confirm `tests/commit_discipline_skill_content.rs::commit_cadence_defers_message_format_to_project_agents_md` still passes; confirm the per-group grain / soft-cap and releasable-unit assertions still pass.

## 4. Quality gates

- [ ] 4.1 Run `just check` (fmt + clippy + tests) and confirm green.
- [ ] 4.2 Run `mdbook build docs/` and confirm the docs build; verify `docs/src/user-guide/coordination.md` carries no Conventional-Commits mandate.
