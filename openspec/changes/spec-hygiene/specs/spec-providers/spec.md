## MODIFIED Requirements

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

- **GIVEN** a configured `[specs]` section, and one fixture spec for each of the four formats
- **WHEN** `list_specs` reports a spec's `path`
- **THEN** the path SHALL be under the same directory the scan used
- **AND** the path, joined to the repository root, SHALL name an existing file or directory

#### Scenario: An if-let on a spec-format variant is caught

- **GIVEN** the line `if let SpecBackendKind::OpenSpec = kind {` in production code outside `src/specs/`
- **WHEN** the spec-format dispatch audit runs
- **THEN** it SHALL report that line

Test: `tests/source_audit.rs::spec_format_dispatch_happens_only_inside_specs`

#### Scenario: A match on a wrapped spec-format variant is caught

- **GIVEN** the line `Some(SpecBackendKind::OpenSpec) => 2,` in production code outside `src/specs/`
- **WHEN** the spec-format dispatch audit runs
- **THEN** it SHALL report that line

Test: `tests/source_audit.rs::spec_format_dispatch_happens_only_inside_specs`
