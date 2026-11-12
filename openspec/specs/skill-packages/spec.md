# skill-packages Specification

## Purpose
Defines how git-paw's bundled skills are packaged and delivered: each bundled skill is an agentskills.io package (a `SKILL.md` with frontmatter plus the `scripts/`, `references/` and `assets/` it owns), a compiled registry is the single source of the bundled set for every consumer (skill resolution, init, worktree provisioning, the root-only `.git-paw/skills/` export, doctor, the `git paw setup-cli` grant table's helper coverage, the no-leak audit), which packages reach an agent worktree is a pure decision, overrides change skill text but never the helpers git-paw installs, and requirements that still name pre-package asset paths are read against the package files.

## Requirements

### Requirement: Bundled skills ship as agentskills.io packages

The system SHALL ship every bundled skill as an agentskills.io package: a directory `assets/agent-skills/<name>/` holding a `SKILL.md` with the required `name` and `description` frontmatter, plus its own optional `scripts/`, `references/` and `assets/`. Each bundled helper SHALL belong to exactly one package, `cli-setup` SHALL ship no helper, and `_paw_common.sh` SHALL have one source file listed in every helper-shipping package. Every package SHALL load through the standardized-skill loader.

#### Scenario: Every bundled package validates as a standardized skill

- **WHEN** each registered bundled package's `SKILL.md` is loaded through the standardized-skill loader
- **THEN** loading SHALL succeed
- **AND** the parsed metadata SHALL carry a non-empty `name` and `description`

#### Scenario: Each helper belongs to one package

- **WHEN** the registered packages' scripts are enumerated
- **THEN** `broker.sh` SHALL appear only in `coordination`, `sweep.sh` only in `orchestrator`, and `docs-fetch.sh` only in `docs-fetch`
- **AND** both boot-block templates SHALL appear only in `coordination`

#### Scenario: The cli-setup skill is a registered package without helpers

- **WHEN** the registered packages are enumerated
- **THEN** a `cli-setup` package SHALL be registered
- **AND** it SHALL list no helper script

#### Scenario: Every helper-shipping package carries the shared preamble

- **WHEN** a registered package lists any helper script
- **THEN** its scripts SHALL also include `_paw_common.sh`
- **AND** that content SHALL be byte-identical across all packages

### Requirement: The package registry is the single source of the bundled skill set

The system SHALL expose a compile-time registry of every bundled package (`SKILL.md`, scripts, references, assets, provisioning gate). Embedded resolution, the helpers `git paw init` installs, the worktree-provisioned set, the `.git-paw/skills/` exports, the doctor bundled-scripts check and the `git paw setup-cli` grant table's helper set SHALL each derive from it. A package file not registered, or registered but missing, SHALL fail the suite.

#### Scenario: Embedded resolution comes from the registry

- **GIVEN** no override exists for a registered package name
- **WHEN** `skills::resolve(<name>)` is called
- **THEN** it SHALL return `Source::Embedded` with the body of that package's registered `SKILL.md`

#### Scenario: A registered helper is installed by init and checked by doctor

- **GIVEN** a registered package that lists a helper script
- **WHEN** `git paw init` runs and then `git paw doctor` runs
- **THEN** init SHALL have installed that helper
- **AND** the doctor bundled-scripts group SHALL report a result for it

#### Scenario: The root-only skill exports come from the registry

- **WHEN** `git paw init` runs in a fresh repository
- **THEN** `.git-paw/skills/` SHALL hold a `SKILL.md` for exactly the registered root-only packages, which include `orchestrator` and `cli-setup`

#### Scenario: An unregistered package file fails the suite

- **GIVEN** a file added under `assets/agent-skills/<name>/` without a registry entry
- **WHEN** the registry conformance test runs
- **THEN** it SHALL fail naming the unregistered file

#### Scenario: The binary renders bundled skills with no assets on disk

- **WHEN** the binary resolves and renders a bundled skill with no `assets/` directory present
- **THEN** rendering SHALL succeed from the compile-time embedded copies

### Requirement: Worktree package selection is a pure decision

The system SHALL decide which bundled packages an agent worktree receives with a pure function of whether the broker is enabled and whether `docs_base_url` is explicitly configured, performing no I/O. It SHALL select `coordination` when the broker is enabled and `docs-fetch` when `docs_base_url` is configured, in that order, and never the root-only `orchestrator` and `cli-setup`. Code that writes package files into a worktree SHALL write exactly the selected set.

