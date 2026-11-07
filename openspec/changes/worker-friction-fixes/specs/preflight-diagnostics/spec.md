## MODIFIED Requirements

### Requirement: Spec-workflow reachability check

`git paw doctor` SHALL include, in its "Spec system" group, a check that the workflow command the boot task prompt instructs spec-driven agents to run is reachable from the repository's worktrees. The check SHALL be a pure function of a probe recording: the resolved spec format, the workflow command that format's task prompt names (if any), the configured worktree placement (`PawConfig::worktree_placement()`), the effective spec-driven worker CLIs — the deduplicated list the discovered specs resolve to, as already surfaced by the spec-worker CLI check, empty when no specs are discovered — and, for each of those CLIs and each location where that CLI's definition of the command can live (as recorded in a table keyed by the CLI's provider family — so a `[clis.<name>]` entry declaring a `family` uses that family's locations — with a generic unknown fallback), whether the file exists at the main checkout and whether it is committed there. "Committed" SHALL mean present at the main checkout's `HEAD`, because `git worktree add` populates a new worktree from a commit: a definition that is ignored, untracked, or merely staged does not travel. Each resolved CLI SHALL be judged on its own:

- reachable when a definition is committed, or when placement is `child` and a definition exists at the main checkout;
- unreachable when its locations are known and either placement is `sibling` and every definition found is uncommitted, or no definition is found;
- unverified when its locations are unknown.

The verdict SHALL be:

- `Ok` (not applicable) when the format's task prompt names no workflow command, or when no spec-driven worker CLI resolves (an empty list);
- `Warn` when at least one resolved CLI is unreachable — naming the command, every unreachable CLI (and only those), the locations checked, and a remedy appropriate to the placement: under `sibling` placement both fixes (commit the definitions, or set `worktree_placement = "child"`), and under `child` placement — where the definitions are simply absent, so naming `child` would contradict the finding — installing the definitions at the repository root and committing them;
- `Warn` when no resolved CLI is unreachable but at least one is unverified — the detail stating that reachability could not be verified for each unverified CLI and scoping any reachability claim to the CLIs it covers, and the remedy naming `[worktree.env] copy` in `.git-paw/config.toml` as the way to provision that CLI's workflow command files into each new worktree;
- otherwise `Ok`.

The check SHALL never be `Fail`, SHALL be read-only, and SHALL appear in `--json` output.

#### Scenario: Sibling placement with an uncommitted definition warns

- **GIVEN** an OpenSpec repository with `worktree_placement = "sibling"`, spec-driven worker CLI `claude`, and `.claude/commands/opsx/apply.md` present at the root but ignored by git
- **WHEN** `git paw doctor` runs
- **THEN** the spec-workflow check SHALL be `Warn`
- **AND** its detail SHALL name `/opsx:apply` and the location checked
- **AND** its remedy SHALL mention committing the definitions and `worktree_placement = "child"`

Test: `doctor::tests::spec_workflow_check_warns_on_sibling_placement_with_an_uncommitted_definition, tests/doctor_e2e.rs::an_uncommitted_workflow_definition_warns_under_sibling_placement`

#### Scenario: A staged but uncommitted definition does not travel

- **GIVEN** the same repository where the definition has been `git add`ed but never committed
- **WHEN** `git paw doctor` runs
- **THEN** the spec-workflow check SHALL be `Warn`, because a new worktree is populated from a commit and would not carry it

Test: `doctor::tests::a_staged_but_uncommitted_definition_does_not_reach_a_sibling_worktree, tests/doctor_e2e.rs::a_staged_but_uncommitted_definition_warns_under_sibling_placement`

#### Scenario: Child placement reaches the root definition

- **GIVEN** the same repository with `worktree_placement = "child"`
- **WHEN** `git paw doctor` runs
- **THEN** the spec-workflow check SHALL be `Ok`

Test: `doctor::tests::spec_workflow_check_passes_on_child_placement_with_a_root_definition, tests/doctor_e2e.rs::the_same_uncommitted_definition_passes_under_child_placement, tests/doctor_e2e.rs::child_placement_still_passes_when_doctor_runs_inside_the_worktree`

#### Scenario: A committed definition is reachable under any placement

- **GIVEN** an OpenSpec repository with sibling placement whose `.claude/commands/opsx/apply.md` is committed
- **WHEN** `git paw doctor` runs
- **THEN** the spec-workflow check SHALL be `Ok`

Test: `doctor::tests::spec_workflow_check_passes_when_the_definition_is_committed, tests/doctor_e2e.rs::a_committed_workflow_definition_passes_under_sibling_placement`

#### Scenario: A missing definition warns

- **GIVEN** an OpenSpec repository whose spec-driven worker CLI is `claude` and which has no definition of the command at any known location
- **WHEN** `git paw doctor` runs
- **THEN** the spec-workflow check SHALL be `Warn` and name the locations checked

Test: `doctor::tests::spec_workflow_check_warns_on_sibling_placement_with_no_definition_at_all, doctor::tests::spec_workflow_check_warns_when_no_definition_exists_anywhere, tests/doctor_e2e.rs::a_missing_definition_warns_under_sibling_placement`

#### Scenario: Under child placement the remedy does not name child placement

