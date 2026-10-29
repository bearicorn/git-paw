## Purpose

Defines how git-paw's bundled skills are packaged and delivered: each bundled skill is an agentskills.io package (a `SKILL.md` with frontmatter plus the `scripts/`, `references/` and `assets/` it owns), a compiled registry is the single source of the bundled set for every consumer (skill resolution, init, worktree provisioning, doctor, the `git paw setup-cli` grant table's helper coverage, the no-leak audit), which packages reach an agent worktree is a pure decision, overrides change skill text but never the helpers git-paw installs, and requirements that still name pre-package asset paths are read against the package files.

## ADDED Requirements

### Requirement: Bundled skills ship as agentskills.io packages

The system SHALL ship every bundled skill as an agentskills.io package: a directory `assets/agent-skills/<name>/` containing a `SKILL.md` whose frontmatter carries the required `name` and `description` fields followed by the skill body, plus that skill's own optional `scripts/`, `references/` and `assets/` subdirectories. Each bundled helper script SHALL belong to exactly one package — `broker.sh` and the boot-block templates to `coordination`, `sweep.sh` to `orchestrator`, `docs-fetch.sh` to `docs-fetch`. The bundled `cli-setup` skill SHALL be a package `cli-setup` that ships no helper script. The shared shell preamble `_paw_common.sh` SHALL have exactly one source file and SHALL be listed in the `scripts/` of every package that ships a helper. Every bundled package SHALL load successfully through the system's own standardized-skill loader.

#### Scenario: Every bundled package validates as a standardized skill

- **WHEN** each registered bundled package's `SKILL.md` is loaded through the standardized-skill loader
- **THEN** loading SHALL succeed
- **AND** the parsed metadata SHALL carry a non-empty `name` and `description`

#### Scenario: Each helper belongs to one package

- **WHEN** the registered packages' scripts are enumerated
- **THEN** `broker.sh` SHALL appear only in `coordination`, `sweep.sh` only in `orchestrator`, and `docs-fetch.sh` only in `docs-fetch`

#### Scenario: The cli-setup skill is a registered package without helpers

- **WHEN** the registered packages are enumerated
- **THEN** a `cli-setup` package SHALL be registered
- **AND** it SHALL list no helper script

#### Scenario: Every helper-shipping package carries the shared preamble

- **WHEN** a registered package lists any helper script
- **THEN** its scripts SHALL also include `_paw_common.sh`
- **AND** that content SHALL be byte-identical across all packages

### Requirement: The package registry is the single source of the bundled skill set

The system SHALL expose a compiled package registry listing every bundled package with its embedded `SKILL.md`, scripts, references, assets and provisioning gate. Embedded skill resolution, the set of helpers `git paw init` installs, the set provisioned into agent worktrees, the doctor bundled-scripts check, and the set of helper names the `git paw setup-cli` grant table must cover SHALL each be derived from the registry rather than from a separately maintained list. The registry SHALL embed its files at compile time so the binary needs no asset directory at runtime. A file present under a package directory but absent from the registry, or registered but absent on disk, SHALL fail the test suite.

#### Scenario: Embedded resolution comes from the registry

- **GIVEN** no override exists for a registered package name
- **WHEN** `skills::resolve(<name>)` is called
- **THEN** it SHALL return `Source::Embedded` with the body of that package's registered `SKILL.md`

#### Scenario: A registered helper is installed by init and checked by doctor

- **GIVEN** a registered package that lists a helper script
- **WHEN** `git paw init` runs and then `git paw doctor` runs
- **THEN** init SHALL have installed that helper
- **AND** the doctor bundled-scripts group SHALL report a result for it

#### Scenario: An unregistered package file fails the suite

- **GIVEN** a file added under `assets/agent-skills/<name>/` without a registry entry
- **WHEN** the registry conformance test runs
- **THEN** it SHALL fail naming the unregistered file

#### Scenario: The binary renders bundled skills with no assets on disk

- **WHEN** the binary resolves and renders a bundled skill with no `assets/` directory present
- **THEN** rendering SHALL succeed from the compile-time embedded copies

### Requirement: Worktree package selection is a pure decision

The system SHALL decide which bundled packages an agent worktree receives with a pure function of the session's inputs — whether the broker is enabled and whether `docs_base_url` is explicitly configured — performing no I/O. The selection SHALL include `coordination` when the broker is enabled and `docs-fetch` when `docs_base_url` is configured, in that order, and SHALL never include the root-only packages `orchestrator` and `cli-setup`. Code that writes package files into a worktree SHALL write exactly the selected set.