#### Scenario: Broker enabled, docs unset

- **WHEN** the selection is computed with the broker enabled and `docs_base_url` unset
- **THEN** it SHALL be exactly `[coordination]`

#### Scenario: Broker enabled, docs configured

- **WHEN** the selection is computed with the broker enabled and `docs_base_url` configured
- **THEN** it SHALL be exactly `[coordination, docs-fetch]`

#### Scenario: Nothing enabled

- **WHEN** the selection is computed with the broker disabled and `docs_base_url` unset
- **THEN** it SHALL be empty

#### Scenario: The orchestrator package never reaches a worktree

- **WHEN** the selection is computed for any combination of inputs
- **THEN** it SHALL NOT contain `orchestrator`

#### Scenario: The cli-setup package never reaches a worktree

- **WHEN** the selection is computed for any combination of inputs
- **THEN** it SHALL NOT contain `cli-setup`

### Requirement: Skill overrides replace skill text, never helper scripts

The system SHALL apply a skill override (a `.agents/skills/<name>/` directory or a user config-directory override) only to the skill text that is rendered. Helper scripts, references and assets carried by an override directory SHALL NOT be installed or provisioned; every helper the system writes SHALL be the binary's embedded copy from the registry.

#### Scenario: An override's script is not installed

- **GIVEN** a `.agents/skills/coordination/` override whose `scripts/broker.sh` differs from the embedded helper
- **WHEN** an agent worktree is provisioned
- **THEN** the provisioned `broker.sh` SHALL be byte-identical to the registry's embedded copy
- **AND** the override's skill text SHALL still be what is rendered for that agent

### Requirement: Requirements naming pre-package asset paths refer to package files

The system SHALL read a requirement that names a bundled asset by its pre-package source path as naming the corresponding package file, with its content constraints unchanged: `assets/agent-skills/<name>.md` maps to `assets/agent-skills/<name>/SKILL.md` (`supervisor.md` to `orchestrator/SKILL.md`), each `assets/scripts/<helper>` to its owning package's `scripts/`, and the boot-block templates to `coordination/assets/`, per the scenarios below.

#### Scenario: Each pre-package path maps to one package file

- **WHEN** a requirement names one of the pre-package paths below
- **THEN** `assets/agent-skills/coordination.md` SHALL mean `assets/agent-skills/coordination/SKILL.md`
- **AND** `assets/agent-skills/supervisor.md` or `assets/agent-skills/orchestrator.md` SHALL mean `assets/agent-skills/orchestrator/SKILL.md`
- **AND** `assets/agent-skills/docs-fetch.md` SHALL mean `assets/agent-skills/docs-fetch/SKILL.md`
- **AND** `assets/agent-skills/cli-setup.md` SHALL mean `assets/agent-skills/cli-setup/SKILL.md`
- **AND** `assets/scripts/broker.sh` SHALL mean `assets/agent-skills/coordination/scripts/broker.sh`
- **AND** `assets/scripts/sweep.sh` SHALL mean `assets/agent-skills/orchestrator/scripts/sweep.sh`
- **AND** `assets/scripts/docs-fetch.sh` SHALL mean `assets/agent-skills/docs-fetch/scripts/docs-fetch.sh`
- **AND** `assets/scripts/_paw_common.sh` SHALL mean `assets/agent-skills/_shared/scripts/_paw_common.sh`
- **AND** `assets/boot-block-template.md` SHALL mean `assets/agent-skills/coordination/assets/boot-block.md`
- **AND** `assets/boot-block-mcp-template.md` SHALL mean `assets/agent-skills/coordination/assets/boot-block-mcp.md`

#### Scenario: A content requirement on a pre-package path is checked against the package file

- **GIVEN** a requirement that the bundled `assets/scripts/sweep.sh` contain a given subcommand
- **WHEN** its covering test runs after packaging
- **THEN** the test SHALL assert against the orchestrator package's `scripts/sweep.sh`
- **AND** the constraint SHALL hold unchanged

### Requirement: Packaging preserves deployed output

Restructuring bundled skills into packages SHALL NOT change any file the system writes into a repository or worktree, nor any rendered text. Under identical inputs, rendered skills, boot blocks and sidecars SHALL be byte-identical to their pre-package rendering, and `git paw init` and worktree provisioning SHALL write the same set of files, with identical content and modes, at the same paths.

