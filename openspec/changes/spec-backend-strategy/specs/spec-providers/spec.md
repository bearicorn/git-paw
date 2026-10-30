## MODIFIED Requirements

### Requirement: SpecBackend trait for format-specific scanning

The system SHALL define a `SpecBackend` trait that carries every behaviour that differs between spec formats, with one implementation per format: the format's identity and conventional directory, scanning a directory into `SpecEntry` results, the spec-path doctrine sentence, the per-agent task prompt for an entry, the artifact whose first heading titles a spec, the artifact list served for a spec, task-list parsing, whether the format's workflow is subject to opsx role gating, and discovery of the format's governance documents. The `scan` method SHALL take a directory path and return a list of `SpecEntry` results.

#### Scenario: Backend returns discovered specs
- **WHEN** a `SpecBackend` implementation scans a directory with pending specs
- **THEN** it SHALL return a `Vec<SpecEntry>` with one entry per pending spec

#### Scenario: Backend returns empty list when no pending specs
- **WHEN** a `SpecBackend` implementation scans a directory with no pending specs
- **THEN** it SHALL return an empty `Vec`

#### Scenario: Per-format outputs are unchanged by the move
- **GIVEN** a fixture spec for each of the four formats
- **WHEN** its task prompt, path doctrine, title, artifact list and parsed tasks are produced through the backend
- **THEN** each SHALL equal the output produced before the behaviour moved into the trait, except the Markdown task prompt (per `core-lang-agnostic`)

## ADDED Requirements

### Requirement: One spec-format type at every boundary

The system SHALL represent a spec format inside git-paw by `SpecBackendKind` alone. The `[specs] type` configuration string SHALL be parsed into it where it is read, the `--specs-format` value SHALL be converted into it, and the MCP `backend` strings SHALL be rendered from it. The set of accepted names SHALL be defined once, by `SpecBackendKind`, and SHALL be what the unknown-type error lists. The configuration and command-line surfaces SHALL be unchanged: the same accepted values, the same `--specs-format` rejection message, and the same "unknown spec type" error.

#### Scenario: Every accepted name round-trips

- **WHEN** each accepted format name is parsed and rendered back
- **THEN** the result SHALL equal the original name
- **AND** the MCP server's set of valid `[specs] type` values SHALL equal the accepted set

#### Scenario: An unknown type keeps its error

- **GIVEN** `[specs] type = "unknown"`
- **WHEN** spec scanning runs
- **THEN** it SHALL fail with `PawError::SpecError` naming `unknown` and listing the accepted names

### Requirement: Per-format behaviour is reached only through the backend

Code outside `src/specs/` SHALL obtain per-format behaviour by asking the entry's or the session's `SpecBackendKind` for its backend. It SHALL NOT compare a spec-type string, and SHALL NOT match on `SpecBackendKind` except through the single mapping from a kind to its backend. Adding a format SHALL therefore require only a new variant, its backend mapping, and its backend implementation.

#### Scenario: opsx role gating follows the backend

- **GIVEN** a session whose spec format is OpenSpec, and another whose format is Spec Kit
- **WHEN** the coordination skill is rendered and the opsx role-gating guard is configured for each
- **THEN** the opsx regions and the guard SHALL be active for the OpenSpec session only

#### Scenario: Spec Kit governance discovery follows the backend

- **GIVEN** `[specs] type = "speckit"` with a `memory/constitution.md` beside the specs directory and no `[governance] constitution`
- **WHEN** the configuration is loaded
- **THEN** the constitution SHALL be wired from the Spec Kit backend's discovery, as before

#### Scenario: MCP spec paths use the scan's directory

- **GIVEN** a configured `[specs]` section
- **WHEN** `list_specs` reports a spec's `path`
- **THEN** the path SHALL be under the same directory the scan used
