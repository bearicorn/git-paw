## MODIFIED Requirements

### Requirement: README Supervisor Mode quick start documents the flags

The README's Quick start SHALL link `docs/src/quick-start-orchestrator.md`, and that page SHALL document the `--no-supervisor` opt-out flag and the `start --force` flag for bypassing the uncommitted-spec validation warning, noting that `--force` has no effect under supervisor mode. Each flag SHALL appear at least once in a command-line example on that page.

#### Scenario: Quick start supervisor mentions --no-supervisor

- **WHEN** `docs/src/quick-start-orchestrator.md` is inspected
- **THEN** it contains the substring `--no-supervisor` in a command-line example

#### Scenario: Quick start supervisor mentions --force

- **WHEN** `docs/src/quick-start-orchestrator.md` is inspected
- **THEN** it contains the substring `--force` (in the context of `start --force`)
- **AND** it states that `--force` has no effect under supervisor mode
