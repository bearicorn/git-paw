# cli-specs-supervisor-filter Specification

## Purpose
Ensures `git paw start --supervisor --specs <names>` launches worktrees only for the named subset — passing the resolved `--specs` value (not just its presence) to the supervisor launch path so it matches non-supervisor behaviour exactly. It pins this with a regression matrix across every selection-flag/supervisor-flag combination and requires identical behaviour across all spec backends (OpenSpec, Markdown, Spec Kit).

## Requirements
### Requirement: --specs list honoured under --supervisor

The dispatcher SHALL pass the `--specs` flag's resolved value
(not just its presence) to the supervisor launch path. When the
user invokes `git paw start --supervisor --specs <names>`, the
system SHALL launch worktrees ONLY for the named subset of
discovered specs, matching the non-supervisor `--specs`
behaviour exactly.

#### Scenario: --supervisor --specs subset filters correctly

- **GIVEN** an `openspec/changes/` directory containing N
  discovered specs (N > 2)
- **WHEN** the user runs
  `git paw start --supervisor --specs a,b` where `a` and `b`
  are exactly two of the N specs
- **THEN** the launch SHALL create exactly two worktrees
  (for `a` and `b`) and SHALL NOT create worktrees for any
  other spec in the directory

#### Scenario: --supervisor --from-all-specs still launches all

- **GIVEN** the same N discovered specs
- **WHEN** the user runs
  `git paw start --supervisor --from-all-specs`
- **THEN** the launch SHALL create worktrees for all N
  discovered specs (current behaviour preserved)

#### Scenario: --supervisor with no spec flag uses branch picker

- **WHEN** the user runs `git paw start --supervisor` with
  neither `--specs` nor `--from-all-specs`
- **THEN** the launch SHALL behave identically to
  `git paw start` (branch picker) but with supervisor mode
  enabled — no automatic discovery of every spec

#### Scenario: --supervisor --specs (picker, no values) opens interactive picker

- **GIVEN** an interactive TTY
- **WHEN** the user runs `git paw start --supervisor --specs`
  with no values
- **THEN** the system SHALL open the multi-select picker;
  selected specs alone SHALL determine the launched worktrees

#### Scenario: --supervisor + --specs without --from-all-specs

- **GIVEN** N discovered specs in `openspec/changes/`
- **WHEN** the user runs
  `git paw start --supervisor --specs cold-start-ci-parity`
  (a single named spec)
- **THEN** exactly one worktree SHALL be created (for
  `cold-start-ci-parity`); no worktree SHALL exist for any
  other spec

### Requirement: Regression matrix for selection-flag combinations

The test suite SHALL include a matrix of integration tests
covering every combination of selection flag and supervisor
flag. The matrix SHALL include at minimum the 15 combinations
of `{none, --from-all-specs, --specs <names>, --specs (picker),
--branches}` × `{no supervisor, --supervisor, --no-supervisor}`.

#### Scenario: Matrix passes for every combination

- **WHEN** the integration matrix runs
- **THEN** every cell SHALL pass — each combination produces
  the documented worktree set, and no combination produces
  worktrees for specs outside the filter

#### Scenario: Matrix catches a regression in any cell

- **GIVEN** a hypothetical regression in any cell (e.g.
  someone re-introduces the v0.6.0 bug where
  `--supervisor --specs <list>` launches all discovered specs)
- **WHEN** the matrix runs
- **THEN** the failing cell's test SHALL fail with a message
  identifying the offending flag combination

### Requirement: Stack-agnostic and backend-agnostic

The filter behaviour SHALL be identical across all configured
spec backends (OpenSpec, Markdown, Spec Kit). The bug fix
SHALL NOT introduce new backend-specific code paths.

#### Scenario: Same filter behaviour on Markdown backend

- **GIVEN** a Markdown-backend repo with N specs
- **WHEN** the user runs
  `git paw start --supervisor --specs a`
- **THEN** the launch SHALL create exactly one worktree for
  spec `a`, matching the OpenSpec-backend behaviour

#### Scenario: Same filter behaviour on Spec Kit backend

- **GIVEN** a Spec Kit `.specify/specs/` repo with N features
- **WHEN** the user runs
  `git paw start --supervisor --specs 003-feature-name`
- **THEN** the launch SHALL create exactly one worktree for
  that feature

