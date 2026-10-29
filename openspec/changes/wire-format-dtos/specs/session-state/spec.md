## ADDED Requirements

### Requirement: The session file is a versioned wire document separate from the session model

The global session file SHALL be serialized from and deserialized into a dedicated wire type, separate from the in-memory `Session` / `WorktreeEntry` model, through total mappers in both directions; the model SHALL NOT derive `Serialize` or `Deserialize`. The wire type SHALL carry a top-level integer `schema_version`, written as 1 and treated as 1 when absent, serialized before every other key. Apart from that key the written bytes SHALL be identical to the pre-change serialization of the same session, including the `created_at` timestamp format and every omission rule for optional fields.

#### Scenario: A pre-change session file loads unchanged

- **GIVEN** a session file written before this change (no `schema_version`)
- **WHEN** it is loaded
- **THEN** loading SHALL succeed and every field SHALL equal what the pre-change loader produced

#### Scenario: A saved session declares schema version 1

- **WHEN** `save_session()` writes a session
- **THEN** the file SHALL contain `"schema_version": 1` as its first key
- **AND** the remainder SHALL equal the pre-change bytes for the same session

#### Scenario: Model and wire round-trip

- **GIVEN** a fully populated session with broker fields, a dashboard pane, and worktree entries carrying every optional field
- **WHEN** it is saved and loaded back
- **THEN** the loaded session SHALL equal the original

### Requirement: Session files from a newer schema are neither ignored nor overwritten

When the session lookup for a repository finds a session file whose recorded repository matches and whose `schema_version` is greater than the binary supports, the lookup SHALL fail with a session error naming the file, the declared version, and the supported version, and telling the user to upgrade git-paw; it SHALL NOT treat the session as absent. A newer-schema file belonging to a different repository SHALL be skipped as before. Saving a session SHALL refuse, with the same error, to replace an existing file that declares a newer `schema_version`.

#### Scenario: A newer session file for this repository blocks the lookup

- **GIVEN** a session file for the current repository declaring `"schema_version": 2` and a binary supporting version 1
- **WHEN** `git paw status` is run
- **THEN** the command SHALL fail with an error naming the file and both versions and saying to upgrade git-paw
- **AND** it SHALL NOT report that no session exists

#### Scenario: start does not overwrite a newer session file

- **GIVEN** the same newer-schema session file
- **WHEN** `git paw start` is run
- **THEN** the command SHALL fail before launching anything
- **AND** the session file SHALL be byte-identical afterwards

#### Scenario: Another repository's newer file is skipped

- **GIVEN** a newer-schema session file belonging to a different repository and none for the current one
- **WHEN** the current repository's session is looked up
- **THEN** the lookup SHALL return no session without error

### Requirement: The per-repo discovery file declares its schema version

The per-repo discovery file (`.git-paw/sessions/<session>.json`) SHALL carry a top-level integer `schema_version` written as 1; a file without it SHALL be read as version 1. Every existing key (`session_name`, `agents[]` with `branch_id`, `worktree_path`, `cli`, `pane_index`, and the optional orchestrator entry) SHALL keep its name, type, and meaning, so the bundled `sweep.sh` reads the file unchanged.

#### Scenario: The discovery file is versioned

- **WHEN** `git paw start` writes the per-repo discovery file
- **THEN** it SHALL contain `"schema_version": 1` alongside the existing keys

#### Scenario: sweep.sh still discovers agents

- **GIVEN** a versioned discovery file
- **WHEN** `sweep.sh` reads it
- **THEN** it SHALL resolve the session name and agent list exactly as for an unversioned file
