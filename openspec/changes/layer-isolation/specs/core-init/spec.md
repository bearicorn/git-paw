## MODIFIED Requirements

### Requirement: Init installs the agent-broker helper script

The system SHALL install the bundled agent-broker helper
(`agent-broker-helper` capability) at
`<repo>/.git-paw/scripts/broker.sh` when `git paw init` runs and the
resulting configuration enables the Broker layer (`[broker] enabled =
true`, or `default_mode = "coordinated"`, per `core-layer-isolation`). The
script SHALL be written with executable mode `0o755` on Unix and SHALL be
overwritten on re-run (binary-managed content). When the configuration
does not enable the Broker layer, init SHALL NOT install the script and
SHALL remove a copy a previous run installed. Init SHALL report whether
the file was created, updated or removed.

#### Scenario: Init installs broker.sh when the broker layer is enabled

- **GIVEN** a git repo whose `.git-paw/config.toml` sets `[broker] enabled = true`
- **WHEN** `git paw init` is run
- **THEN** `<repo>/.git-paw/scripts/broker.sh` SHALL exist
- **AND** its first line SHALL be a bash shebang
- **AND** on Unix it SHALL have the user/group/other execute bits set

#### Scenario: Init overwrites a stale broker.sh

- **GIVEN** a Broker-layer configuration and a `<repo>/.git-paw/scripts/broker.sh` containing stale local
  content
- **WHEN** `git paw init` is run
- **THEN** the file SHALL be overwritten with the bundled helper content
- **AND** init SHALL report that `.git-paw/scripts/broker.sh` was updated

#### Scenario: Init reports broker.sh creation

- **GIVEN** a Broker-layer configuration and no prior
  `.git-paw/scripts/broker.sh`
- **WHEN** `git paw init` is run
- **THEN** stdout SHALL report creation of
  `.git-paw/scripts/broker.sh`

#### Scenario: Init without the broker layer installs no broker helper

- **WHEN** `git paw init` is run non-interactively in a git repo with no `.git-paw/`
  directory (recording `default_mode = "bare"` and no broker)
- **THEN** `<repo>/.git-paw/scripts/broker.sh` SHALL NOT exist

#### Scenario: Disabling the broker removes the helper

- **GIVEN** a `<repo>/.git-paw/scripts/broker.sh` installed by an earlier init
- **AND** a configuration that no longer enables the Broker layer
- **WHEN** `git paw init` is run
- **THEN** the file SHALL be removed
- **AND** init SHALL report the removal

### Requirement: Init installs the shared shell preamble

The system SHALL install a bundled shared shell preamble at
`<repo>/.git-paw/scripts/_paw_common.sh` when `git paw init` runs and
installs at least one of the `broker.sh`, `sweep.sh` and `docs-fetch.sh`
helpers, and SHALL remove a previously installed copy when init installs
none of them. The file SHALL be written
with executable mode `0o755` on Unix and SHALL be overwritten on re-run
(binary-managed content). Init SHALL report whether the file was created,
updated or removed, consistent with the other bundled scripts.

Each bundled helper (`broker.sh`, `sweep.sh`, `docs-fetch.sh`) SHALL obtain its
shared functions — repository-root resolution, Python-3 interpreter detection,
config-value reading, broker-URL / docs-base-URL discovery, and branch slugify —
by sourcing `_paw_common.sh`, and SHALL NOT re-define those functions inline. A
helper SHALL resolve the sourced path relative to its own script location, and
the preamble SHALL be installed as a sibling of every helper wherever that
helper is installed, so the source succeeds from any installed location of the
helper. The preamble SHALL have exactly one source file in the bundled package
tree; every package that ships a helper SHALL carry that same content. The
observable behavior of every helper SHALL be unchanged from the pre-refactor
inline implementation.

The shared preamble SHALL remain project-agnostic: it SHALL source the broker URL
and docs base URL from resolved configuration and SHALL NOT hard-code any
consumer's toolchain.

#### Scenario: Init installs _paw_common.sh alongside a helper

- **GIVEN** a configuration that enables at least one layer with a helper script
- **WHEN** `git paw init` is run
- **THEN** `<repo>/.git-paw/scripts/_paw_common.sh` SHALL exist
- **AND** its first line SHALL be a bash shebang
- **AND** on Unix it SHALL have the user/group/other execute bits set

#### Scenario: Init overwrites a stale _paw_common.sh

- **GIVEN** a configuration that enables a layer with a helper script and a
  `<repo>/.git-paw/scripts/_paw_common.sh` containing stale local content
- **WHEN** `git paw init` is run
- **THEN** the file SHALL be overwritten with the bundled preamble content
- **AND** init SHALL report that `.git-paw/scripts/_paw_common.sh` was updated

