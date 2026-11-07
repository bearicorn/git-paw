## MODIFIED Requirements

### Requirement: Init injects an orchestrator pointer section into the root AGENTS.md

The system SHALL, when `git paw init` runs, inject into `<repo>/AGENTS.md` (creating the file when absent) a section delimited by `<!-- git-paw-orchestrator:start -->` and `<!-- git-paw-orchestrator:end -->` whose prose comes from the bundled asset `assets/agents-md-orchestrator-section.md` and tells an agent reading the file that, when a git-paw session is active, it is the session's orchestrator and should read `.git-paw/skills/orchestrator/SKILL.md` and `.git-paw/AGENTS.local.md`. Because every worktree checks out the tracked `AGENTS.md`, the section SHALL state that it applies only to the agent session running at the repository root (the main checkout), and that a coding agent running in a worktree SHALL ignore it and follow its own session assignment instead. Injection SHALL be idempotent (a second run replaces the section in place, leaving all other content byte-identical). The section SHALL carry no broker URL, no secret, no stack or toolchain token, and no vendor product name or path. `git paw stop` and `git paw purge` SHALL NOT remove this section.

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

#### Scenario: The pointer is scoped to the root session

- **WHEN** `git paw init` has injected the pointer section
- **THEN** the section SHALL state that it applies only to the session at the repository root
- **AND** SHALL state that a coding agent running in a worktree ignores it
- **AND** SHALL NOT address its reader as the orchestrator without that condition
