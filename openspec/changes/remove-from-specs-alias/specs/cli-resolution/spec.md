## MODIFIED Requirements

### Requirement: Prompt at most once

The system SHALL prompt the user for CLI selection at most once during a spec-driven launch (`--from-all-specs` or `--specs`), applying the choice to all branches without a `paw_cli` or `default_spec_cli`.

#### Scenario: Single prompt for remaining branches
- **WHEN** 5 specs are launched, 2 have `paw_cli`, and the picker fires for the remaining 3
- **THEN** the picker SHALL fire once and the chosen CLI SHALL be applied to all 3
