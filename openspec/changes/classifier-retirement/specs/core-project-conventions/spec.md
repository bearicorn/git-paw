## MODIFIED Requirements

### Requirement: README features section enumerates the current user-facing surface

`README.md`'s Features section SHALL include entries describing
every user-facing capability so users can
discover the feature set from a single page. The Features list
SHALL include (at minimum) entries for: `--specs-format`,
`--no-supervisor`, `start --force`, the Spec Kit backend
(`[specs] type = "speckit"`), the `[governance]` config table,
the `[supervisor.conflict]` config table, the
`[supervisor.learnings_config]` config table, `agent.intent`
forward coordination, automatic conflict detection
(forward / in-flight / ownership), and learnings mode (the
`.git-paw/session-learnings.md` output file). The Features list
SHALL NOT present automatic approval of permission prompts as a
feature.

#### Scenario: README Features section mentions Spec Kit

- **WHEN** the README's Features section is inspected
- **THEN** it contains the substring `Spec Kit` (case-insensitive)
- **AND** it contains the substring `speckit` (the TOML value)

#### Scenario: README Features section mentions forward coordination and conflict detection

- **WHEN** the README's Features section is inspected
- **THEN** it contains the substring `agent.intent`
- **AND** it contains the substring `conflict detection` (case-insensitive)

#### Scenario: README Features section mentions learnings mode

- **WHEN** the README's Features section is inspected
- **THEN** it contains the substring `learnings` (case-insensitive)
- **AND** it contains the substring `.git-paw/session-learnings.md`

#### Scenario: README Features section mentions every CLI flag

- **WHEN** the README's Features section is inspected
- **THEN** it contains the substring `--specs-format`
- **AND** it contains the substring `--no-supervisor`
- **AND** it contains the substring `--force` (in a `start --force` context)

#### Scenario: README Features section does not advertise auto-approval

- **WHEN** the README's Features section is inspected
- **THEN** it SHALL NOT contain `[supervisor.auto_approve]`
- **AND** it SHALL NOT describe git-paw as auto-approving permission prompts
