## 1. Seam: fragments with today's text (behavior-preserving for Claude agents)

- [ ] 1.1 Registry: fragments at `assets/agent-skills/coordination/assets/cli/<family>/{skill,boot}.md` (`<family>` = `CliFamily::as_str()`); pure `select_fragment(family: CliFamily, slot, package)` with `generic` fallback, no comparison against a family or CLI name (the `cli-provider` source audit stays green); table test (registered family, unregistered family, `generic`)
- [ ] 1.2 Move the "Per-CLI mechanism" table body out of the coordination `SKILL.md` into `claude/skill.md` (`/compact`, `/clear`) and `generic/skill.md` (look for `/compact`, `/save`, `/reset`); `{{CLI_NOTES}}` in its place
- [ ] 1.3 Move the boot-block paste paragraph into `claude/boot.md` (`[Pasted text #N]` + second Enter) and `generic/boot.md` (vendor-neutral + second Enter); `{{CLI_BOOT_NOTES}}` in both boot-block forms
- [ ] 1.4 `render` / `render_for_agent_id` take the agent's `CliFamily`; `build_boot_block(branch_id, broker_url, &CliProfile)` (and `boot_block_for_id`) read the form from `CliProfile::mcp_capable()` and the fragment from `CliProfile::family()` (no `mcp_capable` flag, no CLI name); add both placeholders to the known set; repository-root render and MCP `get_skill` pass `CliFamily::Generic`; update `tests/boot_block_bare_helper_calls.rs` and the boot-block tests in `src/skills.rs` / `src/commands/launch.rs` to build profiles with `CliProfile::for_family(..)`; tests "Function accepts required parameters", "Function returns boot block string", "The generic profile yields the helper form with the generic fragment", "Branch ID substitution", "Broker URL substitution"

## 2. Wire the profile

- [ ] 2.1 Pass the agent's resolved `CliProfile` / its `family()` at every per-agent render site: `src/commands/start.rs` (both `render` sites and both `build_boot_block` sites, which already hold a profile), `src/commands/launch.rs` (`attach_agent`'s `render`, `compose_agent_launch`'s `build_boot_block`), package provisioning from `skill-package-deploy`; custom variants resolve their declared family, an undeclared or unknown CLI ⇒ `generic`
- [ ] 2.2 Tests: "A Claude-family agent gets the Claude fragment", "An unrecognised CLI gets the generic fragment", "A custom variant inherits its declared family", "A family without a fragment falls back to generic", "Agents on different CLIs in one session get different fragments" (e2e over the sidecar), "A render with no agent uses generic"
- [ ] 2.3 Verify `codex` and `agy` compaction commands and paste display against each CLI's current documentation; author `codex/` / `agy/` fragments only for verified facts, otherwise ship none (they render `generic`) and record the outcome in the changelog

## 3. Requirement tests

- [ ] 3.1 Context-budget tests for *Compact mechanism for the agent's own CLI*: "Claude-family agent sees its slash commands", "Generic fallback points users at their CLI's equivalent", "No custom variant is named" — re-point `skills::tests::coordination_skill_per_cli_mechanism_table` and `tests/coordination_helper_only_skill_content.rs::coordination_skill_carries_no_git_paw_history_or_variant_names` (they pinned the removed *Per-CLI compact mechanism table*)
- [ ] 3.2 Boot-block tests for both forms: "Paste handling instruction included" (every family), "Paste instruction format" (Claude-family agent), "Paste instruction for a generic agent names no vendor"

## 4. Export guard

- [ ] 4.1 `tests/lang_agnostic_skill_audit.rs`: render per family + `generic`; vendor tokens allowed only from the family's own fragment; a package file under `assets/cli/<family>/` is read as that family's fragment by the per-file audit; regression tests "A fragment naming another vendor fails" and "A vendor name in a base template fails"; "Generic rendering is vendor-free"

## 5. Docs

- [ ] 5.1 `docs/src/user-guide/skill-templates.md`: CLI fragments, families, `generic` fallback, how custom variants get a family (`[clis.<name>].family`), overrides may use `{{CLI_NOTES}}`; placeholders table gains `{{CLI_NOTES}}` / `{{CLI_BOOT_NOTES}}`
- [ ] 5.2 `docs/src/user-guide/coordination.md` context-budget section and `docs/src/user-guide/agents-md.md` boot-block section reflect per-CLI rendering

## 6. Gates

- [ ] 6.1 Every scenario maps to a named test
- [ ] 6.2 `just check` (exit code read directly), `just deny`, `mdbook build docs/` green; full suite vs merge-base green
