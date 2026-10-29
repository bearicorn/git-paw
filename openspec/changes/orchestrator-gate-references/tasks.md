## 1. Seam: render and test the package as a whole (no content moved)

- [ ] 1.1 `skills`: render a package's references with the `SKILL.md` placeholder set; a full-package render helper (`SKILL.md` + named references, each under `## Reference: <path>`)
- [ ] 1.2 Re-point orchestrator skill-content tests (`src/skills.rs` tests, `tests/*_skill_content.rs`, `tests/governance_supervisor_boot.rs`, `tests/skill_stuck_shapes_prose.rs`, `tests/lang_agnostic_skill_audit.rs`) to the full-package render; suite green with content still in `SKILL.md`

## 2. Move the audit gates into references

- [ ] 2.1 Create `assets/agent-skills/orchestrator/references/gates/{spec-audit,doc-audit,security-audit}.md`; move the Spec Audit Procedure (+ governance sub-step), doc-audit and security-audit bodies out of `SKILL.md`; register them; each opens with its inputs paragraph (tip, merge-base, change id, gate command)
- [ ] 2.2 `SKILL.md` Workflow: one heading per audit gate with purpose + installed reference path (`{{…}}`-rendered) + "read before running"; testing and regression stay inline
- [ ] 2.3 Tests: "The three audit references exist and are rendered", "Each reference is self-contained", "Gate content is moved, not duplicated", "Testing and regression stay inline", "A moved section still satisfies its requirement"

## 3. Delegation contract

- [ ] 3.1 Add the "Delegating gates" section to the orchestrator `SKILL.md` (verdict shape, orchestrator-only publishing, ordering, inline fallback, read-only exploration delegation), vendor-neutral
- [ ] 3.2 Tests: "The delegation section defines the verdict shape", "Only the orchestrator publishes", "Inline fallback without subagents", "The delegation section is vendor-neutral" (audit)

## 4. MCP

- [ ] 4.1 `mcp/tools/project.rs::get_skill` returns the full-package render; tests "get_skill includes the orchestrator's gate references", "A skill without references is unchanged" (no disk write)

## 5. Docs

- [ ] 5.1 `docs/src/user-guide/supervisor.md` verification section: gate references, delegation contract, no persona files (and why); `skill-templates.md`: bundled references; `mcp.md`: `get_skill` returns references
- [ ] 5.2 `AGENTS.md` five-gate section unchanged in substance; note that gates 3–5 have per-gate references (repo-local doc, not an export)

## 6. Gates

- [ ] 6.1 Every scenario maps to a named test; byte-for-byte check that the union of `SKILL.md` + references contains every pre-move gate sentence
- [ ] 6.2 `just check` (exit code read directly), `just deny`, `mdbook build docs/` green; full suite vs merge-base green
