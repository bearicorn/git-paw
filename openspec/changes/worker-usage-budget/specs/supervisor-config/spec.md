## ADDED Requirements

### Requirement: Budget caps in `[supervisor]`

`[supervisor]` SHALL accept two optional budget caps: `budget_max_tokens`, a positive integer
counting input plus output tokens across the session's attributed usage, and `budget_max_cost`, a
positive number in the currency the CLI's own cost figure uses. Absent means no cap. A value of `0`
or a negative cost SHALL be rejected at config load with an error naming the field. Both fields
SHALL be omitted on serialization when absent, so existing configs round-trip unchanged.

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
