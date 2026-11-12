## MODIFIED Requirements

### Requirement: Helpers provisioned into agent worktrees

`git paw start` and `git paw add` SHALL provision the bundled skill packages an agent uses into that agent's worktree at `.git-paw/skills/<package>/` — each package's rendered `SKILL.md` and its helper scripts — together with the helpers' compatibility copies at `.git-paw/scripts/`, making them present and executable before the agent boots, so the agent never has to hand-copy a helper from the source tree. This SHALL hold on every launch path that creates an agent worktree — the supervisor launch, the bare launch, the spec-driven launch with or without supervisor mode, and `git paw add` — since an agent's need for the helper does not depend on how its session was started. The set of packages SHALL be the worktree package selection (`coordination` whenever the broker is enabled; `docs-fetch` whenever `docs_base_url` is configured, mirroring the docs-fetch skill's injection gate), and the same selection SHALL govern which skills are injected into that agent's managed block. The scripts SHALL be sourced from the binary's embedded package registry (matching the running binary's version), and provisioning SHALL be idempotent — attaching to a fresh or reused worktree (re)writes any package file that is missing or differs rather than failing.

#### Scenario: start provisions the broker helper into each worktree

- **GIVEN** a supervisor session with the broker enabled
- **WHEN** `git paw start` sets up an agent's worktree
- **THEN** `<worktree>/.git-paw/skills/coordination/scripts/broker.sh` exists and is executable before the agent's boot prompt is submitted
- **AND** `<worktree>/.git-paw/scripts/broker.sh` exists as its byte-identical compatibility copy
- **AND** the agent does not need to copy the helper from the source tree

Test: `tests/worktree_helper_provisioning.rs::supervisor_launch_provisions_broker_helper_into_each_worktree`, `init::tests::provision_writes_executable_broker_helper_when_broker_enabled`

#### Scenario: A bare launch provisions the helper

- **GIVEN** a broker-enabled session launched with `git paw start --branches <branch>` (no supervisor mode, no specs)
- **WHEN** the agent worktrees are set up
- **THEN** each worktree's `.git-paw/skills/coordination/scripts/broker.sh` exists and is executable before that agent's pane is launched

Test: `tests/worktree_helper_provisioning.rs::bare_launch_provisions_broker_helper_into_each_worktree`

#### Scenario: A spec-driven launch without supervisor mode provisions the helper

- **GIVEN** a broker-enabled session launched with `--specs` and no supervisor mode
- **WHEN** the agent worktrees are set up
- **THEN** each worktree's `.git-paw/skills/coordination/scripts/broker.sh` exists and is executable before that agent's prompt is submitted
- **AND** no agent needs to copy the helper in order to publish its first status

Test: `tests/worktree_helper_provisioning.rs::spec_driven_launch_provisions_broker_helper_into_each_worktree`

#### Scenario: add provisions the helper into a mid-session worktree

- **WHEN** `git paw add <branch>` attaches a new agent worktree to a broker-enabled session
- **THEN** that worktree's `.git-paw/skills/coordination/` package exists with an executable `scripts/broker.sh`, identical to a start-time agent's

Test: `tests/worktree_helper_provisioning.rs::add_provisions_broker_helper_into_the_new_worktree`

#### Scenario: docs-fetch helper provisioned only when configured

- **WHEN** an agent worktree is set up in a project that has configured `docs_base_url`
- **THEN** `<worktree>/.git-paw/skills/docs-fetch/scripts/docs-fetch.sh` is provisioned alongside the coordination package, with its compatibility copy `<worktree>/.git-paw/scripts/docs-fetch.sh`
- **AND** when `docs_base_url` is unset, neither the docs-fetch package nor `<worktree>/.git-paw/scripts/docs-fetch.sh` is provisioned

Test: `init::tests::provision_writes_docs_fetch_only_when_configured`, `tests/worktree_helper_provisioning.rs::re_attaching_a_surviving_worktree_refreshes_the_helper`

#### Scenario: The docs-fetch skill is injected on every launch path that provisions it

- **GIVEN** a broker-enabled project that has configured `docs_base_url`
- **WHEN** an agent worktree is set up by the bare launch, the spec-driven launch, the supervisor launch or `git paw add`
- **THEN** that agent's managed block carries the docs-fetch skill alongside the coordination skill

Test: `tests/worktree_helper_provisioning.rs::bare_launch_provisions_broker_helper_into_each_worktree`, `tests/worktree_helper_provisioning.rs::spec_driven_launch_provisions_broker_helper_into_each_worktree`, `tests/worktree_helper_provisioning.rs::supervisor_launch_provisions_broker_helper_into_each_worktree`, `tests/worktree_helper_provisioning.rs::add_provisions_broker_helper_into_the_new_worktree`

#### Scenario: provisioning is idempotent and version-matched

- **WHEN** an agent worktree that already contains `.git-paw/skills/` is re-attached (a repeat `start`/`add`)
- **THEN** the packages and compatibility copies are refreshed from the running binary's embedded registry without error, so a worktree's helpers match the binary that launched the session

Test: `init::tests::provision_is_idempotent_and_refreshes_stale_helper`, `tests/worktree_helper_provisioning.rs::re_attaching_a_surviving_worktree_refreshes_the_helper`

## ADDED Requirements

### Requirement: Helpers resolve session state from the main checkout

The bundled helpers SHALL resolve the session state they keep under `.git-paw/` — the discovery files in `.git-paw/sessions/` and `sweep.sh`'s own progress, dedup and approve-claim files — in the repository's main working tree (the checkout that owns the git common directory), not in the top level of the current directory's checkout. A helper run from inside a linked worktree SHALL therefore find the same session as one run at the repository root, wherever the helper is installed.

#### Scenario: sweep.sh run inside an agent's worktree finds the session

- **GIVEN** a session whose discovery file is `<repo>/.git-paw/sessions/<session>.json` and a linked agent worktree that has no `.git-paw/sessions/`
- **WHEN** `<repo>/.git-paw/skills/orchestrator/scripts/sweep.sh status` is run with that worktree as the current directory
- **THEN** it SHALL resolve that session and its agent roster
- **AND** it SHALL NOT report that no session was found

Test: `tests/sweep_sh_session_discovery.rs::sweep_sh_run_inside_an_agents_worktree_finds_the_session`

#### Scenario: The approve claim is shared between the root and a worktree

- **GIVEN** an approve claim taken for a pane by `sweep.sh approve` run at the repository root
- **WHEN** `sweep.sh approve` for the same pane is run with a linked worktree as the current directory
- **THEN** it SHALL see the existing claim under `<repo>/.git-paw/tmp/`

Test: `tests/sweep_sh_approve_claim.rs::the_approve_claim_is_shared_between_the_root_and_a_worktree`

#### Scenario: broker.sh still identifies its agent by the current worktree

- **GIVEN** a linked worktree on branch `feat/x`
- **WHEN** `.git-paw/skills/coordination/scripts/broker.sh status "x"` is run there without `--agent`
- **THEN** it SHALL publish as agent `feat-x`

Test: `tests/helper_publish_contract.rs::broker_sh_still_identifies_its_agent_by_the_current_worktree`
