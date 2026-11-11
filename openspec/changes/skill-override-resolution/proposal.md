## Why

User-level skill overrides have not worked since v0.4.0. `skills::resolve(name)` calls
`resolve_with_config_dir(name, None)`, and `try_load_standardized_skill` consults
`<config_dir>/git-paw/agent-skills/<name>/SKILL.md` only when that argument is `Some`, so every
production caller (`commands/start.rs`, `commands/add.rs`, `commands/supervisor.rs`,
`init::export_skills`, MCP `get_skill`) gets only the repository `.agents/skills/` lookup and the
embedded default. The
repository lookup walks up to five directories from the process's current directory, so it can
miss the repository root from a deep subdirectory, can read a directory *above* the repository,
and ignores `get_skill`'s `--repo` root. The spec, the docs and the code each describe a different
order (listed in design.md). v0.20.0 makes skills packages, and `skill-package-layout` states the
override boundary; this change makes the overrides themselves real, with one order.

## What Changes

- **One resolution order:** the repository's `.agents/skills/<name>/SKILL.md` → the user's
  `<config_dir>/git-paw/agent-skills/<name>/SKILL.md` (`crate::dirs::config_dir()`) → the
  embedded default. First match wins. Directory form only; the v0.3 flat `<name>.md` file is not
  read.
- **Explicit roots, no cwd walk.** The repository level is read from the repository's main
  working tree only, never from a linked worktree (so never from an agent worktree) and never
  from a directory above the repository. Production callers resolve through
  `skills::resolve(name, repo_root)`, which supplies the real config directory;
  `resolve_with_config_dir` is removed, and the explicit-roots form `resolve_from` is reserved for
  `skills.rs` and tests (a guard test enforces it).
- **Overrides change text only.** Override `scripts/`, `references/` and `assets/` are ignored (as
  `skill-package-layout` states). Since v0.17.0 `classifier-retirement` git-paw makes no permission
  decision, so an override changes instructions (as `AGENTS.md` does), never whether a command
  runs; an agent's write into an override location prompts like any other write, and under
  `--unattended` is escalated.
- **`cli-setup` is not overridable.** The bundled `cli-setup` skill (v0.17.0 `cli-setup-consent`)
  carries the consent rules for writing an agent CLI's permission settings, so it always resolves
  from the embedded copy. An override directory for it at either level is ignored, and
  `git paw doctor` warns that it is.
- **Visibility:** MCP `get_skill` reports the source the production resolver chose
  (`standard | user_override | embedded`), now including `user_override`; `git paw doctor` prints
  one line naming the source of each bundled skill, ✗ when an override fails to load, ⚠ when a
  `cli-setup` override is present and ignored.
