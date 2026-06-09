# custom-cli-curl-seeding Specification

## Purpose
TBD - created by archiving change claude-oss-launch-v0-6-x. Update Purpose after archive.
## Requirements
### Requirement: Config-driven broker-curl seeding for custom CLIs

When the broker is enabled, the system SHALL seed the
broker-curl allowlist into each session CLI's configured
settings file, given by `[clis.<name>].settings_path`. The
seeding target is CONFIG-DRIVEN — the system SHALL NOT
hardcode any CLI's settings path or name. A leading `~` in
the configured path SHALL be expanded to the home directory.
This is in addition to the always-seeded repo-local
`.claude/settings.json`.

#### Scenario: Configured settings_path is seeded

- **GIVEN** `[clis.mycli].settings_path = "~/.mycli/settings.json"`
  with the `~/.mycli/` directory present, and a session using
  `mycli` with the broker enabled
- **WHEN** the session launches
- **THEN** the broker endpoints SHALL be seeded into
  `~/.mycli/settings.json` so the CLI's boot-time
  `curl .../publish` does not raise a permission prompt

#### Scenario: No hardcoded CLI name or path

- **WHEN** the seeding code is inspected
- **THEN** it SHALL NOT reference any specific CLI name or
  settings path; custom-CLI seeding targets come only from
  `[clis.<name>].settings_path`

#### Scenario: CLI without settings_path seeds nothing extra

- **GIVEN** a session CLI that has no `[clis.<name>]` entry, or
  one without `settings_path`
- **WHEN** the session launches
- **THEN** only the repo-local `.claude/settings.json` SHALL be
  seeded; no other settings file is written for that CLI

### Requirement: Never create a CLI's config directory

The system SHALL seed a configured `settings_path` only when
its parent directory already exists, mirroring the
dev-allowlist seeder's caution — git-paw SHALL NOT create a
CLI's config directory.

#### Scenario: Missing parent directory is skipped

- **GIVEN** `[clis.mycli].settings_path` whose parent
  directory does not exist
- **WHEN** the session launches
- **THEN** the system SHALL NOT create the directory and SHALL
  NOT write the settings file for that path

### Requirement: Seeding is idempotent, deduped, and non-fatal

Seeding SHALL be idempotent (re-seeding never duplicates
allowlist entries and preserves pre-existing entries), SHALL
seed each distinct settings path at most once per launch even
when supervisor and agent CLIs resolve to the same path, and
SHALL be non-fatal (a write failure logs a stderr warning and
session launch continues).

#### Scenario: Re-attach does not duplicate entries

- **GIVEN** a CLI whose configured settings file was already
  seeded
- **WHEN** seeding runs again on re-attach
- **THEN** the broker-endpoint entries SHALL appear exactly
  once and pre-existing unrelated entries SHALL remain

#### Scenario: Same path for supervisor and agent seeds once

- **GIVEN** the supervisor CLI and the agent CLI resolve to the
  same configured `settings_path`
- **WHEN** the session launches
- **THEN** that path SHALL be seeded exactly once

#### Scenario: Unwritable settings file warns and continues

- **GIVEN** a configured `settings_path` whose parent exists
  but the file cannot be written
- **WHEN** seeding attempts to run
- **THEN** the system SHALL emit a stderr warning and continue
  launching the session

