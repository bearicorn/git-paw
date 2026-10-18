## ADDED Requirements

### Requirement: Session file records the authority credential

The `Session` struct SHALL include an optional field `broker_authority: Option<String>` annotated `#[serde(default, skip_serializing_if = "Option::is_none")]`, so session files written before this change load with `None` and a session without a broker serializes byte-identically to before. `save_session` SHALL write the session file with owner-only permissions (`0600`) on Unix. The per-repo discovery file (`RepoSessionFile`) SHALL NOT carry the credential.

#### Scenario: Credential round-trips

- **GIVEN** a `Session` with `broker_authority = Some("<hex>")` saved to disk
- **WHEN** the session is loaded back
- **THEN** `broker_authority` SHALL be `Some("<hex>")`

#### Scenario: Older session files load with None

- **GIVEN** a session JSON file with no `broker_authority` field
- **WHEN** the session is loaded
- **THEN** `broker_authority` SHALL be `None`
- **AND** loading SHALL succeed

#### Scenario: Session file is owner-only

- **WHEN** `save_session` writes a session file on Unix
- **THEN** the file's mode SHALL be `0600`

#### Scenario: Discovery file never carries the credential

- **GIVEN** a session with a credential
- **WHEN** the per-repo discovery file is written
- **THEN** it SHALL NOT contain a `broker_authority` key
