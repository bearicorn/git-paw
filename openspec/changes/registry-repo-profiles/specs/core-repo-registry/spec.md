## ADDED Requirements

### Requirement: Repository profile resolution

The system SHALL resolve a profile for an available registry entry from that repository's merged configuration.

The profile SHALL carry:

- `default_cli`, from the merged configuration, or null when unset;
- `default_spec_cli`, from the merged configuration, or null when unset.

Each value SHALL be the CLI id as configured (the `[clis.<name>]` key or known binary name), without resolving it on PATH. The profile SHALL NOT carry any folder-trust state. The registry file SHALL NOT store any part of the profile. A profile SHALL always reflect the repository's configuration at the time it is resolved.

#### Scenario: Profile reflects the repository configuration

- **GIVEN** an available registered repository whose configuration sets `default_cli = "claude"` and leaves `default_spec_cli` unset
- **WHEN** the repository's profile is resolved
- **THEN** the profile SHALL have `default_cli` equal to `claude` and a null `default_spec_cli`
- **AND** the profile SHALL carry no trust field

#### Scenario: Configuration changes are reflected without re-registration

- **GIVEN** a registered repository whose `default_cli` is changed from `claude` to `codex` after registration
- **WHEN** the repository's profile is resolved, with no `git paw init` run since the change
- **THEN** the profile's `default_cli` SHALL be `codex`

#### Scenario: The registry stores no profile data

- **WHEN** `git paw init` registers a repository whose configuration sets `default_cli`
- **THEN** the repository's registry entry SHALL contain no CLI name and no trust field

### Requirement: Profile resolution has no side effects

Resolving a repository profile SHALL NOT write any CLI or git-paw configuration file, or the registry, and SHALL NOT read any agent CLI's own state. It SHALL NOT spawn any agent CLI process.

#### Scenario: Resolution writes nothing

- **GIVEN** a registered repository with a configuration file
- **WHEN** the repository's profile is resolved
- **THEN** the registry file and the repository's configuration file SHALL be byte-identical to before

#### Scenario: No agent CLI is spawned

- **WHEN** profiles are resolved for every registered repository
- **THEN** no child process whose argv[0] is an agent CLI binary SHALL be spawned
