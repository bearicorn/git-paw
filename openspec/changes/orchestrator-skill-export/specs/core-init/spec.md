## ADDED Requirements

### Requirement: Init installs the orchestrator skill in the standardized format

The system SHALL, when `git paw init` runs, render the orchestrator skill and write it in the agentskills.io standardized layout at `<repo>/.git-paw/skills/orchestrator/SKILL.md` with a frontmatter block carrying `name: orchestrator` (equal to its directory name, as the standardized format requires) and a one-sentence `description`. Init runs with no session, so the rendering SHALL use only values the repository and its config supply: the `[supervisor]` gate commands (unset commands rendering as `(not configured)`, an unset `doc_tool_command` as empty); `{{SPEC_PATH_DOCTRINE}}` rendered from the configured spec format (`[specs]`), with the no-backend sentinel when none is configured, and the OpenSpec-only regions kept exactly when that format is OpenSpec; and `{{CHANGE_ID}}` left verbatim (it is expanded per verification). The rendered file SHALL contain no session-specific value — no broker URL, no `{{GIT_PAW_BROKER_URL}}` placeholder and no tmux session name — and no other unsubstituted placeholder; the session's values live only in the repo-root sidecar `.git-paw/AGENTS.local.md`. Because the inputs are session-independent, `git paw start` re-rendering the file with an unchanged config and binary SHALL produce the same bytes. The file SHALL be overwritten on re-run (binary-managed content), `.git-paw/skills/` SHALL be added to the required `.gitignore` entries, and init SHALL report whether the file was created or updated.

#### Scenario: Init installs the skill in a fresh repo

- **WHEN** `git paw init` is run in a git repo with no `.git-paw/` directory
- **THEN** `<repo>/.git-paw/skills/orchestrator/SKILL.md` SHALL exist
- **AND** its first line SHALL open a frontmatter block containing `name: orchestrator`
- **AND** stdout SHALL report its creation

#### Scenario: Skill renders configured gate commands

- **GIVEN** `.git-paw/config.toml` with `[supervisor] test_command = "make test"`
- **WHEN** `git paw init` is run
- **THEN** the written `SKILL.md` SHALL contain `make test` where the skill's test-command placeholder renders
- **AND** an unset gate command SHALL render as `(not configured)`

#### Scenario: Init renders without a session

- **GIVEN** a repository with `[specs] type = "openspec"` and no running session
- **WHEN** `git paw init` is run
- **THEN** the written `SKILL.md` SHALL contain the OpenSpec spec-path doctrine where `{{SPEC_PATH_DOCTRINE}}` renders
- **AND** it SHALL contain no `{{GIT_PAW_BROKER_URL}}`, no `http://` broker URL and no `paw-` session name
- **AND** `{{CHANGE_ID}}` SHALL appear verbatim and no other `{{…}}` placeholder SHALL remain

#### Scenario: No spec system renders the doctrine sentinel

- **GIVEN** a repository whose config has no `[specs]` section
- **WHEN** `git paw init` is run
- **THEN** the written `SKILL.md` SHALL contain the no-backend doctrine sentinel where `{{SPEC_PATH_DOCTRINE}}` renders

#### Scenario: Start re-renders the same bytes for an unchanged config

- **GIVEN** a repository where `git paw init` wrote `.git-paw/skills/orchestrator/SKILL.md`
- **WHEN** `git paw start --supervisor` runs with the same config and binary
- **THEN** the file's content SHALL be byte-identical to what init wrote

#### Scenario: Init overwrites a stale skill file

- **GIVEN** a `<repo>/.git-paw/skills/orchestrator/SKILL.md` containing stale local content
- **WHEN** `git paw init` is run
- **THEN** the file SHALL be overwritten with the freshly rendered skill
- **AND** init SHALL report that it was updated

#### Scenario: Skill directory is gitignored

- **WHEN** `git paw init` is run
- **THEN** `.gitignore` SHALL contain an entry ignoring `.git-paw/skills/`

### Requirement: Init installs the cli-setup skill in the standardized format

The system SHALL, when `git paw init` runs, write the resolved `cli-setup` skill (the bundled skill of `skill-agent`, *Embedded cli-setup skill*, frontmatter included) to `<repo>/.git-paw/skills/cli-setup/SKILL.md` in the main checkout, beside the orchestrator skill, so an orchestrator that loads skills from a directory finds the consent rules for `git paw setup-cli` next to its own procedure. The file SHALL be overwritten on re-run (binary-managed content) and init SHALL report whether it was created or updated. It SHALL be written only under the main checkout's `.git-paw/skills/` (already gitignored), never into a worktree, because coding agents never run `git paw setup-cli`.

