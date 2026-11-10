## ADDED Requirements

### Requirement: Script-running tests SHALL clear the session broker address

A test that runs a bundled helper script SHALL build the command through the shared builder in `tests/helpers/script_cmd.rs`, which clears `GIT_PAW_BROKER_URL`, so a test run inside a live git-paw session never publishes to that session's broker. `TmuxTestEnv::apply` SHALL clear it too.

#### Scenario: A test never inherits the session's broker address

- **GIVEN** a test run inside a live git-paw session, whose environment sets `GIT_PAW_BROKER_URL` to the session's broker
- **WHEN** the test runs a bundled helper against its own broker
- **THEN** the helper SHALL publish to the test's broker and nothing SHALL reach the session's

Test: `tests/helper_publish_contract.rs::a_script_test_ignores_an_inherited_broker_url`
