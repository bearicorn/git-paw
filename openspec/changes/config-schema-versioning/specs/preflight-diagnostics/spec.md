## ADDED Requirements

### Requirement: Config check reports the schema version

The Config group of `git paw doctor` SHALL include a schema-version check for the repository config. It SHALL report ✓ with the declared version when the file declares a supported `version`, and ✓ noting the implicit version 1 when the file declares none. It SHALL report ✗ with a remedy to upgrade git-paw when the file declares a version newer than the binary supports, and ✗ with a remedy to correct the value when the version is invalid. The `version` key SHALL NOT be reported as an unknown field. The check SHALL be read-only.

#### Scenario: Declared supported version passes

- **GIVEN** a `.git-paw/config.toml` declaring `version = 1`
- **WHEN** `git paw doctor` is run
- **THEN** the Config group SHALL report ✓ for the schema version, naming version 1
- **AND** `version` SHALL NOT appear in any unknown-fields check

#### Scenario: Unversioned config passes as implicit version 1

- **GIVEN** a `.git-paw/config.toml` without a `version` key
- **WHEN** `git paw doctor` is run
- **THEN** the Config group SHALL report ✓ noting the implicit version 1

#### Scenario: A newer config fails with an upgrade remedy

- **GIVEN** a `.git-paw/config.toml` declaring `version = 2` and a binary supporting version 1
- **WHEN** `git paw doctor` is run
- **THEN** the Config group SHALL report ✗ for the schema version with a remedy to upgrade git-paw
- **AND** the process SHALL exit non-zero
- **AND** the config file SHALL be unchanged
