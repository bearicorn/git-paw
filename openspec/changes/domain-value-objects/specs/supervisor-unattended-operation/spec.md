## ADDED Requirements

### Requirement: The per-pane claim TTL has one canonical definition mirrored by the helper

The time after which an untouched per-pane claim — the claim that guards the explicit `sweep.sh approve` action and the drive loop's nudge, stale-input recovery and buffered-input submit keystrokes — is treated as abandoned SHALL be defined once, by the binary's claim module, and the bundled `sweep.sh` helper's value SHALL be a declared mirror of it. A standing behavioral test SHALL verify the mirror: a claim file at the binary's claim path whose age is just below the TTL SHALL be treated as held by both the binary's acquirer and the helper, a claim whose age is just above the TTL SHALL be treated as abandoned by both, and the helper's claim path for a pane SHALL equal the binary's.

#### Scenario: A fresh claim is held for both senders

- **GIVEN** a claim file for pane 3 aged two seconds less than the TTL
- **WHEN** the binary's acquirer and the helper's abandonment check evaluate it
- **THEN** both SHALL treat the claim as held

#### Scenario: An expired claim is abandoned for both senders

- **GIVEN** a claim file for pane 3 aged two seconds more than the TTL
- **WHEN** the binary's acquirer and the helper's abandonment check evaluate it
- **THEN** both SHALL treat the claim as abandoned

#### Scenario: Both compute the same claim path

- **WHEN** the binary and the helper compute the claim path for pane 3 in the same repository
- **THEN** the two paths SHALL be equal
