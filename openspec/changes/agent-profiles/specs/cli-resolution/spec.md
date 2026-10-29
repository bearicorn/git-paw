## MODIFIED Requirements

### Requirement: CLI resolution chain for spec-driven launches

The system SHALL resolve which CLI — or which worker profile — to use for each spec-driven branch using a 5-level priority chain, from highest to lowest priority:

1. the `--profile` or `--cli` flag (mutually exclusive) — every branch, no prompt;
2. the spec's `paw_profile` or `paw_cli` frontmatter field — that branch only, no prompt;
3. `default_spec_cli` — every remaining branch, no prompt;
4. `default_profile`, else `default_cli` — pre-selected in the picker shown once for the remaining branches;
5. nothing — the full picker for the remaining branches, with no pre-selection.

This chain SHALL apply identically to non-supervisor and supervisor-mode spec-driven launches — the `--supervisor` flag SHALL NOT bypass or alter it. In particular, `default_spec_cli` SHALL be honoured for spec branches lacking a `paw_cli` or `paw_profile` override even when `--supervisor` is combined with `--specs`, so supervisor-mode workers are never silently launched with a different (e.g. unsandboxed) CLI than configured. A spec that sets both `paw_profile` and `paw_cli` SHALL fail the launch with an error naming the spec. A `--profile`, `paw_profile` or `default_profile` value that names no profile, or names a delegate profile, SHALL fail the launch with an error naming the value and its source. A branch resolved to a profile SHALL launch as that worker profile specifies. The picker SHALL list worker profiles alongside the available CLIs.

#### Scenario: --cli flag overrides everything
- **WHEN** `--cli claude` is passed and specs have various `paw_cli` values
- **THEN** all branches SHALL use `"claude"` regardless of spec or config values

#### Scenario: --profile flag overrides everything
- **WHEN** `--profile routine` is passed and specs have various `paw_cli` and `paw_profile` values
- **THEN** all branches SHALL launch with the worker profile `routine` and no picker SHALL be shown

#### Scenario: paw_cli in spec overrides config
- **WHEN** no `--cli` flag is passed and a spec has `paw_cli: gemini`
- **THEN** that branch SHALL use `"gemini"` regardless of `default_spec_cli` or `default_cli`

#### Scenario: paw_profile in spec overrides config
- **WHEN** no `--cli` or `--profile` flag is passed and a spec has `paw_profile: routine`
- **THEN** that branch SHALL launch with the worker profile `routine` regardless of `default_spec_cli`, `default_profile` or `default_cli`

#### Scenario: A spec with both paw_profile and paw_cli is rejected
- **WHEN** a spec sets both `paw_profile: routine` and `paw_cli: claude`
- **THEN** the launch SHALL fail with an error naming that spec

#### Scenario: paw_profile naming an unknown or delegate profile is rejected
- **WHEN** a spec sets `paw_profile: nonexistent`, and separately one sets `paw_profile: explorer` where `explorer` is a delegate profile
- **THEN** each launch SHALL fail with an error naming the value and the spec

#### Scenario: default_spec_cli fills remaining without prompt
- **WHEN** no `--cli` flag, some specs have no `paw_cli`, and `default_spec_cli = "claude"` in config
- **THEN** specs without `paw_cli` SHALL use `"claude"` with no interactive prompt

#### Scenario: default_cli pre-selects in picker
- **WHEN** no `--cli` flag, no `paw_cli`, no `default_spec_cli`, and `default_cli = "claude"` in config
- **THEN** the CLI picker SHALL be shown with `"claude"` pre-selected

#### Scenario: default_profile pre-selects in picker ahead of default_cli
- **WHEN** no flag, no `paw_cli` or `paw_profile`, no `default_spec_cli`, `default_profile = "routine"` and `default_cli = "claude"` in config
- **THEN** the picker SHALL be shown once for the remaining branches with the `routine` profile pre-selected

#### Scenario: No defaults — full picker
- **WHEN** no `--cli` flag, no `paw_cli`, no `default_spec_cli`, and no `default_cli`
- **THEN** the CLI picker SHALL be shown with no pre-selection

#### Scenario: default_spec_cli is honoured under --supervisor --specs
- **WHEN** `git paw start --supervisor --specs a,b` runs with no `--cli`, specs `a` and `b` having no `paw_cli`, and `default_spec_cli = "claude-oss"` in config
- **THEN** the `a` and `b` worktree panes SHALL launch the `"claude-oss"` CLI (not a fallback or default CLI), identical to the non-supervisor `--specs` resolution
