## ADDED Requirements

### Requirement: README is a short landing page that links the docs

`README.md` SHALL be a landing page of at most 130 lines. It SHALL contain, in this order: title, tagline and badges; What it does; Demo; Highlights; Install; Quick start; Commands; Configuration; Supported CLIs; Security; Documentation; Contributing; License. Detail SHALL live in `docs/src/` and be linked rather than repeated, and the README SHALL NOT carry release-procedure or contributor-only build detail.

#### Scenario: The README fits the landing-page budget

- **WHEN** `README.md` is read
- **THEN** it SHALL have at most 130 lines
- **AND** its `## ` headings SHALL appear in the order above

#### Scenario: The quick start links the three quick-start pages

- **WHEN** the README's Quick start section is inspected
- **THEN** it SHALL link `quick-start-same-cli.md`, `quick-start-per-branch.md` and `quick-start-orchestrator.md`
- **AND** it SHALL show `git paw init`, `git paw start --supervisor` and `git paw start --unattended --from-all-specs`

#### Scenario: The command table carries no version tags and links the reference

- **WHEN** the README's Commands table is inspected
- **THEN** no cell SHALL contain a `(v0.` version tag
- **AND** the section SHALL link `cli-reference.md`

#### Scenario: Superseded content is gone

- **WHEN** `README.md` is read
- **THEN** it SHALL NOT contain `show_message_log`, a `## Releases` section, or a worktree diagram that places agent worktrees only at `../`

### Requirement: README-only facts move into the docs before removal

A fact the README states that no page under `docs/src/` states SHALL be added to the matching docs page before it is removed from the README.

#### Scenario: The CLAUDE.md import tip lives in the AGENTS.md guide

- **WHEN** `docs/src/user-guide/agents-md.md` is inspected
- **THEN** it SHALL contain `@AGENTS.md` as the first line of a `CLAUDE.md`, explain why the import beats a symlink, and show `ln -s AGENTS.md GEMINI.md` for a CLI without imports

#### Scenario: start --force under supervisor mode is documented

- **WHEN** `docs/src/cli-reference.md` and `docs/src/user-guide/spec-driven-launch.md` are inspected
- **THEN** each SHALL state that `start --force` has no effect under supervisor mode

### Requirement: README Security note states the trust model in brief

The README `## Security` section SHALL state, in at most about five lines, that agents run arbitrary code with the user's privileges, that git-paw approves no permission prompt, that the agent CLI's own permission system decides, and that an optional sandbox confines writes. It SHALL link `security-posture.md` and `sandbox.md`.

#### Scenario: The Security note names the trust model and links both pages

- **WHEN** the README's `## Security` section is inspected
- **THEN** it SHALL contain `arbitrary code`, `sandbox` and a statement that git-paw approves no prompt
- **AND** it SHALL link `security-posture.md` and `sandbox.md`

## MODIFIED Requirements

### Requirement: README features section enumerates the current user-facing surface

`README.md`'s `## Highlights` section SHALL give one line per capability family, each linking the docs page that covers it, so users can discover the feature set from a single page. Highlights SHALL cover spec-driven launch (naming Spec Kit), the broker with conflict detection, and learnings mode, and the linked pages SHALL hold the detail. The README SHALL NOT present automatic approval of permission prompts as a feature.

#### Scenario: README Features section mentions Spec Kit

- **WHEN** the README's Highlights section is inspected
- **THEN** it contains the substring `Spec Kit` (case-insensitive) and links `spec-driven-launch.md`
- **AND** `docs/src/user-guide/spec-driven-launch.md` contains the substring `speckit` (the TOML value)

#### Scenario: README Features section mentions forward coordination and conflict detection

- **WHEN** the README's Highlights section is inspected
- **THEN** it contains the substring `conflict detection` (case-insensitive)
- **AND** `docs/src/user-guide/coordination.md` contains the substring `agent.intent`

#### Scenario: README Features section mentions learnings mode

- **WHEN** the README's Highlights section is inspected
- **THEN** it contains the substring `learnings` (case-insensitive) and links `learnings.md`
- **AND** `docs/src/user-guide/learnings.md` contains the substring `.git-paw/session-learnings.md`

#### Scenario: README Features section mentions every CLI flag

- **WHEN** the README's Commands section is inspected
- **THEN** it links `cli-reference.md`
- **AND** `docs/src/cli-reference.md` contains the substrings `--specs-format`, `--no-supervisor` and `--force` (in a `start --force` context)

#### Scenario: README Features section does not advertise auto-approval

- **WHEN** the README's Highlights section is inspected
- **THEN** it SHALL NOT contain `[supervisor.auto_approve]`
- **AND** it SHALL NOT describe git-paw as auto-approving permission prompts
