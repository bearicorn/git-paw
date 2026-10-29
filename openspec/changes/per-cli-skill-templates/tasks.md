## 1. Seam: fragments with today's text (behavior-preserving for Claude agents)

- [ ] 1.1 Registry: fragments at `assets/agent-skills/coordination/assets/cli/<family>/{skill,boot}.md`; pure `select_fragment(family, slot, package)` with `generic` fallback; table test (registered family, unregistered family, `generic`)
- [ ] 1.2 Move the "Per-CLI mechanism" table body out of the coordination `SKILL.md` into `claude/skill.md` (`/compact`, `/clear`) and `generic/skill.md` (look for `/compact`, `/save`, `/reset`); `{{CLI_NOTES}}` in its place; drop the `claude-oss` row
- [ ] 1.3 Move the boot-block paste paragraph into `claude/boot.md` (`[Pasted text #N]` + second Enter) and `generic/boot.md` (vendor-neutral + second Enter); `{{CLI_BOOT_NOTES}}` in both boot-block forms
- [ ] 1.4 `render` takes the skill family; `build_boot_block(branch_id, broker_url, &CliProfile)` reads both the helper/MCP form and the family from the profile (no `mcp_capable` flag, no CLI name); add both placeholders to the known set; repository-root render and MCP `get_skill` pass `generic`; tests "Function accepts required parameters", "Function returns boot block string", "The generic profile yields the helper form with the generic fragment"

## 2. Wire the profile

- [ ] 2.1 Read the skill family from the v0.19.0 resolved CLI profile at every per-agent render site (`commands/{start,add,supervisor}.rs`, `main.rs::attach_agent`, package provisioning); the skill family is the record's provider family (custom variants inherit their declared family; unknown ⇒ `generic`)
- [ ] 2.2 Tests: "A Claude-family agent gets the Claude fragment", "An unrecognised CLI gets the generic fragment", "A custom variant inherits its declared family", "A family without a fragment falls back to generic", "Agents on different CLIs in one session get different fragments" (e2e over the sidecar), "A render with no agent uses generic"
- [ ] 2.3 Verify `codex` and `agy` compaction commands and paste display against each CLI's current documentation; author `codex/` / `agy/` fragments only for verified facts, otherwise ship none (they render `generic`) and record the outcome in the changelog

## 3. Requirement tests

- [ ] 3.1 Context-budget tests: "Claude-family agent sees its slash commands", "Generic fallback points users at their CLI's equivalent", "No custom variant is named" (replace the old claude/claude-oss table tests)
- [ ] 3.2 Boot-block tests for both forms: "Paste handling instruction included" (every family), "Paste instruction format for a Claude-family agent", "Paste instruction for a generic agent names no vendor"

## 4. Export guard

- [ ] 4.1 `tests/lang_agnostic_skill_audit.rs`: render per family + `generic`; vendor tokens allowed only from the family's own fragment; regression tests "A fragment naming another vendor fails" and "A vendor name in a base template fails"; "Generic rendering is vendor-free"

## 5. Docs

- [ ] 5.1 `docs/src/user-guide/skill-templates.md`: CLI fragments, families, `generic` fallback, how custom variants get a family, overrides may use `{{CLI_NOTES}}`; placeholders table gains `{{CLI_NOTES}}` / `{{CLI_BOOT_NOTES}}`
- [ ] 5.2 `docs/src/user-guide/coordination.md` context-budget section and `docs/src/user-guide/agents-md.md` boot-block section reflect per-CLI rendering

## 6. Gates

- [ ] 6.1 Every scenario maps to a named test
- [ ] 6.2 `just check` (exit code read directly), `just deny`, `mdbook build docs/` green; full suite vs merge-base green
