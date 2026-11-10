## MODIFIED Requirements

### Requirement: Doctor flags a cost cap no worker CLI can satisfy

When `budget_max_cost` is set and not every configured CLI reports cost, `git paw doctor` SHALL report a warning naming `budget_max_cost`. The configured CLIs are `default_cli`, `default_spec_cli` and the presets' CLIs; the doctor reads config, not a session, so it checks these and not the running workers the drive loop checks. It SHALL NOT be a config error, and SHALL NOT warn when no CLI is configured.

#### Scenario: Doctor warns about a cost cap no worker CLI can satisfy

- **GIVEN** `budget_max_cost = 25.0` and only Claude-family worker CLIs, which report no cost
- **WHEN** `git paw doctor` runs
- **THEN** it SHALL report a warning for `budget_max_cost`, not a failure

Test: `src/doctor_tests.rs::the_probe_flags_a_cost_cap_over_a_claude_fleet`, `src/doctor_tests.rs::a_cost_cap_no_worker_cli_reports_is_a_warning_not_a_failure`

#### Scenario: No worker CLI configured raises no cost-cap warning

- **GIVEN** `budget_max_cost = 25.0` and no `default_cli`, `default_spec_cli` or preset CLI
- **WHEN** `git paw doctor` runs
- **THEN** it SHALL NOT report a `budget_max_cost` warning

Test: `src/doctor_tests.rs::no_worker_cli_configured_raises_no_cost_cap_warning`
