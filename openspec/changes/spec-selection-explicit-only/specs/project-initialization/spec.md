## ADDED Requirements

### Requirement: Init records the spec system explicitly, never by detection

When generating a fresh `.git-paw/config.toml`, `git paw init` SHALL determine the spec system by an EXPLICIT choice, never by probing the filesystem:

- In an interactive terminal, it SHALL prompt the user to choose one of `openspec`, `markdown`, `speckit`, or `superpowers`, and write the matching `[specs]` section (the chosen `type` plus that format's conventional `dir`).
- In a non-interactive context (no TTY — CI, tests, piped stdin), it SHALL write a commented `[specs]` template listing the four choices, so init stays scriptable and the user fills it in later.

Init SHALL NOT probe `.specify/`, `docs/superpowers/plans/`, or any other path to pre-fill the spec system.

#### Scenario: Non-interactive init writes a commented [specs] template

- **GIVEN** `git paw init` runs with a non-terminal stdin and no existing config
- **WHEN** the config is generated
- **THEN** it SHALL contain a commented `# [specs]` template listing `openspec`, `markdown`, `speckit`, and `superpowers`
- **AND** it SHALL NOT contain an active (uncommented) `[specs]` section inferred from the filesystem

#### Scenario: Interactive init records the chosen spec system

- **GIVEN** `git paw init` runs interactively and the user selects `speckit`
- **WHEN** the config is generated
- **THEN** it SHALL contain a `[specs]` section with `type = "speckit"` and `dir = ".specify/specs"`
