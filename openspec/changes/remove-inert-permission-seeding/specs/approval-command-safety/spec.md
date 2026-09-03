## ADDED Requirements

### Requirement: No permission grants are seeded into agent CLI settings files

The system SHALL NOT write permission grants into any agent CLI's settings file, SHALL NOT create a vendor-specific settings directory inside a worktree, and SHALL NOT add a version-control exclusion entry for such a directory. Prompt-free agent operation is provided by the CLI's own resolved permission mode together with git-paw's command classifier, which recognises the bundled helper scripts by their stable paths.

#### Scenario: No settings file is written at session start

- **GIVEN** a session starting with any agent CLI
- **WHEN** the session initialises
- **THEN** no permission grants SHALL be written into that CLI's settings file

#### Scenario: No vendor settings directory is created in a worktree

- **GIVEN** a worktree provisioned for any agent CLI
- **WHEN** the worktree is provisioned
- **THEN** no vendor-specific settings directory SHALL be created inside it
- **AND** no version-control exclusion entry SHALL be added for such a directory

#### Scenario: Pre-existing settings files are left untouched

- **GIVEN** a settings file that a previous version of git-paw wrote
- **WHEN** a session starts
- **THEN** that file SHALL be left byte-identical
- **AND** it SHALL NOT be deleted

### Requirement: Prompt-free operation is attributed to the permission mode and classifier

Documentation and bundled guidance SHALL attribute prompt-free agent operation to the CLI's resolved permission mode and git-paw's command classifier, and SHALL NOT attribute it to a seeded settings-file allowlist.

#### Scenario: Supervisor guidance does not credit a seeded allowlist

- **WHEN** the bundled supervisor skill's permission guidance is inspected
- **THEN** it SHALL NOT state that a seeded settings-file allowlist is why an agent's first broker call avoids a permission prompt
- **AND** it SHALL attribute prompt-free operation to the resolved permission mode and the classifier

### Requirement: Settings path remains available to memory isolation

Removing permission seeding SHALL NOT remove or alter the use of a CLI's configured settings path for memory isolation. The parent directory of a configured settings path SHALL continue to participate in the memory-isolation set.

#### Scenario: Memory isolation still uses the settings path parent

- **GIVEN** a configured CLI with a settings path
- **WHEN** the memory-isolation set is computed
- **THEN** the parent directory of that settings path SHALL still be included

#### Scenario: Settings path still parses and round-trips

- **WHEN** a configuration declaring a CLI settings path is loaded and re-serialized
- **THEN** the settings path SHALL be preserved

## REMOVED Requirements

### Requirement: Curl allowlist setup

**Reason**: The seeding wrote the key `allowed_bash_prefixes`, which the target CLI does not read (it reads `permissions.allow`), so the grant never took effect on any CLI. The key is git-paw's own invention and was read back only by git-paw's tests. The requirement described machinery that was inert in practice, and it caused vendor-specific directories to be created for CLIs that have no such surface.

**Migration**: Prompt-free broker calls are provided by the CLI's resolved permission mode (see `unattended-boot-hardening`) together with git-paw's command classifier, which recognises the bundled helper scripts by their stable path. The operating guidance that replaces this mechanism is exported as agnostic skills — worktree orientation in the coordination skill and the tiered permission model in the supervisor skill (see `worker-guidance-skill-export`). No successor seeding mechanism is introduced. A user who maintains their own allowlist by hand is unaffected: git-paw no longer writes to it, and existing files are left in place rather than deleted.

### Requirement: Allowlist file format

**Reason**: The requirement mandated writing to a single vendor's settings file (`.claude/settings.json`) using a key that vendor does not read. It hard-coded one CLI's layout into a capability meant to be CLI-neutral, and it is superseded by the removal of seeding altogether.

**Migration**: No file is written, so no format is mandated. If real permission seeding is wanted in future it requires its own proposal, because writing live permission grants into a user's settings file is a materially larger security decision than the inert write being removed here.
