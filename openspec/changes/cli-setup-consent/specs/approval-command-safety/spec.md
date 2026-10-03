## MODIFIED Requirements

### Requirement: No permission grants are seeded into agent CLI settings files

The system SHALL NOT automatically write permission rules, trust markers or hook configuration into any agent CLI's settings or state file, SHALL NOT automatically create a vendor-specific settings directory inside a repository or a worktree, and SHALL NOT automatically add a version-control exclusion entry for such a directory or file. Such writes SHALL happen only when the user explicitly runs `git paw setup-cli` (the `cli-setup` capability), and running it is the consent. No other command, process or interface — including `start`, `add`, `init`, session recovery, the drive loop, the dashboard and the MCP server — SHALL invoke it, apply its items, or write the setup record. Outside that command git-paw writes no permission rule of its own.

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

#### Scenario: Init writes no setup entry

- **GIVEN** a repository with no `.claude/` directory and no setup record
- **WHEN** `git paw init` runs
- **THEN** no `.claude/` directory and no setup record SHALL exist afterwards

#### Scenario: Earlier setup is not extended by a later launch

- **GIVEN** `git paw setup-cli claude --allowlist --yes` was applied in a repository
- **WHEN** `git paw add` creates a new worktree in that repository's session
- **THEN** no settings directory SHALL be created in the new worktree
- **AND** the main checkout's settings file and the setup record SHALL be byte-identical before and after

#### Scenario: Recovery and the drive loop write no setup entries

- **GIVEN** a repository with a setup record
- **WHEN** a stopped session is recovered and the drive loop runs to completion
- **THEN** the setup record and every file it names SHALL be byte-identical before and after

#### Scenario: No MCP tool applies setup

- **WHEN** the tool list of `git paw mcp` is read
- **THEN** no tool SHALL invoke `git paw setup-cli`, write an agent CLI's settings file or write the setup record
