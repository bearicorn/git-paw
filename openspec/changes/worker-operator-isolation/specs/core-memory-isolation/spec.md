## ADDED Requirements

### Requirement: Workers launch without the operator's personal extensions

By default, every coding-agent worker git-paw launches SHALL run with its CLI family's operator isolation applied. Wherever the family encodes how, the worker SHALL NOT load the operator's user-level hooks, plugins or settings, and SHALL NOT write the auto memory the operator's sessions share. The repository's project and local settings SHALL still apply, and git-paw SHALL read and write no agent-CLI settings file to achieve this.

#### Scenario: A worker does not run the operator's user plugins or hooks

- **GIVEN** an operator whose user-level Claude Code settings enable a plugin and define a hook
- **AND** whose environment sets `CLAUDE_CODE_PLUGIN_DIRS` to a directory of plugins that carry hooks
- **WHEN** a claude-family worker is launched by `git paw start` with default configuration
- **THEN** the worker's CLI SHALL be started with the `user` setting source excluded (`--setting-sources project,local`), so it loads neither that plugin nor that hook
- **AND** its environment SHALL carry `CLAUDE_CODE_PLUGIN_DIRS=` (empty), so it loads none of the plugins, nor their hooks, that the inherited variable points at

Test: `tests/worker_operator_isolation_e2e.rs::start_and_add_launch_claude_workers_with_identical_isolation` (Claude Code behaviour: design D5)

#### Scenario: A worker does not write the shared auto memory

- **GIVEN** a claude-family worker launched with default configuration
- **WHEN** the worker's CLI is started
- **THEN** its environment SHALL carry `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1`, so its auto memory is off and nothing is written under the repository's shared memory directory

Test: `tests/worker_operator_isolation_e2e.rs::start_and_add_launch_claude_workers_with_identical_isolation` (Claude Code behaviour: design D5)

#### Scenario: Repository settings still apply to an isolated worker

- **GIVEN** an allow rule in the main checkout's project-local Claude Code settings
- **WHEN** a claude-family worker is launched with default configuration
- **THEN** its setting sources SHALL still include `project` and `local`, so that rule still applies to the worker
- **AND** git-paw SHALL NOT have read or written any Claude Code settings file

Test: `tests/worker_operator_isolation_e2e.rs::start_and_add_launch_claude_workers_with_identical_isolation` (Claude Code behaviour: design D5)

### Requirement: Operator isolation is not applied to the orchestrator

The operator's own driving session and the drive loop's headless orchestrator one-shot SHALL launch exactly as before this requirement, without any operator-isolation environment or arguments.

#### Scenario: The orchestrator one-shot is unchanged

- **GIVEN** an unattended session whose drive loop hands a judgment call to a headless orchestrator one-shot of a claude-family CLI
- **WHEN** the one-shot is spawned
- **THEN** its argv SHALL NOT contain `--setting-sources`
- **AND** its added environment SHALL contain neither `CLAUDE_CODE_DISABLE_AUTO_MEMORY` nor `CLAUDE_CODE_PLUGIN_DIRS`

Test: `supervisor::escalation::tests::the_orchestrator_one_shot_is_unchanged`