#### Scenario: Broker enabled, docs unset

- **WHEN** the selection is computed with the broker enabled and `docs_base_url` unset
- **THEN** it SHALL be exactly `[coordination]`

#### Scenario: Broker enabled, docs configured

- **WHEN** the selection is computed with the broker enabled and `docs_base_url` configured
- **THEN** it SHALL be exactly `[coordination, docs-fetch]`

#### Scenario: Nothing enabled

- **WHEN** the selection is computed with the broker disabled and `docs_base_url` unset
- **THEN** it SHALL be empty

#### Scenario: The orchestrator package never reaches a worktree

- **WHEN** the selection is computed for any combination of inputs
- **THEN** it SHALL NOT contain `orchestrator`

#### Scenario: The cli-setup package never reaches a worktree

- **WHEN** the selection is computed for any combination of inputs
- **THEN** it SHALL NOT contain `cli-setup`

### Requirement: Skill overrides replace skill text, never helper scripts

The system SHALL apply a skill override (a `.agents/skills/<name>/` directory or a user config-directory override) only to the skill text that is rendered. Helper scripts, references and assets carried by an override directory SHALL NOT be installed or provisioned; every helper the system writes SHALL be the binary's embedded copy from the registry.

#### Scenario: An override's script is not installed

- **GIVEN** a `.agents/skills/coordination/` override whose `scripts/broker.sh` differs from the embedded helper
- **WHEN** an agent worktree is provisioned
- **THEN** the provisioned `broker.sh` SHALL be byte-identical to the registry's embedded copy
- **AND** the override's skill text SHALL still be what is rendered for that agent

### Requirement: Requirements naming pre-package asset paths refer to package files

The system SHALL treat any requirement that identifies a bundled asset by its pre-package source path as referring to the corresponding package file, per this mapping:

| Pre-package path | Package file |
|---|---|
| `assets/agent-skills/coordination.md` | `assets/agent-skills/coordination/SKILL.md` |
| `assets/agent-skills/supervisor.md` (or `orchestrator.md`) | `assets/agent-skills/orchestrator/SKILL.md` |
| `assets/agent-skills/docs-fetch.md` | `assets/agent-skills/docs-fetch/SKILL.md` |
| `assets/agent-skills/cli-setup.md` | `assets/agent-skills/cli-setup/SKILL.md` |
| `assets/scripts/broker.sh` | `assets/agent-skills/coordination/scripts/broker.sh` |
| `assets/scripts/sweep.sh` | `assets/agent-skills/orchestrator/scripts/sweep.sh` |
| `assets/scripts/docs-fetch.sh` | `assets/agent-skills/docs-fetch/scripts/docs-fetch.sh` |
| `assets/scripts/_paw_common.sh` | `assets/agent-skills/_shared/scripts/_paw_common.sh` |
| `assets/boot-block-template.md` | `assets/agent-skills/coordination/assets/boot-block.md` |
| `assets/boot-block-mcp-template.md` | `assets/agent-skills/coordination/assets/boot-block-mcp.md` |

Such a requirement's content constraints SHALL apply unchanged to the mapped file.

#### Scenario: A content requirement on a pre-package path is checked against the package file

- **GIVEN** a requirement that the bundled `assets/scripts/sweep.sh` contain a given subcommand
- **WHEN** its covering test runs after packaging
- **THEN** the test SHALL assert against the orchestrator package's `scripts/sweep.sh`
- **AND** the constraint SHALL hold unchanged

### Requirement: Packaging preserves deployed output

Restructuring bundled skills into packages SHALL NOT change any file the system writes into a repository or worktree, nor any rendered text. Under identical inputs, rendered skills, boot blocks and sidecars SHALL be byte-identical to their pre-package rendering, and `git paw init` and worktree provisioning SHALL write the same set of files, with identical content and modes, at the same paths.

#### Scenario: Rendering is byte-identical across the restructure

- **GIVEN** a fixed branch, broker URL, project, gate-command set and backend set
- **WHEN** each bundled skill and both boot-block forms are rendered before and after packaging
- **THEN** the outputs SHALL be byte-identical

#### Scenario: Init writes the same files

- **WHEN** `git paw init` runs in a fresh repository after packaging
- **THEN** the files it writes under `.git-paw/scripts/` SHALL have the same names, content and executable mode as before packaging
