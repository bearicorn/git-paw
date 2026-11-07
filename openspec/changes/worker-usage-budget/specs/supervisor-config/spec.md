## ADDED Requirements

### Requirement: Budget caps in `[supervisor]`

`[supervisor]` SHALL accept two optional budget caps: `budget_max_tokens`, a positive integer
counting input, output, cache-read and cache-write tokens across the attributed usage, and
`budget_max_cost`, a positive number in the currency the CLI's own cost figure uses. Absent means
no cap. A value of `0` or a negative value SHALL be rejected at config load naming the field. Both
SHALL be omitted on save when absent.

#### Scenario: Absent caps load as no cap

- **GIVEN** a `[supervisor]` section with neither budget field
- **WHEN** the config is loaded
- **THEN** no budget cap SHALL be in effect

#### Scenario: A zero cap is rejected

- **GIVEN** `[supervisor] budget_max_tokens = 0`
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error naming `budget_max_tokens`

#### Scenario: Caps round-trip

- **GIVEN** `[supervisor] budget_max_tokens = 2000000` and `budget_max_cost = 25.0`
- **WHEN** the config is loaded and saved back
- **THEN** both values SHALL be preserved

### Requirement: The budget exhaustion policy

`[supervisor]` SHALL accept an optional `budget_on_exhausted` of `"escalate"` or `"abandon"`,
defaulting to `"escalate"` when absent and omitted on save when absent. Any other value SHALL be
rejected at config load naming the accepted set. It SHALL be independent of
`[supervisor.correction] on_exhausted`.

#### Scenario: The budget policy is independent of the correction policy

- **GIVEN** `budget_on_exhausted = "abandon"` and `[supervisor.correction] on_exhausted = "escalate"`
- **WHEN** the config is loaded and saved back
- **THEN** each policy SHALL keep its own value

### Requirement: Doctor flags a cost cap no worker CLI can satisfy

When `budget_max_cost` is set and at least one worker CLI is configured but not every configured
worker CLI reports cost — the drive loop's condition — `git paw doctor` SHALL report a warning
naming `budget_max_cost`. It SHALL NOT be a config error.

#### Scenario: Doctor warns about a cost cap no worker CLI can satisfy

- **GIVEN** `budget_max_cost = 25.0` and only Claude-family worker CLIs, which report no cost
- **WHEN** `git paw doctor` runs
- **THEN** it SHALL report a warning for `budget_max_cost`, not a failure
