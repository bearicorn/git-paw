## ADDED Requirements

### Requirement: User-facing surfaces carry no retired supervisor vocabulary

The word "supervisor" (in any letter case) SHALL NOT appear in: the `--help` output of `git paw` and of every non-hidden subcommand; `README.md`; the pages under `docs/src/` other than the configuration reference's version-1 migration table, the changelog page and the stability chapter's list of removals; the files under `assets/`; the generated default config; the MCP server's instructions and tool descriptions; or the output of `git paw doctor` for a repository whose configuration uses only version-2 keys. A standing test SHALL scan each of these and fail naming the file or command and the offending line. The `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]` tables, deprecated with no effect since v0.17.0 and dropped by the version-1 to version-2 migration, SHALL be named in the user docs only as rows of that migration table; no other page, including the rest of the configuration reference, SHALL name them. Requirement text under `openspec/specs/` SHALL use the coordinated-mode vocabulary except in requirements that name a removed spelling in order to reject or migrate it (including *Deprecated auto-approval tables are parsed but have no effect*), which the test SHALL allow by name. Rust module, type and function names and source or test paths (for example `src/supervisor/drive.rs`, `cmd_supervisor`, or a `Test:` annotation naming one) are internal and are not renamed; the scan of `openspec/specs/` SHALL ignore backticked code identifiers and paths and `Test:` annotation lines.

#### Scenario: The help output is clean

- **WHEN** `git paw --help` and each non-hidden subcommand's `--help` are run
- **THEN** none of the outputs SHALL contain `supervisor`

#### Scenario: A reintroduced mention fails the guard

- **GIVEN** a change that adds the word "supervisor" to `assets/agent-skills/coordination.md`
- **WHEN** the guard runs
- **THEN** it SHALL fail naming that file and line

#### Scenario: The migration table is allowed

- **WHEN** the guard scans `docs/src/configuration/README.md`
- **THEN** the version-1 migration table's `[supervisor]` paths SHALL NOT fail it

#### Scenario: The deprecated auto-approval tables appear only in the migration table

- **GIVEN** the configuration reference's version-1 migration table lists `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]` as dropped (deprecated 0.17.0, no effect)
- **AND** a change that adds a sentence naming `[supervisor.auto_approve]` to another section of `docs/src/configuration/README.md`
- **WHEN** the guard runs
- **THEN** the migration-table rows SHALL NOT fail it
- **AND** the added sentence SHALL fail it, naming the file and line

#### Scenario: Internal code names quoted in specs are allowed

- **GIVEN** a requirement under `openspec/specs/` whose text cites `` `src/supervisor/drive.rs` `` and a `Test:` line naming `supervisor::drive::tests::…`
- **WHEN** the guard scans `openspec/specs/`
- **THEN** neither occurrence SHALL fail it
- **AND** the same word in that requirement's prose outside backticks SHALL fail it
