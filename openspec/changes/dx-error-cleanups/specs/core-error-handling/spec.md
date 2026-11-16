## ADDED Requirements

### Requirement: A budget abandon is reported as a budget error

When an unattended run stops because a budget cap was reached under `budget_on_exhausted = "abandon"`, the command SHALL fail with a dedicated budget-exhausted error variant whose message begins `Budget:`, states that a budget cap ended the run and that the agents are still running, and SHALL exit with the general error code (1). It SHALL NOT be reported as a session error.

#### Scenario: An abandoned run prints a budget error

- **GIVEN** an unattended run with `budget_on_exhausted = "abandon"` whose usage reaches `budget_max_tokens`
- **WHEN** the drive loop stops on the cap
- **THEN** stderr SHALL contain `error: Budget:` and SHALL NOT contain `Session:`
- **AND** the exit code SHALL be 1

#### Scenario: The variant maps to the general error code

- **WHEN** the exit code of the budget-exhausted error is read
- **THEN** it SHALL be 1

### Requirement: Configuration value constructors return a typed error

A configuration value type that validates its input on construction, such as the budget cost cap, SHALL return a dedicated error type that carries the rejected value, not a bare string. Its rendered message SHALL name the field and the rejected value, and the configuration error a user sees for that value SHALL be unchanged.

#### Scenario: A non-positive cost cap is a typed error

- **WHEN** the budget cost cap is constructed from `0`
- **THEN** it SHALL return the cost-cap error type holding `0`
- **AND** its message SHALL name `budget_max_cost` and `0`

#### Scenario: The config error text is unchanged

- **GIVEN** a config file with `budget_max_cost = -1`
- **WHEN** the config is loaded
- **THEN** the load SHALL fail with a configuration error naming the file, `budget_max_cost` and `-1`
