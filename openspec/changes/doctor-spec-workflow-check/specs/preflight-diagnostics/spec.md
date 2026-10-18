## ADDED Requirements

### Requirement: Spec-workflow reachability check

`git paw doctor` SHALL include, in its "Spec system" group, a check that the workflow command the boot task prompt instructs spec-driven agents to run is reachable from the repository's worktrees. The check SHALL be a pure function of a probe recording: the resolved spec format, the workflow command that format's task prompt names (if any), the configured worktree placement (`PawConfig::worktree_placement()`), the effective spec-driven worker CLIs — the deduplicated list the discovered specs resolve to, as already surfaced by the spec-worker CLI check, empty when no specs are discovered — and, for each of those CLIs and each location where that CLI's definition of the command can live (as recorded in a per-CLI table with a generic unknown fallback), whether the file exists at the repository root and whether git tracks it. Each resolved CLI SHALL be judged on its own:

- reachable when a definition is tracked by git, or when placement is `child` and a definition exists at the repository root;
- unreachable when its locations are known and either placement is `sibling` and every definition found at the root is untracked, or no definition is found;
- unverified when its locations are unknown.

The verdict SHALL be:

- `Ok` (not applicable) when the format's task prompt names no workflow command, or when no spec-driven worker CLI resolves (an empty list);
- `Warn` when at least one resolved CLI is unreachable — naming the command, every unreachable CLI (and only those), the locations checked, and the remedies (track the definitions, or set `worktree_placement = "child"`);
- otherwise `Ok`, with a detail stating that reachability could not be verified for each unverified CLI.

The check SHALL never be `Fail`, SHALL be read-only, and SHALL appear in `--json` output.

#### Scenario: Sibling placement with an untracked definition warns

- **GIVEN** an OpenSpec repository with `worktree_placement = "sibling"`, spec-driven worker CLI `claude`, and `.claude/commands/opsx/apply.md` present at the root but ignored by git
- **WHEN** `git paw doctor` runs
- **THEN** the spec-workflow check SHALL be `Warn`
- **AND** its detail SHALL name `/opsx:apply` and the location checked
- **AND** its remedy SHALL mention tracking the definitions and `worktree_placement = "child"`

#### Scenario: Child placement reaches the root definition

- **GIVEN** the same repository with `worktree_placement = "child"`
- **WHEN** `git paw doctor` runs
- **THEN** the spec-workflow check SHALL be `Ok`

#### Scenario: A tracked definition is reachable under any placement

- **GIVEN** an OpenSpec repository with sibling placement whose `.claude/commands/opsx/apply.md` is tracked by git
- **WHEN** `git paw doctor` runs
- **THEN** the spec-workflow check SHALL be `Ok`

#### Scenario: A missing definition warns

- **GIVEN** an OpenSpec repository whose spec-driven worker CLI is `claude` and which has no definition of the command at any known location
- **WHEN** `git paw doctor` runs
- **THEN** the spec-workflow check SHALL be `Warn` and name the locations checked

#### Scenario: A format without a workflow command is not applicable

- **GIVEN** a repository whose resolved spec format is Markdown, Spec Kit or Superpowers
- **WHEN** `git paw doctor` runs
- **THEN** the spec-workflow check SHALL be `Ok` and state that the format names no workflow command

#### Scenario: No resolved worker CLI is not applicable

- **GIVEN** an OpenSpec repository in which no specs are discovered, so no spec-driven worker CLI resolves
- **WHEN** `git paw doctor` runs
- **THEN** the spec-workflow check SHALL be `Ok` and state that there is no spec-driven worker CLI to check

#### Scenario: Several worker CLIs warn naming only the unreachable ones

- **GIVEN** an OpenSpec repository with `worktree_placement = "sibling"` whose discovered specs resolve to two CLIs with known locations, `claude` and a second CLI, where only `claude`'s definition is tracked by git
- **WHEN** `git paw doctor` runs
- **THEN** the spec-workflow check SHALL be `Warn`
- **AND** its detail SHALL name the second CLI and its locations checked
- **AND** its detail SHALL NOT name `claude` as unreachable

#### Scenario: An unknown CLI is reported, not warned

- **GIVEN** an OpenSpec repository whose spec-driven worker CLI has no entry in the location table
- **WHEN** `git paw doctor` runs
- **THEN** the spec-workflow check SHALL be `Ok` with a detail stating reachability could not be verified

#### Scenario: The check is in the JSON report

- **WHEN** `git paw doctor --json` runs
- **THEN** the output SHALL contain the spec-workflow check with its status and detail
