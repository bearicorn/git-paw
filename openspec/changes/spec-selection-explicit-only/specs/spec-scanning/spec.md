## ADDED Requirements

### Requirement: Spec-system selection is explicit (config or CLI only)

The spec system SHALL be resolved from EXPLICIT sources only, in this precedence (highest first):

1. the `--specs-format` CLI value;
2. the `[specs]` section in `.git-paw/config.toml`.

git-paw SHALL NOT probe the filesystem to infer the spec system. When neither an `[specs]` section nor `--specs-format` is provided, spec scanning SHALL fail with an actionable error naming both remedies (add a `[specs]` section, or pass `--specs-format`). When `--specs-format` names a format but no `dir` is configured, the format's conventional directory SHALL be supplied (`.specify/specs` for `speckit`, `docs/superpowers/plans` for `superpowers`).

#### Scenario: Unconfigured repo errors even when layouts exist on disk

- **GIVEN** a repo with `.specify/specs/` and `docs/superpowers/plans/*.md` present on disk, no `[specs]` section, and no `--specs-format`
- **WHEN** spec scanning runs
- **THEN** it SHALL fail with an error naming `[specs]` and `--specs-format`
- **AND** it SHALL NOT infer a spec system from the filesystem

#### Scenario: Config [specs] is used verbatim regardless of on-disk layout

- **GIVEN** `[specs] type = "markdown"`, `dir = "specs"` in config, and a `.specify/specs/` directory also present on disk
- **WHEN** spec scanning runs
- **THEN** the Markdown backend SHALL be used (the `.specify/` layout is ignored)

#### Scenario: --specs-format supplies the format's conventional dir

- **WHEN** `--specs-format speckit` is passed with no configured `dir`
- **THEN** `specs.dir` SHALL default to `.specify/specs`
- **AND** `--specs-format superpowers` SHALL likewise default `specs.dir` to `docs/superpowers/plans`

## REMOVED Requirements

### Requirement: Auto-detection of Spec Kit projects

**Reason**: Filesystem auto-detection made the resolved spec system depend on directory layout rather than on the config that is meant to be authoritative, and required precedence rules once a second detectable layout (Superpowers) existed. The spec system is now selected explicitly via the `[specs]` config section or the `--specs-format` flag.

**Migration**: A Spec Kit project that relied on `.specify/`-only auto-detection with no `[specs]` section SHALL add `[specs] type = "speckit"`, `dir = ".specify/specs"` to `.git-paw/config.toml` (or re-run `git paw init` and choose Spec Kit), or pass `--specs-format speckit` per launch.

## MODIFIED Requirements

### Requirement: --specs-format accepts speckit value

The system SHALL accept `speckit` as a valid value for the `--specs-format` CLI flag, alongside `openspec` and `markdown`. The flag's value SHALL override the `[specs]` config (there is no filesystem auto-detection to override).

#### Scenario: --specs-format speckit selects SpecKit backend

- **WHEN** `--specs-format speckit` is passed
- **THEN** the SpecKit backend SHALL be used regardless of any `[specs] type` set in config

#### Scenario: --specs-format with unknown value is rejected

- **WHEN** `--specs-format unknown-value` is passed
- **THEN** the CLI SHALL reject the invocation with an error listing valid values: `openspec`, `markdown`, `speckit`
