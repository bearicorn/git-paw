## REMOVED Requirements

### Requirement: README features section enumerates the current user-facing surface

**Reason**: The README becomes a landing page (see "README is a landing page that forwards detail to the documentation"). Enumerating every flag and config table in the README duplicated the CLI reference, the configuration reference and the user guide, and turned the Features section into a changelog.
**Migration**: Each item this requirement listed is documented where readers look it up: `--specs-format`, `--no-supervisor`, `start --force` in `docs/src/cli-reference.md`; `[specs] type = "speckit"`, `[governance]`, `[supervisor.conflict]`, `[supervisor.learnings_config]` in `docs/src/configuration/README.md`; `agent.intent`, conflict detection and learnings mode in their user-guide chapters. The README links to those chapters. Its rule that the README does not present automatic approval of permission prompts as a feature (v0.17.0) is carried into "README is a landing page that forwards detail to the documentation".

## ADDED Requirements

### Requirement: README is a landing page that forwards detail to the documentation

`README.md` SHALL be a landing page containing, in order of appearance: a short statement of what git-paw is and why it exists; a demo; installation instructions; exactly one quick start that reaches a first running session; a command table with one line per top-level subcommand; the Supported AI CLIs list; the security disclaimer; and a link to the documentation site for everything else. Its Features list, if present, SHALL contain at most 12 bullets, each naming a capability in one sentence and linking to the user-guide chapter that documents it. The README SHALL NOT document individual configuration tables or keys (no `[supervisor.` table names), SHALL NOT carry version-stamped feature descriptions, SHALL NOT present automatic approval of permission prompts as a feature, and SHALL NOT duplicate user-guide or reference content beyond the items above. A standing test SHALL enforce these properties.

#### Scenario: Required landing sections are present

- **WHEN** `README.md` is inspected
- **THEN** it SHALL contain an installation section, exactly one quick-start section, a command table, the Supported AI CLIs list, a `## Security` section, and a link to the documentation site

#### Scenario: Feature list is short and linked

- **WHEN** the README's Features section is inspected
- **THEN** it SHALL contain at most 12 bullets
- **AND** every bullet SHALL contain a link into the documentation

#### Scenario: No configuration-table documentation

- **WHEN** `README.md` is inspected
- **THEN** it SHALL NOT contain the substring `[supervisor.`

#### Scenario: No auto-approval feature

- **WHEN** the README's Features section is inspected
- **THEN** it SHALL NOT describe git-paw as auto-approving permission prompts

#### Scenario: Command table covers every subcommand

- **WHEN** the README command table is compared with the top-level subcommands `git paw --help` lists
- **THEN** every listed subcommand SHALL have exactly one row
