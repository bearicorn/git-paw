## ADDED Requirements

### Requirement: Init template carries commented agent-profile examples

The configuration `git paw init` generates SHALL carry commented-out examples of `default_profile`, one worker profile, and one delegate profile per bundled delegate role skill (`explorer`, `spec-auditor`, `doc-auditor`, `security-auditor`), each with its `kind`, a `description` stating its purpose, and placeholder `cli` and `args` values.

#### Scenario: The template shows profile examples

- **WHEN** the default config string is generated
- **THEN** it SHALL contain commented `[profiles.<name>]` stanzas with `kind = "worker"` and with `kind = "delegate"` for each of `explorer`, `spec-auditor`, `doc-auditor` and `security-auditor`, and a commented `default_profile`

### Requirement: Init profile examples are inert and project-agnostic

The profile examples in the generated configuration SHALL name no vendor CLI, no model identifier and no project toolchain command, and loading the generated configuration without edits SHALL yield no profiles and no `default_profile`.

#### Scenario: The examples are inert and agnostic

- **WHEN** the generated config is loaded without edits and its profile stanzas are audited
- **THEN** the loaded config SHALL have no profiles and no `default_profile`
- **AND** the stanzas SHALL contain no vendor CLI name, model identifier or toolchain command
