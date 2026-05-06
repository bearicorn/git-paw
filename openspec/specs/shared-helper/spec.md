# shared-helper Specification

## Purpose
TBD - created by archiving change boot-prompt-standard. Update Purpose after archive.
## Requirements
### Requirement: Shared boot block helper function

The system SHALL provide a shared `build_boot_block()` function in `src/skills.rs` that can be called from both supervisor and manual mode code paths.

#### Scenario: Function is accessible from multiple modules

- **GIVEN** `build_boot_block()` defined in `src/skills.rs`
- **WHEN** called from `src/main.rs` (supervisor mode)
- **THEN** it SHALL return the boot block string

#### Scenario: Same function used in manual mode

- **GIVEN** `build_boot_block()` defined in `src/skills.rs`
- **WHEN** called from `src/tmux.rs` (manual mode)
- **THEN** it SHALL return the same boot block string

### Requirement: Helper function signature

The `build_boot_block()` function SHALL have the following signature:
```rust
pub fn build_boot_block(branch_id: &str, broker_url: &str) -> String
```

#### Scenario: Function accepts required parameters

- **WHEN** `build_boot_block("feat/errors", "http://localhost:9119")` is called
- **THEN** it SHALL accept both parameters without error

#### Scenario: Function returns boot block string

- **WHEN** `build_boot_block("feat/errors", "http://localhost:9119")` is called
- **THEN** it SHALL return a `String` containing the boot instructions

### Requirement: Helper function reusability

The `build_boot_block()` function SHALL be designed for maximum reusability with no dependencies on calling context or global state.

#### Scenario: Function is pure (no side effects)

- **GIVEN** same input parameters
- **WHEN** `build_boot_block()` is called multiple times
- **THEN** it SHALL return identical output each time

#### Scenario: Function requires no external state

- **WHEN** `build_boot_block()` is called
- **THEN** it SHALL not access any global variables, configuration, or external services
- **AND** it SHALL only use its input parameters

### Requirement: Helper function testing

The `build_boot_block()` function SHALL be fully testable with comprehensive unit test coverage.

#### Scenario: Function can be tested in isolation

- **WHEN** unit tests call `build_boot_block()` with various inputs
- **THEN** the function SHALL produce expected output without requiring tmux or broker

#### Scenario: Edge cases are testable

- **WHEN** tests provide edge case inputs (empty strings, special characters)
- **THEN** the function SHALL handle them gracefully

