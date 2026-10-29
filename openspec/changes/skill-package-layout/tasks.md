## 1. Registry over the current paths (seam first, no file moves)

- [ ] 1.1 Add `BundledPackage` / `BundledFile` / `ProvisionGate` and `skills::bundled_packages()` listing today's assets at their current paths (`assets/agent-skills/*.md` including v0.17.0's `cli-setup.md`, `assets/scripts/*.sh`, boot-block templates); doc comments on every public item
- [ ] 1.2 Derive `skills::embedded_default` from the registry; `skills::resolve` order unchanged; existing resolve tests green
- [ ] 1.3 Derive `init::bundled_scripts()`, the `run_init` install loop (same order, same "Created/Updated" lines) and `init::provision_worktree_helpers` from the registry
- [ ] 1.4 Add `skills::packages_for_worktree(inputs)` (pure); route provisioning and every per-command skill resolution in `commands/{start,add,supervisor}.rs` and `main.rs::attach_agent` through it while keeping each launch path's current docs-fetch behavior; table test for the five selection scenarios (`orchestrator` and `cli-setup` never selected)
- [ ] 1.5 Derive the `setup-cli` grant table's helper coverage guard (`src/cli_setup/facts.rs`) and the doctor bundled-scripts probe from the registry; existing setup-cli and doctor tests green unchanged; e2e test: every registered helper is installed by `init` and reported by `doctor` (scenario "A registered helper is installed by init and checked by doctor")
- [ ] 1.6 Behavior-preservation check: full suite green with no test edits beyond 1.x additions; record byte-identity of rendered skills/boot blocks before vs after (scenario "Rendering is byte-identical across the restructure")

## 2. Move assets into packages

- [ ] 2.1 `git mv` into `assets/agent-skills/{coordination,orchestrator,docs-fetch,cli-setup}/SKILL.md`, helpers into each package's `scripts/`, boot-block templates into `coordination/assets/`, `_paw_common.sh` into `assets/agent-skills/_shared/scripts/`; update only the registry's `include_str!` paths
- [ ] 2.2 Registry conformance test: every file under `assets/agent-skills/<pkg>/` is registered and every registered file exists (scenario "An unregistered package file fails the suite")
- [ ] 2.3 Test: each package's `SKILL.md` loads through the standardized loader with non-empty name/description; each helper belongs to exactly one package; every helper-shipping package lists a byte-identical `_paw_common.sh`; `cli-setup` is registered with no helper
- [ ] 2.4 Test: binary renders every bundled skill with no `assets/` on disk (compile-time embed)

## 3. Override boundary

- [ ] 3.1 Ensure installation/provisioning always writes registry content, ignoring an override directory's `scripts/`/`references/`/`assets/`; test with a `.agents/skills/coordination/scripts/broker.sh` decoy (scenario "An override's script is not installed")

## 4. Tests stop pinning source paths

- [ ] 4.1 `tests/helpers`: install-a-package-into-a-tempdir (scripts + sibling preamble) and read-a-package-file accessors over `bundled_packages()`
- [ ] 4.2 Re-point every `include_str!("../assets/scripts/…")` / `include_str!("../assets/agent-skills/…")` and every `assets/scripts` tempdir copy in `tests/` and `src/**` test modules to the accessors (sweep_sh_*, broker_sh_*, docs_fetch_helper, paw_common_conventions, cli_init_writes_*, *_skill_content, lang_agnostic_skill_audit, boot_block_integration, …)
- [ ] 4.3 `tests/paw_common_conventions.rs`: assert sibling sourcing relative to own location and add "A package installed into an arbitrary directory runs" + "The preamble has one source"

## 5. Export guard

- [ ] 5.1 `tests/lang_agnostic_skill_audit.rs` enumerates registered package text files (SKILL.md, references, assets — rendered for every backend where templated) instead of a hand list; test that the audited set equals the registry's text files; regression test with a synthetic reference containing a forbidden token

## 6. Docs

- [ ] 6.1 `docs/src/architecture.md` asset-layout section; `docs/src/user-guide/skill-templates.md` "bundled skills are packages" note (layout only — no behavior change); `.agents/skills/export-agnosticism/SKILL.md` inventory re-pointed to `assets/agent-skills/<pkg>/…` (`tests/agent_skills_conform.rs` green)
- [ ] 6.2 Record, in the change's archive note, the requirements that still name pre-package paths (approval-command-safety, boot-block, broker-agent-helper, broker-conflict-detection, core-ci-hygiene, core-lang-agnostic, git-hook-injection, mcp-agent-docs, session-state, skill-agent, supervisor-learnings, supervisor-skill-discipline, supervisor-unattended-operation) for the v1.0.0 pre-freeze wording sweep
- [ ] 6.3 At archive, confirm `openspec/specs/skill-packages/spec.md` carries the real `## Purpose` and the Specifications page lists the capability (`tests/spec_purpose_backfilled.rs`, `tests/specifications_page_lists_every_capability.rs`)

## 7. Gates

- [ ] 7.1 Every scenario in this change maps to a named test
- [ ] 7.2 `just check` (exit code read directly, not through a pipe), `just deny`, `mdbook build docs/` green; full suite vs merge-base green
