## ADDED Requirements

### Requirement: Bundled helpers resolve session state from the authoritative location

The bundled helper scripts SHALL locate session state through the authoritative location defined by `session-state` ("Session state location is authoritative"), not through a repository-relative directory. A helper SHALL therefore resolve the active session when invoked from the repository root, with no requirement that it run inside the session's terminal multiplexer.

A helper MAY additionally fall back to inferring the session from its own environment when it is running inside the session, but that fallback SHALL NOT be the only path that works. When no session can be resolved, the helper SHALL say so explicitly rather than silently degrading every subcommand to an empty result.

#### Scenario: Helper resolves the session from the repository root

- **GIVEN** an active session for a repository, and a shell at that repository's root that is not inside the session's multiplexer
- **WHEN** a bundled helper subcommand that needs the session is invoked
- **THEN** it SHALL resolve the active session
- **AND** SHALL operate on that session's panes rather than reporting no session

#### Scenario: Empty repo-relative directory does not defeat discovery

- **GIVEN** an active session and a repository containing an empty `.git-paw/sessions/` directory
- **WHEN** a bundled helper resolves the session
- **THEN** it SHALL resolve the session from the authoritative location

#### Scenario: No active session is reported explicitly

- **GIVEN** a repository with no active session
- **WHEN** a bundled helper subcommand that needs the session is invoked
- **THEN** it SHALL report that no session was found
- **AND** SHALL NOT exit as though it had done its work

## MODIFIED Requirements

### Requirement: Helpers provisioned into agent worktrees

`git paw start` and `git paw add` SHALL provision the bundled helper scripts an agent invokes into that agent's worktree at `.git-paw/scripts/`, making them present and executable before the agent boots, so the agent never has to hand-copy a helper from `assets/`. This SHALL hold on every launch path that creates an agent worktree — the supervisor launch, the bare launch, the spec-driven launch with or without supervisor mode, and `git paw add` — since an agent's need for the helper does not depend on how its session was started. The scripts SHALL be sourced from the same bundled assets `git paw init` uses (matching the running binary's version), and provisioning SHALL be idempotent — attaching to a fresh or reused worktree (re)writes the scripts rather than failing. `broker.sh` SHALL be provisioned whenever the broker is enabled; `docs-fetch.sh` SHALL be provisioned whenever `docs_base_url` is configured (mirroring the docs-fetch skill's injection gate).

#### Scenario: start provisions the broker helper into each worktree

- **GIVEN** a supervisor session with the broker enabled
- **WHEN** `git paw start` sets up an agent's worktree
- **THEN** `<worktree>/.git-paw/scripts/broker.sh` exists and is executable before the agent's boot prompt is submitted
- **AND** the agent does not need to copy the helper from `assets/`

#### Scenario: A spec-driven launch without supervisor mode provisions the helper

- **GIVEN** a broker-enabled session launched with `--specs` and no supervisor mode
- **WHEN** the agent worktrees are set up
- **THEN** each worktree's `.git-paw/scripts/broker.sh` exists and is executable before that agent's prompt is submitted
- **AND** no agent needs to copy the helper from `assets/` in order to publish its first status

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
