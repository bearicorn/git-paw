## RENAMED Requirements

- FROM: `### Requirement: README features section enumerates the current user-facing surface`
- TO: `### Requirement: README Highlights section enumerates the current user-facing surface`

## MODIFIED Requirements

### Requirement: README Highlights section enumerates the current user-facing surface

`README.md`'s `## Highlights` section SHALL give one line per capability family, each linking the docs page that covers it, so users can discover the feature set from a single page. Highlights SHALL cover spec-driven launch (naming Spec Kit), the broker with conflict detection, and learnings mode, and the linked pages SHALL hold the detail. The README SHALL NOT present automatic approval of permission prompts as a feature.

#### Scenario: README Features section mentions Spec Kit

- **WHEN** the README's Highlights section is inspected
- **THEN** it contains the substring `Spec Kit` (case-insensitive) and links `spec-driven-launch.md`
- **AND** `docs/src/user-guide/spec-driven-launch.md` contains the substring `speckit` (the TOML value)

Test: `tests/readme_features.rs::highlights_mention_spec_kit_and_link_its_page`

#### Scenario: README Features section mentions forward coordination and conflict detection

- **WHEN** the README's Highlights section is inspected
- **THEN** it contains the substring `conflict detection` (case-insensitive) and links `conflict-detection.md`
- **AND** `docs/src/user-guide/conflict-detection.md` contains the substring `agent.intent`

Test: `tests/readme_features.rs::highlights_mention_conflict_detection`

#### Scenario: README Features section mentions learnings mode

- **WHEN** the README's Highlights section is inspected
- **THEN** it contains the substring `learnings` (case-insensitive) and links `learnings.md`
- **AND** `docs/src/user-guide/learnings.md` contains the substring `.git-paw/session-learnings.md`

Test: `tests/readme_features.rs::highlights_mention_learnings_and_link_its_page`

#### Scenario: README Features section mentions every CLI flag

- **WHEN** the README's Commands section is inspected
- **THEN** it links `cli-reference.md`
- **AND** `docs/src/cli-reference.md` contains the substrings `--specs-format`, `--no-supervisor` and `--force` (in a `start --force` context)

Test: `tests/readme_features.rs::commands_link_the_cli_reference_which_names_every_flag`

#### Scenario: README Features section does not advertise auto-approval

- **WHEN** the README's Highlights section is inspected
- **THEN** it SHALL NOT contain `[supervisor.auto_approve]`
- **AND** it SHALL NOT describe git-paw as auto-approving permission prompts

Test: `tests/readme_features.rs::highlights_do_not_advertise_auto_approval`

#### Scenario: Every Highlights bullet links a docs page

- **WHEN** the README's Highlights section is inspected
- **THEN** every bullet SHALL contain a Markdown link (`](`)

Test: `tests/readme_features.rs::every_highlights_bullet_links_a_page`

### Requirement: README-only facts move into the docs before removal

`docs/src/user-guide/agents-md.md` SHALL document the `CLAUDE.md` import of `AGENTS.md`, and `docs/src/cli-reference.md` and `docs/src/user-guide/spec-driven-launch.md` SHALL state that `start --force` has no effect under supervisor mode, so the README can link these facts instead of stating them.

#### Scenario: The CLAUDE.md import tip lives in the AGENTS.md guide

- **WHEN** `docs/src/user-guide/agents-md.md` is inspected
- **THEN** it SHALL contain `@AGENTS.md` as the first line of a `CLAUDE.md`, explain why the import beats a symlink, and show `ln -s AGENTS.md GEMINI.md` for a CLI without imports

Test: `tests/readme_features.rs::claude_md_import_tip_lives_in_the_agents_md_guide`

#### Scenario: start --force under supervisor mode is documented

- **WHEN** `docs/src/cli-reference.md` and `docs/src/user-guide/spec-driven-launch.md` are inspected
- **THEN** each SHALL state that `start --force` has no effect under supervisor mode

Test: `tests/readme_features.rs::start_force_no_effect_under_supervisor_is_documented`

### Requirement: README Security note states the trust model in brief

The README `## Security` section SHALL state, in at most 6 non-blank lines, that agents run arbitrary code with the user's privileges, that git-paw approves no permission prompt, that the agent CLI's own permission system decides, and that an optional sandbox confines writes. It SHALL link `security-posture.md` and `sandbox.md`.

#### Scenario: The Security note names the trust model and links both pages

- **WHEN** the README's `## Security` section is inspected
- **THEN** it SHALL contain `arbitrary code`, `sandbox` and a statement that git-paw approves no prompt
- **AND** it SHALL link `security-posture.md` and `sandbox.md`
- **AND** it SHALL have at most 6 non-blank lines

Test: `tests/readme_features.rs::security_note_names_the_trust_model_and_links_both_pages`
