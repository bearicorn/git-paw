## ADDED Requirements

### Requirement: Remote worker readiness check

When `[remote] vendor` is set, `git paw doctor` SHALL report a remote readiness check: ✗ when the vendor's key variable is unset or holds a control character, naming the variable, printing no part of any key and making no request; ✗ when the vendor transport is unavailable; ✗ when `origin` is not a GitHub repository; ✗ when the vendor reports the key unauthorised or the repository not visible; ⚠ while a cool-down is active or a daily or concurrency limit is reached, naming when it ends; and ✓ otherwise. The check SHALL start no vendor session and SHALL send no message. When `[remote] vendor` is unset, the check SHALL NOT be reported, so the report is unchanged for a user without remote workers.

#### Scenario: A missing key is reported without a key

- **GIVEN** `[remote] vendor = "jules"` and `JULES_API_KEY` unset
- **WHEN** `git paw doctor` runs
- **THEN** it SHALL report ✗ for the remote check naming `JULES_API_KEY`

#### Scenario: A key with a line break is reported without the key

- **GIVEN** `[remote] vendor = "jules"` and `JULES_API_KEY` set to `test-key-abc123` followed by a carriage return
- **WHEN** `git paw doctor` runs
- **THEN** it SHALL report ✗ for the remote check naming `JULES_API_KEY`
- **AND** its output SHALL NOT contain `test-key-abc123`
- **AND** the fake vendor SHALL have recorded no call

#### Scenario: A ready repository passes without starting work

- **GIVEN** `[remote] vendor = "jules"`, the key set, a GitHub `origin` and a fake vendor that reports the repository visible
- **WHEN** `git paw doctor` runs
- **THEN** it SHALL report ✓ for the remote check
- **AND** the fake SHALL have recorded no session creation and no message

#### Scenario: No remote check without a vendor

- **GIVEN** a configuration with no `[remote] vendor`
- **WHEN** `git paw doctor` runs
- **THEN** no remote check SHALL appear in the report