#### Scenario: Rendering is byte-identical across the restructure

- **GIVEN** a fixed branch, broker URL, project, gate-command set and backend set
- **WHEN** each bundled skill and both boot-block forms are rendered before and after packaging
- **THEN** the outputs SHALL be byte-identical

#### Scenario: Init writes the same files

- **WHEN** `git paw init` runs in a fresh repository after packaging
- **THEN** the files it writes under `.git-paw/scripts/` SHALL have the same names, content and executable mode as before packaging
- **AND** the skills it exports under `.git-paw/skills/` SHALL have the same paths and content as before packaging

### Requirement: Bundled packages are installed under `.git-paw/skills/`

The system SHALL install a bundled package as the directory `.git-paw/skills/<name>/` holding the package's `SKILL.md` rendered for that location, its `scripts/` with executable mode `0o755` on Unix, and its `references/` rendered like `SKILL.md`. A package's `assets/` SHALL NOT be installed; they are render-time inputs. `.git-paw/skills/` SHALL be among the `.gitignore` entries `git paw init` guarantees.

#### Scenario: The worktree's SKILL.md is rendered for that agent

- **WHEN** an agent worktree for branch `feat/x` is provisioned with the coordination package
- **THEN** its `.git-paw/skills/coordination/SKILL.md` SHALL contain `feat-x` where the skill's branch placeholder renders
- **AND** SHALL contain no unsubstituted known placeholder

Test: `init::tests::the_worktree_skill_md_is_rendered_for_that_agent`, `tests/worktree_helper_provisioning.rs::spec_driven_launch_provisions_broker_helper_into_each_worktree`

#### Scenario: Package directory is gitignored

- **WHEN** `git paw init` runs
- **THEN** `.gitignore` SHALL contain an entry ignoring `.git-paw/skills/`

Test: `tests/cli_init_installs_packages.rs::package_directory_is_gitignored`

### Requirement: Init installs every package at the repository root

`git paw init` SHALL install every registered package at the repository root, writing each package file that is missing or differs from the running binary's copy and reporting each such file as created or updated, as *Init reports updates only when content changed, and its closing line agrees* requires.

#### Scenario: Init installs every package at the root

- **WHEN** `git paw init` runs in a fresh repository
- **THEN** `.git-paw/skills/<name>/SKILL.md` SHALL exist for every registered package
- **AND** each package's helpers SHALL exist under `.git-paw/skills/<name>/scripts/` and be executable on Unix
- **AND** no package `assets/` directory SHALL be written

Test: `tests/cli_init_installs_packages.rs::init_installs_every_package_at_the_root`, `init::tests::the_root_refresh_installs_every_package`

#### Scenario: Re-install refreshes a stale package

- **GIVEN** a `.git-paw/skills/coordination/scripts/broker.sh` with stale content
- **WHEN** `git paw init` runs again
- **THEN** the file SHALL equal the embedded helper
- **AND** stdout SHALL contain `Updated .git-paw/skills/coordination/scripts/broker.sh`

Test: `tests/cli_init_installs_packages.rs::re_install_refreshes_a_stale_package`

### Requirement: Every start installs the bundled packages at the repository root

Every `git paw start` — bare, spec-driven or supervised — SHALL, before any agent pane is launched, install every registered package at the repository root with its helpers' compatibility copies, writing only files that are missing or differ from the running binary's copy. A fresh clone SHALL therefore get its root helpers without `git paw init`. This SHALL NOT write the config, `.gitignore` or `AGENTS.md`, which stay `git paw init`'s.

#### Scenario: A bare start after a fresh clone installs the root packages

- **GIVEN** a clone of an initialized repository whose `.git-paw/config.toml` is committed and which has no `.git-paw/skills/` and no `.git-paw/scripts/`
- **WHEN** `git paw start --branches a` is run without `git paw init`
- **THEN** `.git-paw/skills/orchestrator/scripts/sweep.sh` and `.git-paw/scripts/sweep.sh` SHALL exist and be executable before the agent's pane is launched
- **AND** `.git-paw/skills/<name>/SKILL.md` SHALL exist for every registered package

Test: `tests/e2e_start_root_packages.rs::a_bare_start_after_a_fresh_clone_installs_the_root_packages`

#### Scenario: A start leaves current root packages untouched

- **GIVEN** a repository whose root packages and compatibility copies already match the running binary
- **WHEN** `git paw start --branches a` is run
- **THEN** every root package file and compatibility copy SHALL keep its modification time

