## ADDED Requirements

### Requirement: default_backend selects the terminal backend for new sessions

The configuration SHALL accept an optional top-level `default_backend` key whose accepted values are exactly `"tmux"` and `"headless"`; any other value SHALL fail loading with a configuration error naming the key and both accepted values. When absent, new sessions SHALL use the tmux backend. It SHALL merge like any other top-level scalar, the repository file's value winning over the global file's. It SHALL be serialized only when set, so a configuration without it round-trips byte-identically, and it SHALL NOT be written by `git paw init`'s default template except as a commented example. The configuration reference SHALL document the key, its values, its default and its precedence below `start --backend`.

#### Scenario: The key parses both values

- **GIVEN** a config with `default_backend = "headless"`
- **WHEN** the config is loaded
- **THEN** the effective default backend SHALL be headless

#### Scenario: An invalid value is refused

- **GIVEN** a config with `default_backend = "screen"`
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error naming `default_backend`, `tmux` and `headless`

#### Scenario: The repository value wins

- **GIVEN** a global config with `default_backend = "headless"` and a repository config with `default_backend = "tmux"`
- **WHEN** the configs are merged
- **THEN** the effective default backend SHALL be tmux

#### Scenario: An absent key round-trips unchanged

- **GIVEN** a configuration file with no `default_backend`
- **WHEN** it is loaded and saved again
- **THEN** the saved file SHALL contain no `default_backend` key

#### Scenario: The reference documents the key

- **WHEN** the configuration reference is read
- **THEN** it SHALL document `default_backend` with the values `tmux` and `headless` and its default
