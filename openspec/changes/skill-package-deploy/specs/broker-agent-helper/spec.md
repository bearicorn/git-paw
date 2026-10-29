## MODIFIED Requirements

### Requirement: Helpers provisioned into agent worktrees

`git paw start` and `git paw add` SHALL provision the bundled skill packages an agent uses into that agent's worktree at `.git-paw/skills/<package>/` — each package's rendered `SKILL.md` and its helper scripts — together with the helpers' compatibility copies at `.git-paw/scripts/`, making them present and executable before the agent boots, so the agent never has to hand-copy a helper from the source tree. This SHALL hold on every launch path that creates an agent worktree — the supervisor launch, the bare launch, the spec-driven launch with or without supervisor mode, and `git paw add` — since an agent's need for the helper does not depend on how its session was started. The set of packages SHALL be the worktree package selection (`coordination` whenever the broker is enabled; `docs-fetch` whenever `docs_base_url` is configured, mirroring the docs-fetch skill's injection gate), and the same selection SHALL govern which skills are injected into that agent's managed block. The scripts SHALL be sourced from the binary's embedded package registry (matching the running binary's version), and provisioning SHALL be idempotent — attaching to a fresh or reused worktree (re)writes the packages rather than failing.

#### Scenario: start provisions the broker helper into each worktree

- **GIVEN** a supervisor session with the broker enabled
- **WHEN** `git paw start` sets up an agent's worktree
- **THEN** `<worktree>/.git-paw/skills/coordination/scripts/broker.sh` exists and is executable before the agent's boot prompt is submitted
- **AND** `<worktree>/.git-paw/scripts/broker.sh` exists as its byte-identical compatibility copy
- **AND** the agent does not need to copy the helper from the source tree

#### Scenario: A bare launch provisions the helper

- **GIVEN** a broker-enabled session launched with `git paw start --branches <branch>` (no supervisor mode, no specs)
- **WHEN** the agent worktrees are set up
- **THEN** each worktree's `.git-paw/skills/coordination/scripts/broker.sh` exists and is executable before that agent's pane is launched

#### Scenario: A spec-driven launch without supervisor mode provisions the helper

- **GIVEN** a broker-enabled session launched with `--specs` and no supervisor mode
- **WHEN** the agent worktrees are set up
- **THEN** each worktree's `.git-paw/skills/coordination/scripts/broker.sh` exists and is executable before that agent's prompt is submitted
- **AND** no agent needs to copy the helper in order to publish its first status

#### Scenario: add provisions the helper into a mid-session worktree

- **WHEN** `git paw add <branch>` attaches a new agent worktree to a broker-enabled session
- **THEN** that worktree's `.git-paw/skills/coordination/` package exists with an executable `scripts/broker.sh`, identical to a start-time agent's

#### Scenario: docs-fetch package provisioned only when configured, on every launch path

- **WHEN** an agent worktree is set up, by any launch path, in a project that has configured `docs_base_url`
- **THEN** `<worktree>/.git-paw/skills/docs-fetch/` is provisioned alongside the coordination package
- **AND** that agent's managed block carries the docs-fetch skill
- **AND** when `docs_base_url` is unset, neither the docs-fetch package nor its skill is provisioned

#### Scenario: provisioning is idempotent and version-matched

- **WHEN** an agent worktree that already contains `.git-paw/skills/` is re-attached (a repeat `start`/`add`)
- **THEN** the packages and compatibility copies are refreshed from the running binary's embedded registry without error, so a worktree's helpers match the binary that launched the session