- **GIVEN** an OpenSpec repository with `worktree_placement = "child"` and no definition of the command at any known location
- **WHEN** `git paw doctor` runs
- **THEN** the spec-workflow check SHALL be `Warn`
- **AND** its remedy SHALL ask for the definitions to be installed and committed
- **AND** its remedy SHALL NOT ask for `worktree_placement = "child"`, which is already set

Test: `doctor::tests::spec_workflow_check_warns_when_no_definition_exists_anywhere`

#### Scenario: A format without a workflow command is not applicable

- **GIVEN** a repository whose resolved spec format is Markdown, Spec Kit or Superpowers
- **WHEN** `git paw doctor` runs
- **THEN** the spec-workflow check SHALL be `Ok` and state that the format names no workflow command

Test: `doctor::tests::spec_workflow_check_is_not_applicable_for_a_format_without_a_workflow_command, doctor::tests::only_the_openspec_format_instructs_a_workflow_command, tests/doctor_e2e.rs::a_repository_without_a_spec_system_reports_no_workflow_command`

#### Scenario: No resolved worker CLI is not applicable

- **GIVEN** an OpenSpec repository in which no specs are discovered, so no spec-driven worker CLI resolves
- **WHEN** `git paw doctor` runs
- **THEN** the spec-workflow check SHALL be `Ok` and state that there is no spec-driven worker CLI to check

Test: `doctor::tests::spec_workflow_check_is_not_applicable_without_a_resolved_worker_cli`

#### Scenario: Several worker CLIs warn naming only the unreachable ones

- **GIVEN** an OpenSpec repository with `worktree_placement = "sibling"` whose discovered specs resolve to two CLIs with known locations, `claude` and a second CLI, where only `claude`'s definition is committed
- **WHEN** `git paw doctor` runs
- **THEN** the spec-workflow check SHALL be `Warn`
- **AND** its detail SHALL name the second CLI and its locations checked
- **AND** its detail SHALL NOT name `claude` as unreachable

Test: `doctor::tests::spec_workflow_check_warns_naming_only_the_unreachable_cli`

#### Scenario: An unknown CLI is reported, not warned

- **GIVEN** an OpenSpec repository whose spec-driven worker CLI has no entry in the location table
- **WHEN** `git paw doctor` runs
- **THEN** the spec-workflow check SHALL be `Warn` (not `Ok`) with a detail stating reachability could not be verified
- **AND** its remedy SHALL name `[worktree.env] copy`
- **AND** when another resolved CLI is reachable, the detail SHALL name that CLI as the one the reachability claim covers

Test: `doctor::tests::spec_workflow_check_warns_on_an_unknown_cli, doctor::tests::a_reachable_and_an_unknown_cli_each_scope_the_detail_to_themselves, tests/doctor_e2e.rs::an_unverifiable_workflow_cli_warns_with_a_provisioning_remedy`

#### Scenario: An unverified CLI never shows as passing

- **GIVEN** an OpenSpec repository whose only spec-driven worker CLI has no entry in the location table
- **WHEN** `git paw doctor --json` runs
- **THEN** the spec-workflow check's status SHALL be the warning status, not the ok status

Test: `tests/doctor_e2e.rs::an_unverifiable_workflow_cli_warns_with_a_provisioning_remedy`

#### Scenario: The check is in the JSON report

- **WHEN** `git paw doctor --json` runs
- **THEN** the output SHALL contain the spec-workflow check with its status and detail

Test: `tests/doctor_e2e.rs::an_uncommitted_workflow_definition_warns_under_sibling_placement, tests/doctor_e2e.rs::json_document_carries_the_required_fields_on_every_check`

#### Scenario: A family variant uses its family's locations

- **GIVEN** an OpenSpec repository whose spec-driven worker CLI is `[clis.claude-opus]` with `family = "claude"`, and whose `.claude/commands/opsx/apply.md` is committed
- **WHEN** `git paw doctor` runs
- **THEN** the spec-workflow check SHALL be `Ok` and name `claude-opus` as reachable

Test: `tests/doctor_e2e.rs::a_family_variant_finds_its_familys_workflow_definitions`

## ADDED Requirements

### Requirement: Doctor flags an out-of-date orchestrator pointer

`git paw doctor` SHALL include in its "Hygiene" group an "orchestrator pointer" check that is `Warn` when the root `AGENTS.md` carries the orchestrator pointer section but not the section the running binary writes (for example one written before the section was scoped to the root session), with a remedy to re-run `git paw init`; and `Ok` when the section is current or absent. It SHALL be read-only and never `Fail`.

#### Scenario: An out-of-date pointer section warns

- **GIVEN** a root `AGENTS.md` whose pointer section was written by an earlier binary
- **WHEN** `git paw doctor` runs
- **THEN** the orchestrator-pointer check SHALL be `Warn` with a remedy naming `git paw init`

Test: `doctor::tests::an_out_of_date_orchestrator_pointer_warns_with_the_init_remedy`, `tests/doctor_e2e.rs::an_out_of_date_orchestrator_pointer_warns`

#### Scenario: A current or absent pointer passes

- **GIVEN** a repository just initialised by the running binary, or one with no pointer section
- **WHEN** `git paw doctor` runs
- **THEN** the orchestrator-pointer check SHALL be `Ok`

Test: `agents::tests::orchestrator_pointer_currency_is_detected`, `doctor::tests::a_clean_repository_passes_every_hygiene_check`
