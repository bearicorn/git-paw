## ADDED Requirements

### Requirement: BrokerError variants with actionable messages

The system SHALL define a `BrokerError` type with variants for broker-specific failures. Each variant SHALL produce a user-facing message that explains the problem and suggests a remedy. `BrokerError` SHALL be wrappable inside `PawError` as a variant.

The following variants SHALL exist:

- `PortInUse { port: u16 }` — the configured port is already occupied
- `ProbeTimeout { port: u16 }` — the stale-broker probe timed out
- `BindFailed(std::io::Error)` — socket bind failed for a reason other than port-in-use
- `RuntimeFailed(std::io::Error)` — tokio runtime construction failed

#### Scenario: PortInUse is actionable
- **GIVEN** `BrokerError::PortInUse { port: 9119 }`
- **WHEN** formatted with `Display`
- **THEN** the message SHALL mention port `9119` and suggest changing `[broker] port` in config

#### Scenario: ProbeTimeout is actionable
- **GIVEN** `BrokerError::ProbeTimeout { port: 9119 }`
- **WHEN** formatted with `Display`
- **THEN** the message SHALL mention the port and suggest checking for stuck processes

#### Scenario: BrokerError exit code
- **GIVEN** any `BrokerError` variant wrapped in `PawError`
- **WHEN** `exit_code()` is called
- **THEN** it SHALL return `1`
