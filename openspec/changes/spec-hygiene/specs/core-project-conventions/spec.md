## REMOVED Requirements

### Requirement: README Highlights section enumerates the current user-facing surface

**Reason**: Its scenarios are still titled "README Features section …" although the README section has been `## Highlights` since v0.19.0, and one of them inspects the Commands section. OpenSpec rejects a scenario rename inside a MODIFIED block, so the requirement returns under a new name with retitled scenarios.

**Migration**: Replaced by "README Highlights section links each capability family" in this change. Each scenario keeps its body and its test.

## ADDED Requirements

### Requirement: README Highlights section links each capability family

`README.md`'s `## Highlights` section SHALL give one line per capability family, each linking the docs page that covers it, so users can discover the feature set from a single page. Highlights SHALL cover spec-driven launch (naming Spec Kit), the broker with conflict detection, and learnings mode, and the linked pages SHALL hold the detail. The README SHALL NOT present automatic approval of permission prompts as a feature.

#### Scenario: README Highlights section mentions Spec Kit

- **WHEN** the README's Highlights section is inspected
- **THEN** it contains the substring `Spec Kit` (case-insensitive) and links `spec-driven-launch.md`
- **AND** `docs/src/user-guide/spec-driven-launch.md` contains the substring `speckit` (the TOML value)

Test: `tests/readme_features.rs::highlights_mention_spec_kit_and_link_its_page`

#### Scenario: README Highlights section mentions conflict detection

- **WHEN** the README's Highlights section is inspected
- **THEN** it contains the substring `conflict detection` (case-insensitive) and links `conflict-detection.md`
- **AND** `docs/src/user-guide/conflict-detection.md` contains the substring `agent.intent`

Test: `tests/readme_features.rs::highlights_mention_conflict_detection`

#### Scenario: README Highlights section mentions learnings mode

- **WHEN** the README's Highlights section is inspected
- **THEN** it contains the substring `learnings` (case-insensitive) and links `learnings.md`
- **AND** `docs/src/user-guide/learnings.md` contains the substring `.git-paw/session-learnings.md`

Test: `tests/readme_features.rs::highlights_mention_learnings_and_link_its_page`

#### Scenario: README Commands section links a reference naming every CLI flag

- **WHEN** the README's Commands section is inspected
- **THEN** it links `cli-reference.md`
- **AND** `docs/src/cli-reference.md` contains the substrings `--specs-format`, `--no-supervisor` and `--force` (in a `start --force` context)

Test: `tests/readme_features.rs::commands_link_the_cli_reference_which_names_every_flag`

#### Scenario: README Highlights section does not advertise auto-approval

- **WHEN** the README's Highlights section is inspected
- **THEN** it SHALL NOT contain `[supervisor.auto_approve]`
- **AND** it SHALL NOT describe git-paw as auto-approving permission prompts

Test: `tests/readme_features.rs::highlights_do_not_advertise_auto_approval`

#### Scenario: Every Highlights bullet links a docs page

- **WHEN** the README's Highlights section is inspected
- **THEN** every bullet SHALL contain a Markdown link (`](`)

Test: `tests/readme_features.rs::every_highlights_bullet_links_a_page`
