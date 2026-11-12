## 1. Seam: render and test the package as a whole (no content moved)

- [x] 1.1 `skills`: render a package's references with the `SKILL.md` placeholder set; a full-package render helper (`SKILL.md` + named references, each under `## Reference: <path>`)
- [x] 1.2 Re-point orchestrator skill-content tests (`src/skills.rs` tests, `tests/*_skill_content.rs`, `tests/governance_supervisor_boot.rs`, `tests/skill_stuck_shapes_prose.rs`, `tests/lang_agnostic_skill_audit.rs`) to the full-package render; suite green with content still in `SKILL.md`

## 2. Move the audit gates into references

- [x] 2.1 Create `assets/agent-skills/orchestrator/references/gates/{spec-audit,doc-audit,security-audit}.md`; move the Spec Audit Procedure (+ governance sub-step), and the bodies of Workflow steps 6, 6a and 6b out of `SKILL.md`; register them; each opens with its inputs paragraph (tip, merge-base, change id, gate command) and restates running the gate command bare, never piped
- [x] 2.2 `SKILL.md` Workflow: steps 6 / 6a / 6b keep one numbered step per audit gate with purpose + installed reference path (`.git-paw/skills/orchestrator/references/gates/<gate>.md`) + "read before running"; testing and regression stay inline
- [x] 2.3 Tests: "The three audit references exist and are rendered", "Gate content is moved, not duplicated", "Each reference is self-contained", "Each reference restates the bare-command rule", "Testing and regression stay inline", "Each audit gate step names its installed reference", "A moved section still satisfies its requirement"
- [x] 2.4 E2E test "A headless hand-off that reads SKILL.md reaches the gates": after `git paw init`, every reference path named by the installed `.git-paw/skills/orchestrator/SKILL.md` exists under the repository root

## 3. Delegation contract

- [x] 3.1 Add the "Delegating gates" section to the orchestrator `SKILL.md` (verdict shape, orchestrator-only publishing, ordering after testing and regression, inline fallback, read-only exploration delegation), vendor-neutral
- [x] 3.2 Tests: "The delegation section defines the verdict shape", "Delegation follows testing and regression", "Only the orchestrator publishes", "Inline fallback without subagents", "The delegation section is vendor-neutral" (audit)

## 4. Self-hosted session runs the branch's build (DF-58)

- [x] 4.1 Add to the orchestrator `SKILL.md` Bootstrap: when the orchestrated repository is git-paw's own source, confirm the `git paw` the session runs is the build from the branch under test, not an installed copy; if not, tell the human to rebuild with the project's own build and run the session on it before driving any agent; give the reason (a stale binary silently re-introduces fixed bugs); name no build tool, package manager or recipe
- [x] 4.2 Tests in `tests/orchestrator_skill_content.rs`: "A self-hosted session checks the binary it runs", "A stale binary is raised before work is driven", "The instruction names no build command" (asserts the section carries no `cargo` / `just` / `make` / `npm` token and the package passes `tests/lang_agnostic_skill_audit.rs`)

## 4a. Baseline step prepares the checkout's dependencies (PK-10)

- [x] 4a.1 Workflow step 1 (Baseline) in the orchestrator `SKILL.md`: before recording, make sure the root checkout (or the verify worktree) has the project's dependencies installed the way the project's own setup or `[worktree.hooks] on_create` hook does it; if the baseline fails for missing dependencies, install them and re-run rather than record a red baseline; name no package manager or build tool
- [x] 4a.2 Tests in `tests/orchestrator_skill_content.rs`: "The Baseline step prepares the checkout first", "A baseline that fails for missing dependencies is not recorded", "The Baseline instruction names no package manager" (asserts the Baseline step carries no `npm` / `pip` / `cargo` / `just` / `make` / `yarn` / `pnpm` token and the package passes `tests/lang_agnostic_skill_audit.rs`)

## 5. MCP

- [x] 5.1 `mcp/tools/project.rs::get_skill` returns the full-package render (orchestrator: the `render_standardized` `SKILL.md` then its references); move `get_skill_serves_the_orchestrator_skill_as_exported_under_both_names` to compare against the package render; tests "get_skill includes the orchestrator's gate references", "A skill without references is unchanged" (no disk write)

## 6. Docs

- [x] 6.1 `docs/src/user-guide/supervisor.md` verification section: gate references, delegation contract, no persona files (and why), the self-hosted-session check; `skill-templates.md`: bundled references; `mcp.md`: `get_skill` returns references
- [x] 6.2 `AGENTS.md` five-gate section unchanged in substance; note that gates 3–5 have per-gate references (repo-local doc, not an export)

## 7. Gates

- [ ] 7.1 Every scenario maps to a named test; byte-for-byte check that the union of `SKILL.md` + references contains every pre-move gate sentence
- [ ] 7.2 `just check` (exit code read directly), `just deny`, `mdbook build docs/` green; full suite vs merge-base green
