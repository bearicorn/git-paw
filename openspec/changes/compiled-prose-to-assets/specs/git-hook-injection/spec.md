## ADDED Requirements

### Requirement: Hook script bodies are sourced from bundled assets

The shell bodies of the git hooks git-paw installs SHALL be authored in bundled asset
files and embedded at compile time, rather than assembled from string literals in
compiled code. This places them on the same footing as the other bundled shell helpers,
where they are visible to the export-agnosticism audits.

The hook **installation mechanism** SHALL remain compiled: marker-delimited block
chaining, executable-permission handling, and common-versus-linked gitdir resolution are
correctness mechanisms, not wording, and SHALL NOT move into an editable asset.

The installed hooks' observable behaviour SHALL be unchanged by this relocation.

#### Scenario: Hook bodies live in assets

- **WHEN** the sources of the installed hook scripts are inspected
- **THEN** their shell bodies SHALL be sourced from bundled assets
- **AND** SHALL NOT be assembled from string literals in compiled code

#### Scenario: Installation mechanism stays compiled

- **WHEN** a hook is installed
- **THEN** marker-delimited block chaining, permission handling, and gitdir resolution SHALL be performed by compiled code

#### Scenario: Installed hook behaviour is unchanged

- **GIVEN** hooks installed from bundled assets
- **WHEN** a commit, push, or cross-worktree commit attempt occurs
- **THEN** the observable hook behaviour SHALL be identical to the behaviour before the bodies were relocated

#### Scenario: Existing user hook content is still preserved

- **GIVEN** a common git dir that already has a hook file with user content
- **WHEN** git-paw installs its block from the bundled asset
- **THEN** the user's content SHALL be preserved
- **AND** only the git-paw block between the markers SHALL be replaced on re-install

#### Scenario: Hook assets are covered by the export audit

- **WHEN** the export-agnosticism audit runs
- **THEN** the bundled hook assets SHALL be included in the audited set
