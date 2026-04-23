# start-force-flag Specification

## Purpose

Define the `--force` flag for the `git paw start --from-specs` command, enabling users to bypass uncommitted-spec validation warnings.

## ADDED Requirements

### Requirement: Force flag definition

The `git paw start` command SHALL accept an optional `--force` boolean flag that defaults to `false`. The flag SHALL appear in `git paw start --help` output with descriptive text explaining that it bypasses the uncommitted-spec validation warning. The flag SHALL parse according to Clap's boolean flag rules.

#### Scenario: Force flag is parsed by clap

- **GIVEN** the `git paw start` command
- **WHEN** the user invokes `git paw start --from-specs --force`
- **THEN** the parsed `Start` arguments expose `force = true`
- **AND** when invoked without `--force`, `force = false`

#### Scenario: Force flag appears in help text

- **WHEN** the user runs `git paw start --help`
- **THEN** the output contains a description for `--force` mentioning uncommitted-spec validation

### Requirement: Validation bypass with force flag

When `--force` is provided, the command SHALL proceed with execution even if uncommitted spec changes are detected. When `--force` is NOT provided and uncommitted specs exist, the command SHALL display a warning that lists the IDs of specs with uncommitted changes and SHALL continue execution. Uncommitted-spec detection SHALL iterate through pending specs from `scan_specs()`, check each spec's directory using `git status --porcelain`, and collect IDs of specs with non-empty git status output.

#### Scenario: No uncommitted specs

- **GIVEN** all pending specs are committed in git
- **WHEN** the user runs `git paw start --from-specs`
- **THEN** the command proceeds normally without any uncommitted-spec warning

#### Scenario: Uncommitted specs without force

- **GIVEN** at least one pending spec has uncommitted changes per `git status --porcelain`
- **WHEN** the user runs `git paw start --from-specs`
- **THEN** stderr contains a warning listing the IDs of specs with uncommitted changes
- **AND** the command continues execution

#### Scenario: Uncommitted specs with force

- **GIVEN** at least one pending spec has uncommitted changes
- **WHEN** the user runs `git paw start --from-specs --force`
- **THEN** the uncommitted-spec warning is suppressed
- **AND** the command logs force usage to stderr for audit purposes
- **AND** the command continues execution

#### Scenario: Force combined with dry run

- **GIVEN** at least one pending spec has uncommitted changes
- **WHEN** the user runs `git paw start --from-specs --force --dry-run`
- **THEN** dry-run output is shown without the uncommitted-spec warning

### Requirement: Force flag error handling

When checking spec git status, the command SHALL propagate `git status` failures as `PawError::GitError`. Invalid spec paths SHALL be skipped without aborting the overall validation.

#### Scenario: Git status failure is reported as PawError

- **GIVEN** a spec directory where `git status --porcelain` exits non-zero
- **WHEN** uncommitted-spec validation runs
- **THEN** the command returns a `PawError::GitError`