#### Scenario: No helper, no preamble

- **GIVEN** a configuration that enables none of the Skills, Broker and Coordinated layers
- **WHEN** `git paw init` is run
- **THEN** `<repo>/.git-paw/scripts/_paw_common.sh` SHALL NOT exist

#### Scenario: Bundled helpers source the shared preamble and do not re-define it

- **GIVEN** the bundled `broker.sh`, `sweep.sh`, and `docs-fetch.sh`
- **WHEN** their content is inspected
- **THEN** each SHALL source `_paw_common.sh` (resolved relative to its own
  location)
- **AND** none SHALL contain its own inline definition of a function provided by
  `_paw_common.sh` (`repo_root`, the interpreter detector, `discover_broker_url`,
  `discover_docs_base_url`, `slugify`)

#### Scenario: A deployed helper resolves shared functions from the sourced file

- **GIVEN** `_paw_common.sh` and a helper are deployed together in
  `<repo>/.git-paw/scripts/`
- **WHEN** the helper runs a command that uses a shared function (for example
  `broker.sh` resolving the broker URL)
- **THEN** the shared function SHALL resolve from `_paw_common.sh`
- **AND** the result SHALL equal what the pre-refactor inline definition produced
  for the same inputs

#### Scenario: A package installed into an arbitrary directory runs

- **GIVEN** a helper-shipping package's scripts installed together into an empty
  temporary directory
- **WHEN** the helper is invoked from there
- **THEN** it SHALL source the sibling `_paw_common.sh` and run

#### Scenario: The preamble has one source

- **WHEN** the bundled package tree is inspected
- **THEN** exactly one `_paw_common.sh` source file SHALL exist

### Requirement: Init installs the orchestrator skill in the standardized format

The system SHALL, when `git paw init` runs and the resulting configuration enables the Coordinated layer (`default_mode = "coordinated"`, per `core-layer-isolation`), render the orchestrator skill and write it in the agentskills.io standardized layout at `<repo>/.git-paw/skills/orchestrator/SKILL.md` with a frontmatter block carrying `name: orchestrator` (equal to its directory name, as the standardized format requires) and a one-sentence `description`. Init runs with no session, so the rendering SHALL use only values the repository and its config supply: the repository's current `[orchestrator.gates]` commands (unset commands rendering as `(not configured)`, an unset `doc_tool_command` as empty); `{{SPEC_PATH_DOCTRINE}}` rendered from the configured spec format (`[specs]`), with the no-backend sentinel when none is configured, and the OpenSpec-only regions kept exactly when that format is OpenSpec; and `{{CHANGE_ID}}` left verbatim (it is expanded per verification). The rendered file SHALL contain no session-specific value — no broker URL, no `{{GIT_PAW_BROKER_URL}}` placeholder and no tmux session name — and no other unsubstituted placeholder; the session's values live only in the repo-root sidecar `.git-paw/AGENTS.local.md`. Because the inputs are session-independent, `git paw start` re-rendering the file with an unchanged config and binary SHALL produce the same bytes. The file SHALL be overwritten on re-run (binary-managed content), `.git-paw/skills/` SHALL be added to the required `.gitignore` entries, and init SHALL report whether the file was created or updated. When the configuration does not enable the Coordinated layer, init SHALL NOT write the file and SHALL remove a copy a previous run wrote, reporting the removal.

#### Scenario: Init installs the skill when coordinated mode is the default

- **GIVEN** a git repo whose `.git-paw/config.toml` sets `default_mode = "coordinated"`
- **WHEN** `git paw init` is run
- **THEN** `<repo>/.git-paw/skills/orchestrator/SKILL.md` SHALL exist
- **AND** its first line SHALL open a frontmatter block containing `name: orchestrator`
- **AND** stdout SHALL report its creation

#### Scenario: Skill renders configured gate commands

- **GIVEN** `.git-paw/config.toml` with `default_mode = "coordinated"` and `[orchestrator.gates] test_command = "make test"`
- **WHEN** `git paw init` is run
- **THEN** the written `SKILL.md` SHALL contain `make test` where the skill's test-command placeholder renders
- **AND** an unset gate command SHALL render as `(not configured)`

#### Scenario: Init renders without a session

- **GIVEN** a repository with `default_mode = "coordinated"`, `[specs] type = "openspec"` and no running session
- **WHEN** `git paw init` is run
- **THEN** the written `SKILL.md` SHALL contain the OpenSpec spec-path doctrine where `{{SPEC_PATH_DOCTRINE}}` renders
- **AND** it SHALL contain no `{{GIT_PAW_BROKER_URL}}`, no `http://` broker URL and no `paw-` session name
- **AND** `{{CHANGE_ID}}` SHALL appear verbatim and no other `{{…}}` placeholder SHALL remain

