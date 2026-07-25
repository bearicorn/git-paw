## ADDED Requirements

### Requirement: Consolidation preserves every OpenSpec scenario's coverage

Test consolidation SHALL preserve the coverage of every OpenSpec scenario: after any
consolidation wave, each WHEN/THEN scenario in `openspec/specs/` SHALL retain at least one
covering test, verified against the requirement→test map. A consolidation SHALL NOT be the
cause of any scenario reaching zero covering tests; if a removal would drop the last test
covering a scenario, that test is a sole guard and SHALL be restored (as a table row if
needed) rather than removed.

#### Scenario: A wave leaves no scenario uncovered

- **GIVEN** a requirement→test map captured before a consolidation wave
- **WHEN** the wave lands
- **THEN** the requirement→test map recomputed after the wave SHALL show every scenario with at least one covering test
- **AND** no scenario that had a covering test before the wave SHALL have zero covering tests after it

#### Scenario: A sole-guard removal is caught and restored

- **GIVEN** a proposed cut that would remove the only test covering a real behavioral branch
- **WHEN** the before/after requirement→test map is diffed
- **THEN** the dropped coverage SHALL be detected
- **AND** the guard SHALL be restored (as a table row if needed) before the wave merges

### Requirement: Tests remain behavioral

Consolidated and rewritten tests SHALL assert observable behavior — inputs → outputs, public
API contracts, error conditions, and the CLI / wire / file / TOML surfaces — and SHALL NOT
assert internal structure such as private field values, source layout, function call counts,
or mock interactions. A source-grep or brace-walk introspection test encountered during
consolidation SHALL be replaced by a behavioral test, not merely deleted.

#### Scenario: A source-grep introspection test is replaced, not deleted

- **GIVEN** a test that asserts source structure (e.g. `include_str!` + brace-walking, or "function X exists in file Y")
- **WHEN** the suite is consolidated
- **THEN** it SHALL be replaced by a test asserting the corresponding observable behavior
- **AND** the behavior it pinned SHALL still have a covering test afterward

#### Scenario: A retained test survives an internal rename

- **GIVEN** a test kept or rewritten during consolidation
- **WHEN** an internal function it exercises is renamed without changing behavior
- **THEN** the test SHALL still pass (it asserts behavior, not structure)

### Requirement: One-per-variant clusters are expressed as table-driven tests

A cluster of one-test-per-{variant, field, flag, bucket} SHALL be expressed as a single
table-driven test with one row per behavioral rule, rather than as many near-identical
tests. Each distinct behavioral rule the original cluster covered SHALL retain its own row;
table-ification SHALL NOT collapse distinct rules into fewer cases when doing so would drop a
rule's coverage.

#### Scenario: A per-variant battery becomes one table

- **GIVEN** a battery of near-identical one-test-per-variant cases (e.g. per-getter, per-flag, per-default)
- **WHEN** the cluster is consolidated
- **THEN** it SHALL become a single table-driven test
- **AND** the table SHALL contain one row for each variant the battery covered

#### Scenario: Distinct rules keep distinct rows

- **GIVEN** a cluster whose members encode distinct behavioral rules (e.g. per-normalization-rule region tests)
- **WHEN** the cluster is table-ified
- **THEN** each distinct rule SHALL retain its own row
- **AND** collapsing distinct rules into fewer arithmetic cases SHALL NOT occur

### Requirement: Sole-guard parity and prose tests are protected

Consolidation SHALL treat the `sweep_sh_*` parity suite (which guards the shipped bundled
bash artifact, a different artifact from the Rust classifier) and the `*_skill_content` /
prose-pin tests (which guard prose-only spec scenarios) as sole guards, and SHALL NOT delete
them. A brittle prose pin SHALL be rewritten to stable-anchor or keyword-set assertions
rather than removed, and the `sweep_sh_*` suite SHALL only be merged intra-file, each
retained row preserving its spec section, and SHALL NOT be cross-cut against the Rust
classifier tests.

#### Scenario: A brittle prose pin is rewritten, not deleted

- **GIVEN** a `*_skill_content` test asserting an exact substring of a bundled asset
- **WHEN** the suite is consolidated
- **THEN** the test SHALL be rewritten to assert a stable anchor or keyword set
- **AND** the prose-only scenario it guards SHALL still have a covering test

#### Scenario: The sweep.sh parity suite is preserved

- **GIVEN** the `sweep_sh_*` parity suite guarding the shipped `.git-paw/scripts/sweep.sh`
- **WHEN** the suite is consolidated
- **THEN** no `sweep_sh_*` test SHALL be deleted as a duplicate of the Rust classifier tests
- **AND** any merge SHALL be intra-file with each retained row still mapping to its spec section

### Requirement: Coverage stays at or above the pre-consolidation baseline

Line coverage SHALL remain at or above the pre-consolidation baseline at every wave gate. The
baseline SHALL be recorded once before the first wave, and each wave's gate SHALL confirm
`just coverage` reports a value greater than or equal to that baseline before the wave
merges.

#### Scenario: A wave gate confirms coverage is not below baseline

- **GIVEN** a recorded pre-consolidation coverage baseline
- **WHEN** a consolidation wave reaches its gate
- **THEN** `just coverage` SHALL report line coverage greater than or equal to the baseline
- **AND** the wave SHALL NOT merge until this holds

### Requirement: Prompt-surface tests are cut only after their PTY replacement exists

Tests subsumed by the `cli-interaction-e2e` PTY matrix SHALL NOT be removed until that PTY
matrix is merged and green. Consolidation SHALL sequence the subsumed prompt-surface tests
into a final wave that starts only after the PTY replacement exists, so no prompt guard is
removed before its behavioral replacement is in place. Argument-parsing tests, which the PTY
matrix does not cover, are exempt and MAY be consolidated in earlier waves.

#### Scenario: A subsumed prompt test is deferred until W1 is green

- **GIVEN** a prompt-surface test the `cli-interaction-e2e` PTY matrix subsumes
- **WHEN** consolidation waves are sequenced
- **THEN** that test SHALL be cut only in a wave that begins after the PTY matrix is merged and green

#### Scenario: Argument-parsing tables are not deferred

- **GIVEN** a clap argument-parsing flag/help cluster (not a prompt)
- **WHEN** the safe first wave runs
- **THEN** the cluster MAY be table-ified in that wave without waiting for the PTY matrix
