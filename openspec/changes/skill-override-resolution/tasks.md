## 1. Resolver seam (functional core first)

- [ ] 1.1 `git::main_worktree_root(path) -> Option<PathBuf>`: a pure `parse_main_worktree(porcelain)` (first `worktree` entry; `None` when it is `bare` or the listing is empty or garbled) plus a shell over `git worktree list --porcelain` through the existing `CommandRunner` seam; table test for main, linked-worktree listing, bare, and garbled input
- [ ] 1.2 `skills::SkillRoots` and `skills::resolve_from(name, &SkillRoots)`: repository → user → embedded, directory form only, `orchestrator`/`supervisor` alias with level-before-name precedence, the D5 failure semantics, no cwd/env/git access; `SkillTemplate.origin_path` (`#[serde(default, skip_serializing_if = "Option::is_none")]`); doc comments on every public item
- [ ] 1.3 `skills::resolve(name, repo_root)` as the shell (`git::main_worktree_root` + `crate::dirs::config_dir()`); delete `resolve_with_config_dir`, `try_load_standardized_skill`'s optional-config branch and `try_load_from_agents_dir`'s cwd walk; rewrite the module doc and `resolve`'s doc comment to the one order
- [ ] 1.4 Unit tests over tempdir roots, one per `skill-agent` scenario: embedded fallback; user over embedded; repository over user; linked worktree ignored (real `git init` + `git worktree add` in a tempdir, through `resolve`); cwd independence (ancestor `.agents/skills/`); installed `.git-paw/skills/` not read; legacy flat file ignored; invalid repository override is a hard error with a valid user override present; alias; legacy `supervisor/` override; level beats name; unknown skill; no config dir; missing `agent-skills/`; directory without `SKILL.md`; permission denied; invalid UTF-8; `cli-setup` overrides at both levels ignored (embedded returned); a broken `cli-setup` override does not fail resolution

## 2. Production call sites and test migration

- [ ] 2.1 Route every production resolution through `skills::resolve(name, &repo_root)`: `commands/start.rs` (bare and `launch_spec_session`), `commands/add.rs`, `commands/supervisor.rs`, `mcp/tools/project.rs::get_skill` (using `self.ctx.root`, not the process cwd), and every site `skill-package-layout` / `skill-package-deploy` / v0.18.0 `orchestrator-skill-export` added (worktree package composition, init install, worktree provisioning, `attach_agent`, orchestrator export)
- [ ] 2.2 Source-audit guard: a pure check over `(path, contents)` that flags any `src/` file other than `src/skills.rs` naming `resolve_from` or constructing `SkillRoots` outside `#[cfg(test)]`, a test that runs it over `src/`, and a table case with a synthetic offender (scenario "A bypass of the production entry point fails the suite")
- [ ] 2.3 Migrate in-process tests to explicit roots: the `resolve("…")` calls in `src/skills.rs` tests go through a test helper over `resolve_from` with empty roots, and every `std::env::set_current_dir` there is removed; `tests/lang_agnostic_skill_audit.rs` and `tests/governance_supervisor_boot.rs` likewise; `tests/agent_skills_conform.rs` resolves with the repository root `CARGO_MANIFEST_DIR` (via `resolve_from`), so it validates the checkout under test even when that checkout is a linked worktree
- [ ] 2.4 Binary-test isolation: `tests/mcp_e2e.rs`'s server spawn, and any other binary test that asserts embedded skill text or `source`, set `HOME` to a tempdir and remove `XDG_DATA_HOME` and `XDG_CONFIG_HOME`
- [ ] 2.5 E2E: a user override under a tempdir `HOME` appears in the managed `AGENTS.md` block of an agent launched by `git paw start`, and of one added by `git paw add` (scenarios "A user override reaches the agent launched by start" / "…added by add")

## 3. Safety

- [ ] 3.1 `resolve_from` returns the embedded `cli-setup` without consulting either level (a non-overridable set with one entry, `cli-setup`, in `src/skills.rs`); MCP `get_skill("cli-setup")` reports `embedded` with a repository override present (e2e)
- [ ] 3.2 Run the `security-and-safety-review` checklist against the diff (no new grant; repository level narrowed to the main working tree; `cli-setup` never overridden; override scripts never installed) and record the outcome in the PR description

## 4. Visibility

- [ ] 4.1 MCP e2e (`tests/mcp_e2e.rs`): `user_override` from a tempdir `HOME`; `--repo` from an outside cwd reports `standard`; a server rooted in a linked worktree with a worktree-only override reports `embedded`; no override reports `embedded` and writes nothing
- [ ] 4.2 Doctor: a `SkillSourcesProbe` collected with `skills::resolve` over the registry's packages, plus the existence of a `cli-setup` override directory at either level; a pure `check_skill_sources(&probe) -> CheckResult` added to `run_checks` beside the package checks; unit table (all embedded ✓ naming every package; user override ✓ naming `user` and the path; ignored `cli-setup` override ⚠ naming the directory; broken override ✗ naming package, path and remedy); e2e `git paw doctor --json` contains `skill sources`; the probe writes nothing

## 5. Docs (no `--help` change: no CLI surface changes)

- [ ] 5.1 `docs/src/user-guide/skill-templates.md`: rewrite *Skill Resolution* (three levels, main working tree only, user path per platform, directory form, `orchestrator`/`supervisor` alias, hard-error semantics, bundled names a repository override replaces); add a short *What an override cannot do* (scripts/references/assets ignored, no effect on any permission decision, `cli-setup` cannot be overridden and an override of it is ignored with a doctor warning, linked worktrees and `.git-paw/skills/` never read); replace the *Migration* step that sends users away from the user directory with "move a flat `<name>.md` into `<name>/SKILL.md` with frontmatter"
- [ ] 5.2 `docs/src/configuration/README.md`: the user skill-override directory next to *Config File Locations* (Linux / `XDG_CONFIG_HOME` / macOS paths), linking to the skill-templates chapter
- [ ] 5.3 `docs/src/user-guide/coordination.md` (the flat-file sentence → the directory form and the order), `mcp.md` (`source` semantics; `--repo` and worktree behavior), `doctor.md` (*What it checks*: the `skill sources` line), `security-posture.md` (an override changes instructions only; `cli-setup` is not overridable; the full-bypass caveat), `docs/src/architecture.md` (Skills row), `docs/src/specifications/README.md` (`skill-agent` index line's order)
- [ ] 5.4 Confirm README.md still does not describe override resolution (no change needed); `mdbook build docs/` green
- [ ] 5.5 At archive: update `skill-agent`'s `## Purpose` ("user override then embedded default") to the three-level order, and the `core-error-handling` *SkillError variants* `UserOverrideRead` example path to the directory form (`tests/spec_purpose_backfilled.rs` green)
- [ ] 5.6 Commit message notes the user-visible change for the git-cliff changelog: user overrides now take effect; the cwd walk-up and the above-the-repository lookup are gone

## 6. Gates

- [ ] 6.1 Every scenario in this change maps to a named test (tasks 1.4, 2.2, 2.5, 3.1, 4.1, 4.2)
- [ ] 6.2 `just check` (exit code read directly, not through a pipe), `just deny`, `mdbook build docs/` green; full suite against the merge-base green
