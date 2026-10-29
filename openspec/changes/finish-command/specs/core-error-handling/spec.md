## MODIFIED Requirements

### Requirement: Exit codes distinguish cancellation from errors

`UserCancelled` and `NotAttempted` SHALL exit with code 2; `IntegrationRefused` — an integration that `git paw finish` declined under its merge policy (not fast-forwardable, or a conflicting merge) after changing nothing — SHALL exit with code 3; all other errors SHALL exit with code 1. `NotAttempted` reports that a command wrote nothing because a condition for proceeding was not met — a declined confirmation, a confirmation that could not be shown because standard input is not a terminal, or a requested item that is not supported or not enabled for its target. Its message SHALL state that condition and, where one exists, name the flag or command that proceeds, and SHALL NOT read only `Cancelled.`.

#### Scenario: UserCancelled exit code
- **GIVEN** `PawError::UserCancelled`
- **WHEN** `exit_code()` is called
- **THEN** it SHALL return `2`

Test: `error::tests::test_user_cancelled_exit_code`

#### Scenario: NotAttempted exit code and message
- **GIVEN** `PawError::NotAttempted` built for a confirmation that could not be shown, naming `--yes`
- **WHEN** `exit_code()` is called and the error is formatted with `Display`
- **THEN** `exit_code()` SHALL return `2`
- **AND** the message SHALL name `--yes`
- **AND** the message SHALL NOT be `Cancelled.`

Test: `error::tests::test_not_attempted_exit_code_and_message`

#### Scenario: IntegrationRefused exit code
- **GIVEN** `PawError::IntegrationRefused` with any message
- **WHEN** `exit_code()` is called
- **THEN** it SHALL return `3`

#### Scenario: General errors exit code
- **GIVEN** any error variant other than `UserCancelled`, `NotAttempted` and `IntegrationRefused`
- **WHEN** `exit_code()` is called
- **THEN** it SHALL return `1`

Test: `error::tests::test_general_errors_exit_code`