Test: `tests/e2e_start_root_packages.rs::a_start_leaves_current_root_packages_untouched`, `init::tests::refresh_root_scripts_leaves_current_scripts_untouched`

#### Scenario: A start does not take over init's files

- **GIVEN** a clone with no `.git-paw/skills/` and a committed `.gitignore` and `AGENTS.md`
- **WHEN** `git paw start --branches a` is run
- **THEN** `.gitignore`, `AGENTS.md` and `.git-paw/config.toml` SHALL be byte-identical afterwards

Test: `tests/e2e_start_root_packages.rs::a_start_does_not_take_over_inits_files`

### Requirement: The root package refresh renders from the current config

When `git paw start` refreshes the root packages, it SHALL compare each rendered file (`SKILL.md`, references) with a fresh render from the config as it is at that start, not with what `git paw init` rendered, and SHALL rewrite it when they differ, so a config change made after `git paw init`, such as a `[supervisor]` gate command, reaches the root packages on the next start.

#### Scenario: A gate command configured after init reaches the root skill on the next start

- **GIVEN** a repository where `git paw init` wrote `.git-paw/skills/orchestrator/SKILL.md` while no `[supervisor] test_command` was set, so the file renders it as `(not configured)`
- **AND** `[supervisor] test_command = "make test"` was then added to `.git-paw/config.toml`
- **WHEN** `git paw start --branches a` is run
- **THEN** `.git-paw/skills/orchestrator/SKILL.md` SHALL contain `make test` where the skill's test-command placeholder renders
- **AND** SHALL NOT render the test command as `(not configured)`

Test: `tests/e2e_start_root_packages.rs::a_gate_command_configured_after_init_reaches_the_root_skill`, `init::tests::the_root_refresh_renders_from_the_current_config`

### Requirement: Root packages render as the orchestrator

The system SHALL render the root packages' branch placeholder with the orchestrator's reserved agent id, never the current branch, so switching branches does not change the rendered root files.

#### Scenario: Root packages render the orchestrator's id whatever the branch

- **GIVEN** a repository whose main checkout is on branch `feat/one`
- **WHEN** the root packages are rendered, and rendered again after switching to `feat/two`
- **THEN** the root `coordination` `SKILL.md` SHALL render its branch placeholder as the orchestrator's reserved id `supervisor`
- **AND** SHALL NOT contain `feat-one`
- **AND** both renders SHALL be identical

Test: `init::tests::root_packages_render_the_supervisor_id_whatever_the_branch`

### Requirement: Every launch path installs the worktree's selected packages

Every launch path that creates or re-attaches an agent worktree — bare, spec-driven and supervisor-mode `git paw start`, and `git paw add` — SHALL install exactly the packages selected for that worktree into `<worktree>/.git-paw/skills/` before the agent's boot prompt is submitted.

#### Scenario: A worktree receives exactly its selected packages

- **GIVEN** a session with the broker enabled and `docs_base_url` configured
- **WHEN** any launch path sets up an agent worktree
- **THEN** `<worktree>/.git-paw/skills/coordination/` and `<worktree>/.git-paw/skills/docs-fetch/` SHALL exist before the agent's prompt is submitted
- **AND** `<worktree>/.git-paw/skills/orchestrator/` SHALL NOT exist

Test: `tests/worktree_helper_provisioning.rs::bare_launch_provisions_broker_helper_into_each_worktree`, `tests/worktree_helper_provisioning.rs::spec_driven_launch_provisions_broker_helper_into_each_worktree`, `tests/worktree_helper_provisioning.rs::supervisor_launch_provisions_broker_helper_into_each_worktree`, `tests/worktree_helper_provisioning.rs::add_provisions_broker_helper_into_the_new_worktree`

### Requirement: Installing packages never touches setup grants

Installing or provisioning packages SHALL NOT create, modify or delete any agent CLI settings file or the `git paw setup-cli` record. Grants recorded for the pre-package `.git-paw/scripts/` paths SHALL stay as they are until the user re-runs `git paw setup-cli`.

#### Scenario: Installing packages leaves setup grants untouched

- **GIVEN** a repository where an earlier version applied `git paw setup-cli claude --allowlist`
- **WHEN** `git paw init` installs the packages and `git paw start` provisions a worktree
- **THEN** the main checkout's `.claude/settings.local.json` and the setup record SHALL be byte-identical before and after

