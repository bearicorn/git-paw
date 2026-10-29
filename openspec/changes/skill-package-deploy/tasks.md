## 1. Install packages (additive — old paths still written)

- [ ] 1.1 `skills`: render a package for a location (SKILL.md + references with the location's inputs; scripts verbatim; assets skipped); `init`: shared writer installs `.git-paw/skills/<pkg>/` (scripts `0o755`) plus the `.git-paw/scripts/<helper>` compatibility copies from the same constants
- [ ] 1.2 `run_init` installs every registered package at the root and reports per package created/updated; `.git-paw/skills/` in `required_gitignore_entries()` (no-op if v0.18.0 already added it); tests: "Init installs every package at the root", "Re-install refreshes a stale package", "Package directory is gitignored", "The compatibility copy matches the package copy", "Installing packages leaves setup grants untouched"
- [ ] 1.3 Worktree provisioning (`worktree_provision::provision_worktree`, `main.rs::attach_agent`) installs exactly `packages_for_worktree(...)` before the boot prompt; e2e tests over bare `start`, spec-driven `start`, supervisor `start` and `add`: "A worktree receives exactly its selected packages", "The worktree's SKILL.md is rendered for that agent", and the five `broker-agent-helper` provisioning scenarios
- [ ] 1.4 Test "An invocation of the old path still works" (`.git-paw/scripts/broker.sh --help` in a provisioned worktree exits 0)

## 2. One selection for provisioning and injection

- [ ] 2.1 `commands/start.rs` (bare + spec-driven), `commands/supervisor.rs`, `commands/add.rs`: resolve and inject skill text for exactly the selected packages (the `start` paths now inject `docs-fetch` when `docs_base_url` is set); test "docs-fetch package provisioned only when configured, on every launch path"; changelog-visible behavior note
- [ ] 2.2 `WorktreeAssignment` carries the package list; `agents::generate_worktree_section` renders the generated `## Skills` index before the inlined bodies; tests: "Index lists the coordination package", "Index follows the selection", "No packages, no index"

## 3. Placeholders replace literal helper paths

- [ ] 3.1 Add `{{BROKER_HELPER}}`, `{{SWEEP_HELPER}}`, `{{DOCS_FETCH_HELPER}}` (values derived from the registry) to `render` and `build_boot_block`, and to the known-placeholder set; test "Helper placeholders raise no warning"
- [ ] 3.2 Replace every literal `.git-paw/scripts/<helper>` in registered templates (coordination, orchestrator, docs-fetch SKILL.md, both boot-block forms, any reference) with the placeholder; guard test "No bundled template carries a literal helper path"
- [ ] 3.3 `drive::SweepLearningSink` invokes the orchestrator package path; test "The drive loop records learnings through the package helper"
- [ ] 3.4 Re-point skill-content and boot-block tests (`src/skills.rs` tests, `tests/*_skill_content.rs`, `tests/boot_block_integration.rs`, `tests/skill_stuck_shapes_prose.rs`, …) from the literal `.git-paw/scripts/…` to the rendered package path; boot-block tests: "Each event invokes the broker helper" (helper form), "Boot block contains no raw broker curl" (both forms), "The boot-block template carries no literal helper path", "DONE section retains the manual done fallback"
- [ ] 3.5 Test for the legacy-path mapping requirement: a content requirement naming `.git-paw/scripts/sweep.sh` is asserted against the rendered package path

## 4. `git paw setup-cli` at the package paths (security)

- [ ] 4.1 Fact gate first (blocking for the verified status): re-run `cli-setup-consent`'s scripted gate with the package-path allow and ask entries and both deny rules — re-point F2–F4 to `.git-paw/skills/<pkg>/scripts/…`, extend F5/F6 to edits of `.git-paw/skills/coordination/scripts/broker.sh`, its `_paw_common.sh`, a nested worktree's package copy and a package `SKILL.md`; record the observations in design.md. If it is not re-run or a blocking step fails, set the `claude` row `verified = false` (plan-only) and say so in the docs
- [ ] 4.2 `src/cli_setup/facts.rs`: grant table derived from the registry's package invocation paths (twelve allow entries, none at `.git-paw/scripts/`); `ask` rules for `sweep.sh approve` / `tell` at the package and compatibility paths; protection `Edit(**/.git-paw/skills/**)` + `Edit(**/.git-paw/scripts/**)` (deny) and `Edit(**/.git-paw/config.toml)` (ask); guard tests for every scenario of the modified *Allowlist entries are path-scoped…* and *The allowlist item guards…* requirements, plus the plan and writes scenario literals
- [ ] 4.3 `src/cli_setup/undo.rs`: the shape check accepts `Bash(.git-paw/skills/<pkg>/scripts/<name>…)` and the old form; protection removed only when no allow entry under either root remains; tests "Entries recorded before the package move are undone" and "Package-path entries are undone" plus the existing undo scenarios
- [ ] 4.4 Doctor setup check: the modified-helper warning follows the paths the applied allow entries name (package or compatibility); a pre-package record is ⚠ out of date with the `--undo` then `--allowlist` remedy; tests "Entries applied before the package move are out of date" and "A granted helper was modified" (package path); assert the check leaves the settings file and record byte-identical
- [ ] 4.5 Security-and-safety review recorded: no new grant (same subcommands, new paths; compatibility path loses its allow entries), protection widened to the package directory, git-paw never rewrites a recorded grant

## 5. Doctor

- [ ] 5.1 `doctor::check_bundled_scripts` iterates packages: `SKILL.md` present (✗), helpers present/executable/matching (✗/✗/⚠ as today), compatibility copies (⚠ only), python3 unchanged; e2e tests for the five scenarios

## 6. Docs

- [ ] 6.1 Re-point the 45 `.git-paw/scripts/` mentions across `docs/src/{architecture,cli-reference}.md`, `docs/src/configuration/README.md`, `docs/src/user-guide/{agents-md,coordination,docs-fetch,doctor,learnings,mcp,skill-templates,supervisor}.md` to the package paths; `skill-templates.md` gains "Installed packages" (layout, rendering, compatibility copy, deprecation)
- [ ] 6.2 `docs/src/user-guide/cli-setup.md` and the security-posture chapter: the grant table and protection rules at the package paths, the ask rules at both paths, the verification status after the re-run gate, and the residual risk of durable grants on `.git-paw/scripts/…`
- [ ] 6.3 Upgrade note (changelog body + `skill-templates.md` + `cli-setup.md`): after upgrading, `git paw doctor` reports setup out of date; run `git paw setup-cli claude --undo` then `--allowlist` (git-paw never rewrites grants itself); hand-written CLI allow rules keyed on `.git-paw/scripts/…` should name `.git-paw/skills/<pkg>/scripts/…`; `.git-paw/scripts/` is deprecated; `start` now injects docs-fetch when configured
- [ ] 6.4 `git paw init --help` / `long_about` mention `.git-paw/skills/`; export-agnosticism dev-skill inventory lists the installed-package surface and the helper placeholders

## 7. Gates

- [ ] 7.1 Every scenario maps to a named test
- [ ] 7.2 `just check` (exit code read directly), `just deny`, `mdbook build docs/` green; full suite vs merge-base green; e2e under a cold (no live session) environment
