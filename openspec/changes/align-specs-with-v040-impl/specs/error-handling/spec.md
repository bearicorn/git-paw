## MODIFIED Requirements

### Requirement: BrokerError variants with actionable messages

The system SHALL define a `BrokerError` type with variants for broker-specific failures. Each variant SHALL produce a user-facing message that explains the problem and suggests a remedy. `BrokerError` SHALL be wrappable inside `PawError` as a variant.

The following variants SHALL exist:

- `PortInUse { port: u16, source: std::io::Error }` — the configured port is already occupied; `source` carries the underlying bind/probe `io::Error` so callers can chain or log the original cause
- `ProbeTimeout { port: u16 }` — the stale-broker probe timed out
- `BindFailed(std::io::Error)` — socket bind failed for a reason other than port-in-use
- `RuntimeFailed(std::io::Error)` — tokio runtime construction failed

`PortInUse.source` SHALL be marked `#[source]` (or equivalent thiserror attribute) so it participates in `std::error::Error::source()` chains. The `Display` output SHALL NOT include the source by default — it is reserved for explicit chaining via `{:?}` or programmatic `.source()` access — to avoid duplicated diagnostics in user-facing CLI output.

#### Scenario: PortInUse is actionable

- **GIVEN** `BrokerError::PortInUse { port: 9119, source: io::Error::from(io::ErrorKind::AddrInUse) }`
- **WHEN** formatted with `Display`
- **THEN** the message SHALL mention port `9119` and suggest changing `[broker] port` in config
- **AND** the message SHALL NOT contain the underlying `io::Error` Display text

#### Scenario: PortInUse exposes underlying cause

- **GIVEN** a `PortInUse` value with an `AddrInUse` source
- **WHEN** `std::error::Error::source()` is called on it
- **THEN** the result SHALL be `Some(&dyn Error)` referencing the wrapped `io::Error`

#### Scenario: ProbeTimeout is actionable

- **GIVEN** `BrokerError::ProbeTimeout { port: 9119 }`
- **WHEN** formatted with `Display`
- **THEN** the message SHALL mention the port and suggest checking for stuck processes

#### Scenario: BrokerError exit code

- **GIVEN** any `BrokerError` variant wrapped in `PawError`
- **WHEN** `exit_code()` is called
- **THEN** it SHALL return `1`