Test: `tests/e2e_start_root_packages.rs::installing_packages_leaves_setup_grants_untouched`, `tests/cli_init_installs_packages.rs::installing_packages_leaves_setup_grants_untouched`

### Requirement: Installed package files are replaced atomically

The system SHALL write each installed package file (`SKILL.md`, references, scripts) and each compatibility copy under `.git-paw/scripts/` by writing the content to a temporary file in the target's directory, setting its mode, and renaming it over the target, so a reader such as a running `sweep.sh` sees the old file or the new one, never a partial one. A symlink at the target SHALL still be refused.

#### Scenario: A reader of the old script is not cut off by a refresh

- **GIVEN** an installed `.git-paw/skills/orchestrator/scripts/sweep.sh` with stale content, opened for reading by another process
- **WHEN** the root packages are refreshed
- **THEN** the open handle SHALL still read the complete stale content
- **AND** the path SHALL then hold the complete bundled script, already executable

Test: `init::tests::a_reader_of_the_old_script_is_not_cut_off_by_a_refresh`

#### Scenario: No temporary file is left behind

- **WHEN** `git paw init` installs the packages and compatibility copies
- **THEN** each `.git-paw/skills/<name>/` and `.git-paw/scripts/` directory SHALL hold only the installed files

Test: `init::tests::no_temporary_file_is_left_behind`, `tests/cli_init_installs_packages.rs::no_temporary_file_is_left_behind`

#### Scenario: A symlink at an install target is refused

- **GIVEN** `.git-paw/scripts/broker.sh` is a symlink
- **WHEN** a worktree is provisioned or `git paw init` runs
- **THEN** the write SHALL fail naming that path
- **AND** the symlink's target SHALL be unchanged

Test: `init::tests::a_symlink_at_an_install_target_is_refused`, `init::tests::install_refuses_a_symlinked_skills_directory`

### Requirement: Installation supersedes the packaging identity pin

The system SHALL read *Packaging preserves deployed output* as governing the source-tree restructure only: from the installation layout of this capability onward, the files written into a repository or worktree, the helper bytes and the rendered skill text are those the installation requirements specify, and the identity scenarios of that requirement SHALL NOT be asserted against pre-package output.

#### Scenario: The identity pin does not constrain installed output

- **GIVEN** a pre-package export of `.git-paw/skills/orchestrator/SKILL.md` that named `.git-paw/scripts/sweep.sh`
- **WHEN** `git paw init` runs
- **THEN** the exported file SHALL name `.git-paw/skills/orchestrator/scripts/sweep.sh` and SHALL NOT be required to equal the pre-package content
- **AND** `.git-paw/skills/<name>/` SHALL exist for every registered package

Test: `tests/cli_init_installs_packages.rs::the_exported_orchestrator_skill_names_the_package_helper`, `tests/skill_package_registry.rs::init_writes_the_same_files`

### Requirement: Bundled prose names helpers by their package path

The system SHALL treat a helper's package path, `.git-paw/skills/<package>/scripts/<helper>`, as its invocation path. Bundled agent-facing prose — every registered package's `SKILL.md`, references and assets, the orchestrator session context and the escalation framing — SHALL name a helper only through `{{BROKER_HELPER}}`, `{{SWEEP_HELPER}}` or `{{DOCS_FETCH_HELPER}}`, which every renderer SHALL substitute from the registry and the unknown-placeholder check SHALL know.

#### Scenario: The rendered boot block invokes the package helper

- **WHEN** the helper form of the boot block is rendered for branch `feat/x`
- **THEN** the REGISTER section SHALL contain `.git-paw/skills/coordination/scripts/broker.sh --agent feat-x status booting`

Test: `skills::tests::the_rendered_boot_block_invokes_the_package_helper`

#### Scenario: No bundled template carries a literal helper path

- **WHEN** every bundled prose file named above is scanned
- **THEN** none SHALL contain `.git-paw/scripts/` or a literal `.git-paw/skills/<package>/scripts/` path

Test: `skills::tests::no_bundled_template_carries_a_literal_helper_path`

#### Scenario: Helper placeholders raise no warning

- **WHEN** a template using `{{BROKER_HELPER}}`, `{{SWEEP_HELPER}}` and `{{DOCS_FETCH_HELPER}}` is rendered
- **THEN** no unsubstituted-placeholder warning SHALL be written to standard error

