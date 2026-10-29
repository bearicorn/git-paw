## ADDED Requirements

### Requirement: `[remote] vendor` and `[remote.jules]` configure the Jules vendor

The `[remote]` table SHALL accept an optional `vendor` key whose only accepted value is `"jules"`, and an optional `[remote.jules]` table with these optional keys: `plan` — `"free"`, `"pro"` or `"ultra"`, defaulting to `"free"`; `daily_tasks` and `concurrent_tasks` — positive integers that override the plan's limits; and `limit_cooldown_minutes` — a positive integer defaulting to `60`. Any other `vendor` or `plan` value, or a value that is not a positive integer where one is required, SHALL fail loading with a configuration error naming the key and its accepted values. An `api_key` key in `[remote.jules]` SHALL fail loading with an error stating that the key is read only from the `JULES_API_KEY` environment variable. Each key SHALL merge independently, the repository file's value winning over the global file's; the tables SHALL be serialized only when set; and adding them SHALL NOT change the configuration schema version. The configuration reference SHALL document every key, its default, the published limits of each plan with the date they were taken, and that the key is read only from `JULES_API_KEY`.

#### Scenario: Defaults apply

- **GIVEN** a configuration with `[remote] vendor = "jules"` and no `[remote.jules]` table
- **WHEN** it is loaded
- **THEN** the effective plan SHALL be `free` with a daily task limit of 15, a concurrency limit of 3 and a cool-down of 60 minutes

#### Scenario: An unknown vendor is refused

- **GIVEN** a configuration with `[remote] vendor = "other"`
- **WHEN** it is loaded
- **THEN** loading SHALL fail with an error naming `vendor` and `jules`

#### Scenario: A key in the configuration is refused

- **GIVEN** a configuration with `[remote.jules] api_key = "abc"`
- **WHEN** it is loaded
- **THEN** loading SHALL fail with an error naming `JULES_API_KEY`
- **AND** the error SHALL NOT contain `abc`

#### Scenario: Keys merge one by one

- **GIVEN** a global configuration with `[remote.jules] plan = "pro"` and a repository configuration with `[remote] vendor = "jules"`
- **WHEN** the configurations are merged
- **THEN** the effective vendor SHALL be `jules` and the effective plan `pro`
