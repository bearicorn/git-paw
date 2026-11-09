## MODIFIED Requirements

### Requirement: The budget exhaustion policy

`[supervisor]` SHALL accept an optional `budget_on_exhausted` of `"escalate"` or `"abandon"`,
defaulting to `"escalate"` when absent and omitted on save when absent. Any other value SHALL be
rejected at config load naming the accepted set. It SHALL be independent of
`[supervisor.correction] on_exhausted`.

#### Scenario: The budget policy is independent of the correction policy

- **GIVEN** `budget_on_exhausted = "abandon"` and `[supervisor.correction] on_exhausted = "escalate"`
- **WHEN** the config is loaded and saved back
- **THEN** each policy SHALL keep its own value

#### Scenario: An unknown budget policy is rejected

- **GIVEN** `[supervisor] budget_on_exhausted = "stop"`
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error naming `escalate` and `abandon`

### Requirement: Doctor flags a cost cap no worker CLI can satisfy

When `budget_max_cost` is set and not every configured CLI reports cost, `git paw doctor` SHALL report a warning naming `budget_max_cost`. The configured CLIs are `default_cli`, `default_spec_cli` and the presets' CLIs; the doctor reads config, not a session, so it checks these and not the running workers the drive loop checks. It SHALL NOT be a config error, and SHALL NOT warn when no CLI is configured.

#### Scenario: Doctor warns about a cost cap no worker CLI can satisfy

- **GIVEN** `budget_max_cost = 25.0` and only Claude-family worker CLIs, which report no cost
- **WHEN** `git paw doctor` runs
- **THEN** it SHALL report a warning for `budget_max_cost`, not a failure
