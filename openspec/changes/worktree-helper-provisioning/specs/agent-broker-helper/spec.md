## ADDED Requirements

### Requirement: Helpers provisioned into agent worktrees

`git paw start` and `git paw add` SHALL provision the bundled helper scripts an agent invokes into that agent's worktree at `.git-paw/scripts/`, making them present and executable before the agent boots, so the agent never has to hand-copy a helper from `assets/`. The scripts SHALL be sourced from the same bundled assets `git paw init` uses (matching the running binary's version), and provisioning SHALL be idempotent — attaching to a fresh or reused worktree (re)writes the scripts rather than failing. `broker.sh` SHALL be provisioned whenever the broker is enabled; `docs-fetch.sh` SHALL be provisioned whenever `docs_base_url` is configured (mirroring the docs-fetch skill's injection gate).

#### Scenario: start provisions the broker helper into each worktree

- **GIVEN** a supervisor session with the broker enabled
- **WHEN** `git paw start` sets up an agent's worktree
- **THEN** `<worktree>/.git-paw/scripts/broker.sh` exists and is executable before the agent's boot prompt is submitted
- **AND** the agent does not need to copy the helper from `assets/`

#### Scenario: add provisions the helper into a mid-session worktree

- **WHEN** `git paw add <branch>` attaches a new agent worktree to a broker-enabled session
- **THEN** that worktree's `.git-paw/scripts/broker.sh` exists and is executable, identical to a start-time agent's

#### Scenario: docs-fetch helper provisioned only when configured

- **WHEN** an agent worktree is set up in a project that has configured `docs_base_url`
- **THEN** `<worktree>/.git-paw/scripts/docs-fetch.sh` is provisioned alongside `broker.sh`
- **AND** when `docs_base_url` is unset, `docs-fetch.sh` is not provisioned

#### Scenario: provisioning is idempotent and version-matched

- **WHEN** an agent worktree that already contains `.git-paw/scripts/` is re-attached (a repeat `start`/`add`)
- **THEN** the helper scripts are refreshed from the running binary's bundled assets without error, so a worktree's helper matches the binary that launched the session
