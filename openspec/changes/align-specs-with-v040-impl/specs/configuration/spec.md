## MODIFIED Requirements

### Requirement: Specs configuration section

The system SHALL support an optional `[specs]` section with a `dir` field and a `type` field. Field names SHALL match the `spec-scanning` capability and the implementation in `src/config.rs::SpecsConfig`.

- `dir: String` — path (relative to the repo root) to the directory containing spec files
- `type: String` — backend identifier (e.g. `"openspec"`, `"markdown"`); the field is exposed as `spec_type` in Rust to avoid clashing with the `type` keyword and is serialised as `type` in TOML/JSON via `#[serde(rename = "type")]`

When the `[specs]` section is absent, the optional `specs` field on `PawConfig` SHALL be `None`.

#### Scenario: Specs section with all fields

- **GIVEN** a TOML file with `[specs]` containing `dir = "openspec/specs"` and `type = "openspec"`
- **WHEN** the file is loaded
- **THEN** `specs.dir` SHALL be `"openspec/specs"`
- **AND** `specs.spec_type` SHALL be `"openspec"`

#### Scenario: Specs section defaults

- **GIVEN** a TOML file without a `[specs]` section
- **WHEN** the file is loaded
- **THEN** `specs` SHALL be `None`

#### Scenario: Round-trip preserves rename

- **GIVEN** a `SpecsConfig { dir: "openspec/specs".into(), spec_type: "openspec".into() }`
- **WHEN** the value is serialised to TOML and parsed back
- **THEN** the resulting TOML SHALL contain `type = "openspec"` (not `spec_type`)
- **AND** parsing SHALL succeed and reproduce the original struct
