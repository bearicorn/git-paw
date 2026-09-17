## MODIFIED Requirements

### Requirement: AGENTS.md dependency table matches `Cargo.toml`

`AGENTS.md`'s approved-dependencies table SHALL list every
production dependency declared in `[dependencies]` of `Cargo.toml`
at archive time and every dev dependency declared in
`[dev-dependencies]`. The production additions SHALL be
present: `schemars`, `chrono`, and `regex` (with
appropriate version suffixes matching the manifest). A crate removed
from `Cargo.toml` SHALL be removed from the table in the same change.

#### Scenario: AGENTS.md table lists prod dependencies

- **WHEN** the AGENTS.md Dependencies table is inspected
- **THEN** it contains the substring `schemars`
- **AND** it contains the substring `chrono`
- **AND** it contains the substring `regex`

#### Scenario: A removed dependency leaves the table

- **GIVEN** `serde_yaml` is not declared in `Cargo.toml`
- **WHEN** the AGENTS.md approved-dependencies table is inspected
- **THEN** it SHALL NOT contain a row for `serde_yaml`

### Requirement: AGENTS.md dependency table SHALL list the dependencies and note `dirs` as intentionally absent

The top-level `AGENTS.md` "Dependencies" table SHALL include rows for `schemars`, `chrono`, and `regex`. Each row's "Purpose" cell SHALL be one sentence (e.g. `schemars` → "JSON Schema derivation for governance config validation").

The `dirs` row SHALL be moved out of the approved-dependencies table and into a "Notable exclusions" sub-section beneath the table. The exclusion entry SHALL state: "Replaced by a homegrown `src/dirs.rs` because the upstream crate's license is not FOSS-compatible. Do not re-add."

The "Notable exclusions" sub-section SHALL also carry a `serde_yaml` entry stating that it was replaced by a dependency-free skill-frontmatter parser because the crate is deprecated upstream and depends on the unmaintained `unsafe-libyaml` (RUSTSEC-2024-0449), and that it SHALL NOT be re-added.

#### Scenario: AGENTS.md lists schemars in dependency table

- **WHEN** `AGENTS.md` is read
- **THEN** the dependencies table SHALL contain a row with `schemars` in the Crate column
- **AND** the Purpose column for that row SHALL mention JSON Schema or governance config

#### Scenario: AGENTS.md lists chrono and regex

- **WHEN** the dependencies table is inspected
- **THEN** rows for `chrono` and `regex` SHALL exist
- **AND** each row's Purpose cell SHALL be non-empty

#### Scenario: AGENTS.md notes `dirs` as intentionally absent

- **WHEN** `AGENTS.md` is read
- **THEN** the `dirs` row SHALL NOT appear in the approved-dependencies table
- **AND** a separate "Notable exclusions" section SHALL describe `dirs` as replaced by `src/dirs.rs` due to a non-FOSS license

#### Scenario: AGENTS.md notes `serde_yaml` as intentionally absent

- **WHEN** `AGENTS.md` is read
- **THEN** no `serde_yaml` row SHALL appear in the approved-dependencies table
- **AND** the "Notable exclusions" section SHALL describe `serde_yaml` as replaced by a dependency-free frontmatter parser, citing its deprecation and the unmaintained `unsafe-libyaml` advisory
