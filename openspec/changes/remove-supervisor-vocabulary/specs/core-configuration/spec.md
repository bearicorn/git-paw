## ADDED Requirements

### Requirement: Version-1 configuration files load throughout 1.x

Every release of the 1.x line SHALL load a configuration file that uses the version-1 `[supervisor]` layout — declaring `version = 1` or no version — through the version-1 to version-2 migration step, with the same effective configuration and the same per-family deprecation warning as in v0.21.0, and the bundled helper scripts SHALL keep their version-1 fallback for the same files. The version-1 paths SHALL NOT be entries of the stable surface: the contract manifest's configuration section SHALL list only version-2 keys, the deprecation registry SHALL record the version-1 paths with no removal release, and a file declaring `version = 2` SHALL keep refusing a `[supervisor]` table. The version-1 `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]` tables, which have had no effect since v0.17.0, SHALL keep loading; the migration step SHALL drop them rather than map them to any version-2 key, and the registry SHALL record them, like the other version-1 paths, with no removal release.

#### Scenario: A 0.x config loads unchanged in 1.0

- **GIVEN** a `.git-paw/config.toml` written for v0.20.0 with `[supervisor] enabled = true`, `test_command = "just check"` and `[supervisor.correction] auto_loopback = true`
- **WHEN** a 1.x release loads it
- **THEN** loading SHALL succeed with coordinated mode as the default, the test command `just check` and the correction loop enabled
- **AND** stderr SHALL carry the deprecation warnings and the file SHALL be unchanged

#### Scenario: A 0.x config with the deprecated auto-approval tables loads in 1.0

- **GIVEN** a version-1 `.git-paw/config.toml` declaring `[supervisor.auto_approve] enabled = true` and `[supervisor.common_dev_allowlist] stacks = ["rust"]`
- **WHEN** a 1.x release loads it
- **THEN** loading SHALL succeed
- **AND** no version-2 key SHALL carry a value from either table
- **AND** stderr SHALL name both tables as deprecated with no effect, and the file SHALL be unchanged

#### Scenario: The manifest lists only version-2 keys

- **WHEN** the contract manifest's configuration section is generated
- **THEN** it SHALL contain `default_mode` and the `[orchestrator]`, `[workers]` and `[drive]` keys
- **AND** it SHALL contain no key under `[supervisor]`

#### Scenario: The helper still reads a version-1 file

- **GIVEN** a config with only `[supervisor] test_command = "just check"`
- **WHEN** `sweep.sh` resolves its test command in a 1.x release
- **THEN** it SHALL resolve `just check`
