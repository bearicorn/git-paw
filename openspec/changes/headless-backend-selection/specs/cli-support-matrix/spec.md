## ADDED Requirements

### Requirement: The matrix records headless-worker support outside the tiers

Every support-matrix row SHALL record one further status, `headless-worker` — `verified`, `gap` (with a non-empty note) or `unverified` — stating whether the CLI can run as a headless worker: its turn plan's first and continuation commands run non-interactively, a second turn continues the first turn's conversation, and an action outside the CLI's permission policy at `auto` is refused with the turn ending rather than waiting for input. `headless-worker` SHALL NOT take part in deriving a row's tier. It SHALL be `verified` only with evidence at the row's version: a reviewed fixture of help text containing every token of the turn plans the CLI's built-in rows render at `auto` and `full-auto`, and dated attestations of the continued conversation and of the refused action. The Supported AI CLIs page and `git paw doctor`'s tier report SHALL show each row's `headless-worker` status, and the docs-page parity test SHALL cover it.

#### Scenario: Headless-worker does not change a tier

- **GIVEN** a row whose twelve lifecycle steps are all `verified` and whose `headless-worker` is `unverified`
- **WHEN** its tier is derived
- **THEN** the tier SHALL be first-class

#### Scenario: A verified status needs its evidence

- **GIVEN** a row that records `headless-worker` as `verified`
- **AND** its fixture set has no `headless-worker` help-text fixture or no attestation
- **WHEN** the matrix parity suite runs
- **THEN** it SHALL fail, naming the row and `headless-worker`

#### Scenario: The docs page shows the status

- **GIVEN** a row recording `headless-worker` as `gap`
- **AND** the Supported AI CLIs page shows that CLI's headless status as verified
- **WHEN** the parity test runs
- **THEN** it SHALL fail, naming the binary and both statuses
