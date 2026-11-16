## MODIFIED Requirements

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

Test: `src/config/supervisor.rs::absent_budget_caps_load_as_no_cap_and_are_not_written_back`

#### Scenario: A zero cap is rejected

- **GIVEN** `[supervisor] budget_max_tokens = 0`
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error naming `budget_max_tokens`

Test: `src/config/supervisor.rs::a_zero_or_negative_cap_is_rejected_naming_the_field`

#### Scenario: Caps round-trip

- **GIVEN** `[supervisor] budget_max_tokens = 2000000` and `budget_max_cost = 25.0`
- **WHEN** the config is loaded and saved back
- **THEN** both values SHALL be preserved

Test: `src/config/supervisor.rs::budget_caps_round_trip`
