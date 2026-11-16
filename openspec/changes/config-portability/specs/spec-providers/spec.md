## MODIFIED Requirements

### Requirement: Spec-system selection is explicit (config or CLI only)

The spec system SHALL be resolved from EXPLICIT sources only, in this precedence (highest first):

1. the `--specs-format` CLI value;
2. the `[specs]` section in `.git-paw/config.toml`.

git-paw SHALL NOT probe the filesystem to infer the spec system. When neither an `[specs]` section nor `--specs-format` is provided, spec scanning SHALL fail with an actionable error naming both remedies (add a `[specs]` section, or pass `--specs-format`). When a format is chosen but no `dir` is configured, the format's conventional directory SHALL be supplied (`specs` for `speckit`, `docs/superpowers/plans` for `superpowers`). For `speckit` only, when `specs/` does not exist and the legacy `.specify/specs/` does, the legacy directory SHALL be used instead. A configured `dir` SHALL be used verbatim.

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

- **WHEN** `--specs-format speckit` is passed with no configured `dir`, and `specs/` exists
- **THEN** `specs.dir` SHALL default to `specs`
- **AND** `--specs-format superpowers` SHALL likewise default `specs.dir` to `docs/superpowers/plans`

#### Scenario: Spec Kit features under specs/ are found without a dir

- **GIVEN** `[specs] type = "speckit"` with no `dir`, and a feature at `specs/001-auth/tasks.md`
- **WHEN** spec scanning runs
- **THEN** the `001-auth` feature SHALL be discovered

#### Scenario: A legacy .specify/specs layout keeps scanning

- **GIVEN** `[specs] type = "speckit"` with no `dir`, no `specs/` directory, and a feature at `.specify/specs/001-auth/tasks.md`
- **WHEN** spec scanning runs
- **THEN** the `001-auth` feature SHALL be discovered from `.specify/specs`
