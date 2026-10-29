## ADDED Requirements

### Requirement: Repository listing reports each repository's CLI profile

In all-repos mode each `list_repos` entry whose `available` is true SHALL carry a `profile` object with `default_cli` and `default_spec_cli`, resolved as *Repository profile resolution* specifies. An entry whose `available` is false SHALL NOT carry a `profile`. Adding `profile` SHALL leave every other `list_repos` field and the entry order unchanged.

#### Scenario: Available entry carries its profile

- **GIVEN** an all-repos server and a registered, available repository whose configuration sets `default_cli = "claude"`
- **WHEN** the client calls `list_repos()`
- **THEN** that entry SHALL carry `profile.default_cli` equal to `claude` and a `profile.default_spec_cli` field
- **AND** `profile` SHALL NOT contain a `trust` field

#### Scenario: Unavailable entry carries no profile

- **GIVEN** a registered repository whose directory has been deleted
- **WHEN** the client calls `list_repos()`
- **THEN** its entry SHALL have `available: false` and SHALL NOT contain a `profile` field

#### Scenario: Existing listing fields are unchanged

- **GIVEN** the same registry before and after this change
- **WHEN** the client calls `list_repos()`
- **THEN** every entry's `id`, `path`, `available`, `home` and `reason` values, and the entry order, SHALL be the same as without the profile
