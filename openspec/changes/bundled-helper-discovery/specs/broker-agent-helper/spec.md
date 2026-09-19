## ADDED Requirements

### Requirement: Bundled helpers read the per-repo discovery file's agent roster

The bundled helper scripts SHALL resolve agents from the per-repo discovery file (`.git-paw/sessions/<session>.json`) using that file's own shape — the `agents` roster, whose entries carry `branch_id`, `worktree_path`, `cli` and `pane_index` — and SHALL NOT read the global session-state shape (`worktrees` entries carrying `branch`). Mapping a pane's working directory to its agent SHALL therefore succeed against a discovery file written by any launch path. Test fixtures that model the discovery file SHALL use the same shape, so the helper's tests cannot pass against a document git-paw never writes.

A helper MAY additionally infer the session from its own environment when it runs inside the session's multiplexer, as a secondary path. When no session can be resolved, the helper SHALL say so explicitly rather than degrading every subcommand to an empty result.

#### Scenario: A pane path resolves to its agent from the discovery file

- **GIVEN** a per-repo discovery file whose `agents` roster contains an agent with `branch_id` `feat-x` and `worktree_path` `/repo/.git-paw/worktrees/feat-x`
- **WHEN** the helper resolves the agent for a pane whose working directory is inside that worktree
- **THEN** it SHALL resolve `feat-x`

#### Scenario: Fixtures use the shape git-paw writes

- **WHEN** the helper's tests construct a discovery file
- **THEN** they SHALL use the `agents` roster shape written by the per-repo discovery writer

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
