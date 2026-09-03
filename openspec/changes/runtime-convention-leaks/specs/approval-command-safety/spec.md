## MODIFIED Requirements

### Requirement: Prompt class identification

The detector SHALL classify each detected prompt into one of a fixed set of permission types so callers can decide whether to auto-approve. The set SHALL NOT include a class defined by a specific language toolchain's build commands. Toolchain-specific verbs, where they are relevant to a safety decision, SHALL be sourced from the resolved stack preset rather than hard-coded into the classifier, so the detector stays stack-neutral for every consumer.

#### Scenario: Curl prompts classified as Curl

- **GIVEN** captured pane content containing both an approval marker and `curl`
- **WHEN** classification runs
- **THEN** the result SHALL be `PermissionType::Curl`

#### Scenario: Unknown prompts classified as Unknown

- **GIVEN** captured pane content containing an approval marker but no recognised command class
- **WHEN** classification runs
- **THEN** the result SHALL be `PermissionType::Unknown`
- **AND** auto-approval SHALL NOT be triggered for `Unknown`

#### Scenario: No toolchain-specific permission class exists

- **WHEN** the set of permission types is inspected
- **THEN** it SHALL NOT contain a class defined by a specific language toolchain's build commands

#### Scenario: Build-tool prompt is not specially classified

- **GIVEN** captured pane content containing an approval marker and a language build-tool invocation
- **WHEN** classification runs
- **THEN** it SHALL NOT be assigned a toolchain-specific class
- **AND** the auto-approval decision SHALL be made by the safe-command classifier, whose behaviour is unchanged