Test: `skills::tests::helper_placeholders_raise_no_warning`

#### Scenario: The exported orchestrator skill names the package helper

- **WHEN** `git paw init` writes `.git-paw/skills/orchestrator/SKILL.md`
- **THEN** it SHALL contain `.git-paw/skills/orchestrator/scripts/sweep.sh snapshot`
- **AND** it SHALL NOT contain `{{SWEEP_HELPER}}`

Test: `skills::tests::the_exported_orchestrator_skill_names_the_package_helper`, `tests/cli_init_installs_packages.rs::the_exported_orchestrator_skill_names_the_package_helper`

### Requirement: Helpers the system invokes itself use the package path

Code that invokes a bundled helper on the system's own behalf, such as the drive loop's learnings sink, SHALL invoke that helper's package path at the repository root, not its compatibility copy.

#### Scenario: The drive loop records learnings through the package helper

- **GIVEN** an unattended session whose drive loop records a learning
- **WHEN** the record is written
- **THEN** it SHALL be written by invoking `.git-paw/skills/orchestrator/scripts/sweep.sh learn` at the repository root

Test: `supervisor::drive::tests::a_fast_learning_runs_with_the_session_broker_url`

### Requirement: The legacy helper location is a deprecated compatibility copy

Wherever the system installs a package's helpers, it SHALL also write each of those helpers, and `_paw_common.sh`, to `.git-paw/scripts/<helper>` with content byte-identical to the package copy and the same executable mode, so invocations of the pre-package path keep working. No bundled template SHALL name the compatibility path. The compatibility copy SHALL be documented as deprecated, with its removal deferred to the v1.0.0 deprecation policy.

#### Scenario: The compatibility copy matches the package copy

- **WHEN** `git paw init` runs
- **THEN** `.git-paw/scripts/sweep.sh` SHALL exist and be byte-identical to `.git-paw/skills/orchestrator/scripts/sweep.sh`

Test: `tests/cli_init_installs_packages.rs::the_compatibility_copy_matches_the_package_copy`, `init::tests::the_compatibility_copy_matches_the_package_copy`

#### Scenario: An invocation of the old path still works

- **GIVEN** a provisioned agent worktree
- **WHEN** `.git-paw/scripts/broker.sh --help` is run there
- **THEN** it SHALL exit 0 and print the helper's usage

Test: `tests/worktree_helper_provisioning.rs::bare_launch_provisions_broker_helper_into_each_worktree`

### Requirement: Requirements naming the legacy deployed helper path are satisfied by the package path

The system SHALL satisfy a requirement that bundled prose, a boot block, a sidecar section or the system itself name or invoke a helper as `.git-paw/scripts/<helper>` by naming or invoking its package path; a requirement that a helper be installed there SHALL be met by the compatibility copy. This SHALL NOT apply to the `cli-setup` requirements or to doctor's setup, broad-grant and grant-residue checks, where each path means what it says.

#### Scenario: A skill-content requirement on the old path is checked against the package path

- **GIVEN** a requirement that the orchestrator skill name `.git-paw/scripts/sweep.sh` as its detection mechanism
- **WHEN** its covering test runs against the rendered skill
- **THEN** the test SHALL find `.git-paw/skills/orchestrator/scripts/sweep.sh` in that position
- **AND** SHALL NOT require the literal `.git-paw/scripts/sweep.sh`

Test: `tests/orchestrator_skill_content.rs::skill_teaches_the_helper_subcommands`

#### Scenario: The setup-cli grant table is not remapped

- **GIVEN** the `cli-setup` requirement that puts `Bash(.git-paw/scripts/sweep.sh approve *)` in `permissions.ask`
- **WHEN** the allowlist item is planned
- **THEN** that compatibility-path ask rule SHALL be planned as written, alongside the package-path one

Test: `cli_setup::facts::tests::allow_entries_are_exactly_the_grant_table`

#### Scenario: Doctor's grant checks are not remapped

- **GIVEN** a main-checkout `.claude/settings.local.json` whose `permissions.allow` holds `Bash(.git-paw/scripts/sweep.sh approve *)`
- **WHEN** `git paw doctor` runs
- **THEN** the grant-residue check SHALL report that entry

Test: `cli_setup::doctor::tests::a_durable_self_approval_grant_is_flagged`, `tests/doctor_e2e.rs::doctor_warns_about_grant_residue_in_both_settings_files_and_edits_neither`
