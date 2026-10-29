## ADDED Requirements

### Requirement: The dashboard is enabled by a `[dashboard]` key

The `[dashboard]` table SHALL accept an optional boolean `enabled`. When it is `true`, new sessions that run the broker get a dashboard pane, per `broker-runtime`'s "Start flow conditionally creates dashboard pane". When it is absent in a version-2 or later document, it SHALL default to `false`. The key SHALL be omitted on serialization when absent. It SHALL merge as part of the `[dashboard]` table, so a repository `[dashboard]` table replaces the global one. `show_message_log` SHALL still be accepted and SHALL have no effect. The configuration reference SHALL document `enabled`, its default, its need for the broker, and that `git paw start --dashboard` overrides it for one launch. The `git paw init` default config SHALL show it as a commented example.

#### Scenario: Absent key means no dashboard

- **GIVEN** a version-2 config with `[broker] enabled = true` and no `[dashboard]` table
- **WHEN** the config is loaded
- **THEN** the dashboard SHALL resolve as not enabled

#### Scenario: The key enables the dashboard

- **GIVEN** a version-2 config with `[dashboard] enabled = true`
- **WHEN** the config is loaded
- **THEN** the dashboard SHALL resolve as enabled

#### Scenario: The key round-trips

- **GIVEN** a config with `[dashboard] enabled = true` and `[dashboard.broker_log] height_lines = 30`
- **WHEN** the config is loaded and saved back
- **THEN** both values SHALL be preserved

#### Scenario: The legacy message-log key is accepted

- **GIVEN** a config with `[dashboard] show_message_log = true`
- **WHEN** the config is loaded
- **THEN** loading SHALL succeed and the dashboard's content SHALL be the same as without the key

### Requirement: Version-1 configs keep the dashboard they implied

The version-1 to version-2 configuration migration SHALL set `[dashboard] enabled = true` in a version-1 document that does not set that key itself and that either sets `[broker] enabled = true` or contains a `[dashboard]` table. A configuration that produced a dashboard before this requirement therefore still produces one. The rule SHALL be a pure function of the document. It SHALL apply in memory and SHALL NOT write the file. It SHALL NOT apply to a document that declares version 2 or later, which is read as written.

When git-paw writes `version = 2` into a version-1 file, or rewrites a version-1 file in the version-2 layout, it SHALL write `[dashboard] enabled = true` into that file whenever the rule implied it, so the stamped file keeps its meaning. These writes are `git paw init`'s version backfill and the `add-cli` / `remove-cli` rewrite. The configuration reference SHALL state this rule next to its migration table.

#### Scenario: A version-1 broker config keeps its dashboard

- **GIVEN** a config with no `version`, `[broker] enabled = true` and no `[dashboard]` table
- **WHEN** it is loaded
- **THEN** the dashboard SHALL resolve as enabled
- **AND** the file's bytes SHALL be unchanged

#### Scenario: A version-1 dashboard table implies the dashboard

- **GIVEN** a repository config with no `version` whose only dashboard setting is `[dashboard.broker_log] height_lines = 30`
- **WHEN** it is loaded
- **THEN** its `[dashboard]` table SHALL resolve with `enabled = true` and `height_lines = 30`

#### Scenario: A version-1 file without broker or dashboard settings implies nothing

- **GIVEN** a global config with no `version` that contains only `[clis.my-agent]`
- **WHEN** it is loaded
- **THEN** it SHALL contribute no `[dashboard]` table

#### Scenario: An explicit value in a version-1 file wins

- **GIVEN** a config with no `version`, `[broker] enabled = true` and `[dashboard] enabled = false`
- **WHEN** it is loaded
- **THEN** the dashboard SHALL resolve as not enabled

#### Scenario: A version-2 file is read as written

- **GIVEN** a config declaring `version = 2` with `[broker] enabled = true` and no `[dashboard] enabled`
- **WHEN** it is loaded
- **THEN** the dashboard SHALL resolve as not enabled

#### Scenario: Stamping a version-1 broker config writes the key

- **GIVEN** an existing `.git-paw/config.toml` with only `[broker] enabled = true`
- **WHEN** `git paw init` backfills `version = 2` into it
- **THEN** the file SHALL declare `version = 2` and `[dashboard] enabled = true`
- **AND** loading it SHALL resolve the dashboard as enabled