- **Behavior change (not breaking for defaults):** a user override placed under the documented
  directory now takes effect; a stray `.agents/skills/` above the repository no longer does.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `skill-agent`: *Skill resolution order*, *User override directory may be absent*, *Unreadable
  user override is a hard error* (directory form, three levels, explicit roots, `cli-setup` never
  overridden), *SkillTemplate value type* (its scenario's call takes the repository root);
  ADDED: *The cli-setup skill is not overridable*; production callers resolve through the real
  search roots. *Embedded cli-setup skill*
  still names the one-argument `skills::resolve("cli-setup")` informally; it is read as the
  two-argument call and left for the pre-freeze wording sweep.
- `mcp-server`: ADDED — `get_skill` reports the production resolver's source.
- `core-ci-hygiene`: ADDED — binary tests also remove `XDG_CONFIG_HOME`, enforced by the standing
  convention test.
- `preflight-diagnostics`: ADDED — doctor reports each bundled skill's source and warns on an
  ignored `cli-setup` override.

## Impact

- **Code:** `src/skills.rs` (`SkillRoots`, `resolve(name, repo_root)`, `resolve_from`, remove
  `try_load_from_agents_dir`'s cwd walk and `resolve_with_config_dir`), `src/git.rs`
  (main-working-tree root lookup), call sites in `src/commands/{start,add,supervisor}.rs`,
  `src/init.rs` (`export_skills`, which v0.18.0 added for the `.git-paw/skills/` export and which
  `run_init` and the supervised launch call), `src/mcp/tools/project.rs`, `src/doctor.rs` (probe +
  one check). Every new call site `skill-package-deploy` adds uses the same entry point.
- **Tests:** ~150 in-module `resolve("…")` calls in `src/skills.rs` (11 of its tests call
  `set_current_dir`), the `resolve` calls in the test modules of `src/specs/backend_pin_tests.rs`
  and `src/mcp/tools/project.rs`, and `tests/{lang_agnostic_skill_audit,governance_supervisor_boot,
  agent_skills_conform,orchestrator_skill_content,orchestrator_budget_skill_content,
  orchestrator_skill_alias,cli_init_orchestrator_export}.rs` move to explicit roots (and stop
  `set_current_dir`; `orchestrator_skill_alias.rs` has a local cwd-switching helper already named
  `resolve_from`, renamed or replaced by the real one); new unit and e2e (`start`, `mcp`, `doctor`)
  tests, including the ignored `cli-setup` override; `tests/mcp_e2e.rs` (`McpClient::spawn`, which
  sets no `HOME` today) gains the HOME/`XDG_CONFIG_HOME` override.
- **Struct-field ripple (GP-40):** `SkillTemplate` gains `origin_path`, breaking every
  `SkillTemplate {` literal: 2 in production (`src/skills.rs` `load_skill_from_directory` and the
  embedded branch of the resolver) and 24 in `src/skills.rs` tests; none under `tests/`. If the
  doctor probe becomes a new `doctor::Probes` field, its exhaustive literals are
  `src/doctor.rs::collect_probes` and `src/doctor_tests.rs::every_non_ok_check_carries_a_remedy`
  (every other `Probes {` literal spreads `..Probes::default()`).
- **Docs:** `docs/src/user-guide/skill-templates.md` (resolution, user location, trust, removal
  of the contradicting migration step), `docs/src/configuration/README.md`,
  `docs/src/user-guide/{coordination,mcp,doctor,security-posture}.md`, `docs/src/architecture.md`,
  `src/skills.rs` module docs. README does not mention overrides; unchanged.
- **Security / safety:** narrows (repository level bound to the main working tree; `cli-setup`
  never overridden, so no repository or user file can rewrite its consent rules); no new grant.
  Reviewed with `security-and-safety-review`.
- **Enum-variant ripple:** none (`Source` keeps its three variants; `BrokerMessage`,
  `SpecBackendKind` untouched).
- **Dependencies:** none.
- **Order within v0.20.0:** after `skill-package-layout` (behavior-preserving; supplies the
  registry the doctor line iterates), before `skill-package-deploy` (its new install/provision call
  sites then render override-aware text from the start); independent of `per-cli-skill-templates`
  and `orchestrator-gate-references`.
- **Cross-version dependencies:**
  - v0.18.0 `orchestrator-skill-export` (shipped) restated *Skill resolution order* in the main
    spec (the `supervisor` → `orchestrator` alias; the repository `.agents/skills/` directory form
    read from the current directory and up to four ancestors, then the embedded default). This
    change MODIFIES that main-spec text and keeps the alias. It also added `init::export_skills`,
    a production resolve site this change re-points.
  - v0.18.0 `cmd-library-surface` (shipped) moved the launch call sites into `src/commands/`; the
    requirement is per call site, not per file.
  - v0.17.0 `test-session-state-isolation` (shipped as `core-ci-hygiene` *Tests invoking git paw
    subcommands SHALL override HOME and XDG_DATA_HOME*) is the test-isolation base; these tests
    also remove `XDG_CONFIG_HOME`.
  - v0.17.0 `mcp-superpowers-support` (shipped) modified *Project knowledge tools*, which already
    states the `.agents/skills/` → user override → embedded order this change makes real; this
    change ADDs a separate `get_skill` requirement instead of modifying it.
  - v0.17.0 `classifier-retirement` removes the classifier and the protected-path set
    (`core-memory-isolation` *Protected-path set is config-driven*), so this change adds neither a
    classification requirement nor protected paths. v0.17.0 `cli-setup-consent` adds the
    `cli-setup` skill; its *Embedded cli-setup skill* requirement resolves it "under the standard
    resolution order", which after this change always yields the embedded copy, so that
    requirement needs no delta.
  - v0.20.0 `skill-package-layout` (registry, override boundary, `cli-setup` registered as a
    root-only package) is required.