#### Scenario: Init installs the cli-setup skill beside the orchestrator skill

- **WHEN** `git paw init` is run in a git repo with no `.git-paw/` directory
- **THEN** `<repo>/.git-paw/skills/cli-setup/SKILL.md` SHALL exist and equal the resolved `cli-setup` skill
- **AND** stdout SHALL report its creation

#### Scenario: Init overwrites a stale cli-setup skill file

- **GIVEN** a `<repo>/.git-paw/skills/cli-setup/SKILL.md` containing stale local content
- **WHEN** `git paw init` is run
- **THEN** the file SHALL be overwritten with the resolved `cli-setup` skill
- **AND** init SHALL report that it was updated

#### Scenario: No worktree receives the cli-setup skill

- **GIVEN** a session with coding-agent worktrees
- **WHEN** `git paw init` and `git paw start` have run
- **THEN** no worktree SHALL contain a `.git-paw/skills/cli-setup/` directory written by git-paw

### Requirement: Init injects an orchestrator pointer section into the root AGENTS.md

The system SHALL, when `git paw init` runs, inject into `<repo>/AGENTS.md` (creating the file when absent) a section delimited by `<!-- git-paw-orchestrator:start -->` and `<!-- git-paw-orchestrator:end -->` whose prose comes from the bundled asset `assets/agents-md-orchestrator-section.md` and tells an agent reading the file that, when a git-paw session is active, it is the session's orchestrator and should read `.git-paw/skills/orchestrator/SKILL.md` and `.git-paw/AGENTS.local.md`. Injection SHALL be idempotent (a second run replaces the section in place, leaving all other content byte-identical). The section SHALL carry no broker URL, no secret, no stack or toolchain token, and no vendor product name or path. `git paw stop` and `git paw purge` SHALL NOT remove this section.

#### Scenario: Init injects the pointer into an existing AGENTS.md

- **GIVEN** a repo whose `AGENTS.md` contains `# My project` and no git-paw markers
- **WHEN** `git paw init` is run
- **THEN** `AGENTS.md` SHALL still begin with `# My project`
- **AND** SHALL contain exactly one `<!-- git-paw-orchestrator:start -->` … `<!-- git-paw-orchestrator:end -->` section naming `.git-paw/skills/orchestrator/SKILL.md`

#### Scenario: Init creates AGENTS.md when absent

- **GIVEN** a repo with no `AGENTS.md`
- **WHEN** `git paw init` is run
- **THEN** `AGENTS.md` SHALL exist and contain the pointer section

#### Scenario: Re-running init replaces the section in place

- **GIVEN** an `AGENTS.md` carrying the pointer section followed by user content
- **WHEN** `git paw init` is run again
- **THEN** the file SHALL contain exactly one pointer section
- **AND** the user content outside the markers SHALL be byte-identical

#### Scenario: Stop and purge leave the pointer

- **GIVEN** an `AGENTS.md` with the pointer section
- **WHEN** `git paw stop` and then `git paw purge --force` run
- **THEN** the pointer section SHALL still be present

### Requirement: Init reports updates only when content changed, and its closing line agrees

`git paw init` SHALL report a binary-managed file it writes — a bundled helper script under `.git-paw/scripts/`, an exported skill under `.git-paw/skills/`, or the root `AGENTS.md` pointer section — as `Updated` only when the content on disk differed from what it wrote, and SHALL NOT report a file whose content was already byte-identical as updated. Its closing line SHALL be `Initialized git-paw.` whenever anything was created or changed (a directory, the config, `.gitignore`, a helper script, an exported skill or the pointer section), and `Already initialized. Nothing to do.` only when nothing was created or changed. Init SHALL NOT print an `Updated …` line followed by `Already initialized. Nothing to do.`.

#### Scenario: A re-run with current content reports nothing to do

- **GIVEN** a repository where `git paw init` has just run with the current binary
- **WHEN** `git paw init` is run again
- **THEN** stdout SHALL contain no `Updated` line
- **AND** its closing line SHALL be `Already initialized. Nothing to do.`

#### Scenario: A stale helper is updated and the closing line agrees

- **GIVEN** an initialized repository whose `.git-paw/scripts/sweep.sh` differs from the bundled copy
- **WHEN** `git paw init` is run
- **THEN** stdout SHALL contain `Updated .git-paw/scripts/sweep.sh`
- **AND** its closing line SHALL be `Initialized git-paw.`
- **AND** stdout SHALL NOT contain `Already initialized. Nothing to do.`
