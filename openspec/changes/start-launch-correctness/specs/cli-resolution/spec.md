## MODIFIED Requirements

### Requirement: CLI resolution chain for spec-driven launches

The system SHALL resolve which CLI to use for each spec-driven branch using a 5-level priority chain, from highest to lowest priority. This chain SHALL apply identically to non-supervisor and supervisor-mode spec-driven launches — the `--supervisor` flag SHALL NOT bypass or alter it. In particular, `default_spec_cli` SHALL be honoured for spec branches lacking a `paw_cli` override even when `--supervisor` is combined with `--specs`, so supervisor-mode workers are never silently launched with a different (e.g. unsandboxed) CLI than configured.

#### Scenario: --cli flag overrides everything
- **WHEN** `--cli claude` is passed and specs have various `paw_cli` values
- **THEN** all branches SHALL use `"claude"` regardless of spec or config values

#### Scenario: paw_cli in spec overrides config
- **WHEN** no `--cli` flag is passed and a spec has `paw_cli: gemini`
- **THEN** that branch SHALL use `"gemini"` regardless of `default_spec_cli` or `default_cli`

#### Scenario: default_spec_cli fills remaining without prompt
- **WHEN** no `--cli` flag, some specs have no `paw_cli`, and `default_spec_cli = "claude"` in config
- **THEN** specs without `paw_cli` SHALL use `"claude"` with no interactive prompt

#### Scenario: default_cli pre-selects in picker
- **WHEN** no `--cli` flag, no `paw_cli`, no `default_spec_cli`, and `default_cli = "claude"` in config
- **THEN** the CLI picker SHALL be shown with `"claude"` pre-selected

#### Scenario: No defaults — full picker
- **WHEN** no `--cli` flag, no `paw_cli`, no `default_spec_cli`, and no `default_cli`
- **THEN** the CLI picker SHALL be shown with no pre-selection

#### Scenario: default_spec_cli is honoured under --supervisor --specs
- **WHEN** `git paw start --supervisor --specs a,b` runs with no `--cli`, specs `a` and `b` having no `paw_cli`, and `default_spec_cli = "claude-oss"` in config
- **THEN** the `a` and `b` worktree panes SHALL launch the `"claude-oss"` CLI (not a fallback or default CLI), identical to the non-supervisor `--specs` resolution
