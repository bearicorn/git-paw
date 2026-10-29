## ADDED Requirements

### Requirement: `[remote] follow_hours` bounds git-paw's follow of a remote session

The configuration SHALL accept an optional `[remote]` table whose optional key `follow_hours` is a positive integer number of hours, defaulting to `4`, that sets the follow time cap of every remote worker. A value that is zero, negative or not an integer SHALL fail loading with a configuration error naming the key. Each key of `[remote]` SHALL merge independently, the repository file's value winning over the global file's. The table SHALL be serialized only when set, so a configuration without it round-trips byte-identically, and adding it SHALL NOT change the configuration schema version. The configuration reference SHALL document the key, its default and that reaching the cap ends git-paw's follow but not the vendor's session.

#### Scenario: The default applies

- **GIVEN** a configuration with no `[remote]` table
- **WHEN** it is loaded
- **THEN** the effective follow time cap SHALL be 4 hours

#### Scenario: A zero value is refused

- **GIVEN** a configuration with `[remote] follow_hours = 0`
- **WHEN** it is loaded
- **THEN** loading SHALL fail with an error naming `follow_hours`

#### Scenario: The repository value wins

- **GIVEN** a global configuration with `[remote] follow_hours = 8` and a repository configuration with `[remote] follow_hours = 2`
- **WHEN** the configurations are merged
- **THEN** the effective follow time cap SHALL be 2 hours

#### Scenario: An absent table round-trips unchanged

- **GIVEN** a configuration file with no `[remote]` table
- **WHEN** it is loaded and saved again
- **THEN** the saved file SHALL contain no `[remote]` table