#### Scenario: No spec system renders the doctrine sentinel

- **GIVEN** a repository with `default_mode = "coordinated"` whose config has no `[specs]` section
- **WHEN** `git paw init` is run
- **THEN** the written `SKILL.md` SHALL contain the no-backend doctrine sentinel where `{{SPEC_PATH_DOCTRINE}}` renders

#### Scenario: Start re-renders the same bytes for an unchanged config

- **GIVEN** a repository with `default_mode = "coordinated"` where `git paw init` wrote `.git-paw/skills/orchestrator/SKILL.md`
- **WHEN** `git paw start --coordinated` runs with the same config and binary
- **THEN** the file's content SHALL be byte-identical to what init wrote

#### Scenario: Init overwrites a stale skill file

- **GIVEN** coordinated mode as the default and a `<repo>/.git-paw/skills/orchestrator/SKILL.md` containing stale local content
- **WHEN** `git paw init` is run
- **THEN** the file SHALL be overwritten with the freshly rendered skill
- **AND** init SHALL report that it was updated

#### Scenario: Skill directory is gitignored

- **WHEN** `git paw init` is run
- **THEN** `.gitignore` SHALL contain an entry ignoring `.git-paw/skills/`

#### Scenario: Init without coordinated mode writes no orchestrator skill

- **GIVEN** a configuration with `default_mode = "bare"`
- **WHEN** `git paw init` is run
- **THEN** `<repo>/.git-paw/skills/orchestrator/SKILL.md` SHALL NOT exist, and a copy written by an earlier init SHALL have been removed

### Requirement: Init injects an orchestrator pointer section into the root AGENTS.md

The system SHALL, when `git paw init` runs and the resulting configuration enables the Coordinated layer (`default_mode = "coordinated"`, per `core-layer-isolation`), inject into `<repo>/AGENTS.md` (creating the file when absent) a section delimited by `<!-- git-paw-orchestrator:start -->` and `<!-- git-paw-orchestrator:end -->` whose prose comes from the bundled asset `assets/agents-md-orchestrator-section.md` and tells an agent reading the file that, when a git-paw session is active, it is the session's orchestrator and should read `.git-paw/skills/orchestrator/SKILL.md` and `.git-paw/AGENTS.local.md`. Injection SHALL be idempotent (a second run replaces the section in place, leaving all other content byte-identical). The section SHALL carry no broker URL, no secret, no stack or toolchain token, and no vendor product name or path. `git paw stop` and `git paw purge` SHALL NOT remove this section. When the configuration does not enable the Coordinated layer, init SHALL NOT inject the section and SHALL remove an existing one by deleting only the marker-delimited section, leaving every other byte of `AGENTS.md` unchanged and never creating `AGENTS.md`.

#### Scenario: Init injects the pointer into an existing AGENTS.md

- **GIVEN** coordinated mode as the default and a repo whose `AGENTS.md` contains `# My project` and no git-paw markers
- **WHEN** `git paw init` is run
- **THEN** `AGENTS.md` SHALL still begin with `# My project`
- **AND** SHALL contain exactly one `<!-- git-paw-orchestrator:start -->` … `<!-- git-paw-orchestrator:end -->` section naming `.git-paw/skills/orchestrator/SKILL.md`

#### Scenario: Init creates AGENTS.md when absent

- **GIVEN** coordinated mode as the default and a repo with no `AGENTS.md`
- **WHEN** `git paw init` is run
- **THEN** `AGENTS.md` SHALL exist and contain the pointer section

#### Scenario: Re-running init replaces the section in place

- **GIVEN** coordinated mode as the default and an `AGENTS.md` carrying the pointer section followed by user content
- **WHEN** `git paw init` is run again
- **THEN** the file SHALL contain exactly one pointer section
- **AND** the user content outside the markers SHALL be byte-identical

#### Scenario: Stop and purge leave the pointer

- **GIVEN** an `AGENTS.md` with the pointer section
- **WHEN** `git paw stop` and then `git paw purge --force` run
- **THEN** the pointer section SHALL still be present

#### Scenario: Init without coordinated mode leaves AGENTS.md without the pointer

- **GIVEN** a configuration with `default_mode = "bare"` and an `AGENTS.md` carrying the pointer section between user content
- **WHEN** `git paw init` is run
- **THEN** the pointer section SHALL be removed and all other content SHALL be byte-identical
- **AND** a repo with no `AGENTS.md` SHALL still have none after init
