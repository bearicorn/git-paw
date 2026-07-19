## ADDED Requirements

### Requirement: Backend dispatch for Superpowers type

The system SHALL select the `SuperpowersBackend` implementation when `specs.type = "superpowers"` is configured. The dispatch SHALL be additive to the existing dispatch table — `"openspec"`, `"markdown"`, and `"speckit"` dispatch SHALL continue to work unchanged.

#### Scenario: Type "superpowers" selects Superpowers backend

- **WHEN** `specs.type = "superpowers"` is configured
- **THEN** the Superpowers backend SHALL be used for scanning

#### Scenario: Existing types continue to dispatch correctly

- **WHEN** `specs.type = "openspec"`, `"markdown"`, or `"speckit"` is configured
- **THEN** the corresponding existing backend SHALL be used for scanning
- **AND** the Superpowers backend SHALL NOT be invoked

#### Scenario: Unknown type error lists superpowers among known types

- **WHEN** `specs.type = "unrecognised"` is configured
- **THEN** the system SHALL return a `PawError::SpecError` mentioning the unknown type
- **AND** the error message SHALL list the known types including `"superpowers"`

### Requirement: Auto-detection of Superpowers projects

The system SHALL probe for a `docs/superpowers/plans/` directory (containing at least one `*.md` file) at the repository root when both of the following are true:

- The user has not set `[specs]` in `.git-paw/config.toml` (no `type` and no `dir`).
- The user has not passed `--specs-format` on the CLI.

When auto-detection runs and finds `docs/superpowers/plans/` with at least one plan file, the system SHALL behave as if the user had configured `specs.type = "superpowers"` and `specs.dir = "docs/superpowers/plans"`. Explicit configuration (TOML or `--specs-format`) SHALL always take precedence over auto-detection.

When more than one auto-detectable layout is present and no explicit configuration is given, auto-detection SHALL apply a deterministic precedence — `openspec` (`openspec/`), then `speckit` (`.specify/specs/`), then `superpowers` (`docs/superpowers/plans/`) — selecting the first match. The chosen backend SHALL be reported (stderr) so the ambiguity is visible, and the user can override with explicit config.

If `docs/superpowers/plans/` does not exist, is not a directory, or contains no `*.md` files, superpowers auto-detection SHALL NOT activate.

#### Scenario: Auto-detection activates Superpowers backend in unconfigured project

- **GIVEN** a repository containing `docs/superpowers/plans/2026-07-20-add-auth.md`, no `[specs]` section in `.git-paw/config.toml`, and no other auto-detectable layout
- **AND** `--specs-format` is not passed
- **WHEN** spec scanning runs
- **THEN** the Superpowers backend SHALL be used
- **AND** `specs.dir` SHALL be `docs/superpowers/plans`

#### Scenario: Explicit config or flag wins over auto-detection

- **GIVEN** a repository containing `docs/superpowers/plans/*.md`
- **WHEN** the user sets `[specs] type = "markdown"` in TOML, or passes `--specs-format openspec`
- **THEN** the configured/flagged backend SHALL be used
- **AND** superpowers auto-detection SHALL NOT activate

#### Scenario: Precedence when multiple layouts co-exist

- **GIVEN** a repository containing both `.specify/specs/` and `docs/superpowers/plans/*.md` and no explicit `[specs]` config or `--specs-format`
- **WHEN** spec scanning runs
- **THEN** the Spec Kit backend SHALL be selected (speckit precedes superpowers)
- **AND** the selected backend SHALL be reported on stderr

#### Scenario: Empty or missing plans dir does not activate Superpowers backend

- **GIVEN** a repository whose `docs/superpowers/plans/` is absent or contains no `*.md` files, and no `[specs]` config
- **WHEN** spec scanning runs
- **THEN** the Superpowers backend SHALL NOT be activated

### Requirement: --specs-format accepts superpowers value

The system SHALL accept `superpowers` as a valid value for the `--specs-format` CLI flag, alongside `openspec`, `markdown`, and `speckit`. The flag's value SHALL override both auto-detection and TOML config.

#### Scenario: --specs-format superpowers selects Superpowers backend

- **WHEN** `--specs-format superpowers` is passed
- **THEN** the Superpowers backend SHALL be used regardless of any `[specs] type` set in config

#### Scenario: --specs-format value list includes superpowers

- **WHEN** `--specs-format unknown-value` is passed
- **THEN** the CLI SHALL reject the invocation with an error listing valid values: `openspec`, `markdown`, `speckit`, `superpowers`
