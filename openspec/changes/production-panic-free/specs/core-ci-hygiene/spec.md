## ADDED Requirements

### Requirement: Unit tests in the binary crate run under the default test and lint gates

The `git-paw` binary target SHALL be testable by default: `Cargo.toml` SHALL NOT set `test = false` on the `[[bin]]` target, so a plain `cargo test` compiles and runs every `#[cfg(test)]` module in `src/main.rs` and `src/commands/*.rs`, and `cargo clippy --all-targets` lints that test code. A unit test written beside binary-crate code SHALL therefore be exercised by `just check` and by CI exactly like a library-crate unit test.

#### Scenario: A failing binary-crate unit test fails cargo test

- **GIVEN** a `#[test]` in a `#[cfg(test)]` module of `src/commands/*.rs` that fails
- **WHEN** `cargo test` runs with no target-selection flags
- **THEN** the run SHALL report that test as failed and exit non-zero

#### Scenario: The binary target is not excluded from testing

- **WHEN** `Cargo.toml` is inspected
- **THEN** the `[[bin]]` entry for `git-paw` SHALL NOT contain `test = false`
