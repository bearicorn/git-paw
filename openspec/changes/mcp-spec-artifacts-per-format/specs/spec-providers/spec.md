## ADDED Requirements

### Requirement: Each spec backend locates its own spec

Each `SpecBackend` SHALL decide, for a scanned entry, where its spec lives under the specs directory, which file its tasks are read from, and which file its title is read from. Code outside `src/specs/` SHALL NOT build a spec's location by joining the specs directory and the entry id.

#### Scenario: Directory formats locate a directory

- **GIVEN** an OpenSpec change `add-auth` under `openspec/changes/`, and a Spec Kit feature `001-login` under `specs/` whose current phase decomposes into the entry `001-login-T002`
- **WHEN** each entry's spec location is resolved through its backend
- **THEN** the OpenSpec location SHALL be the directory `openspec/changes/add-auth`
- **AND** the Spec Kit location SHALL be the feature directory `specs/001-login`, not `specs/001-login-T002`
- **AND** each format's tasks file SHALL be `tasks.md` inside that directory

#### Scenario: Single-file formats locate the file

- **GIVEN** a Markdown spec `specs/fix-bug.md` and a Superpowers plan `plans/2026-07-20-add-auth.md`
- **WHEN** each entry's spec location is resolved through its backend
- **THEN** each location SHALL be that `.md` file
- **AND** each format's tasks file and title file SHALL be that same file

#### Scenario: A Spec Kit phase entry locates its feature

- **GIVEN** a Spec Kit feature `001-login` whose current phase yields the consolidated entry `001-login-phase-2`
- **WHEN** the entry's spec location is resolved
- **THEN** it SHALL be the feature directory `specs/001-login`

## MODIFIED Requirements

### Requirement: SpecBackend trait for format-specific scanning

The system SHALL define a `SpecBackend` trait carrying every behaviour that differs between spec formats, one implementation per format: identity and conventional directory, scanning a directory into `SpecEntry` results, the path doctrine, the task prompt, a spec's location and its title and tasks files, the artifacts served, task parsing, opsx role gating, and governance discovery. `scan` SHALL take a directory path and return a list of `SpecEntry` results.

#### Scenario: Backend returns discovered specs
- **WHEN** a `SpecBackend` implementation scans a directory with pending specs
- **THEN** it SHALL return a `Vec<SpecEntry>` with one entry per pending spec

#### Scenario: Backend returns empty list when no pending specs
- **WHEN** a `SpecBackend` implementation scans a directory with no pending specs
- **THEN** it SHALL return an empty `Vec`

#### Scenario: Per-format outputs are unchanged by the move
- **GIVEN** a fixture spec for each of the four formats
- **WHEN** its task prompt, path doctrine, title, artifact list and parsed tasks are produced through the backend
- **THEN** each SHALL equal the output produced before the behaviour moved into the trait, except the Markdown task prompt (per `core-lang-agnostic`) and the MCP spec path, title, artifacts and tasks that `mcp-spec-artifacts-per-format` corrected (per `mcp-server` "Spec read tools serve each format's own files")

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
