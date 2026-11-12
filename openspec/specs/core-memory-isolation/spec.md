# core-memory-isolation Specification

## Purpose
Keeps coding agents from writing outside their worktree into the operator's
configuration and memory territory. Defines a config-driven protected-path set
(the home-level Claude-format config dir, `CLAUDE_CONFIG_DIR`, every configured
`[clis.<name>].settings_path` parent, their `projects/**/memory` subtrees, and
the host repository's `.claude/` and `.git-paw/` when the agent runs in a
separate worktree), requires the safe-command classifier to escalate writes into
that set as a terminal danger-class refusal (never auto-approved), and scopes
agent memory guidance to the worktree in the bundled coordination and supervisor
skills.

## Requirements

### Requirement: Coding-agent memory guidance is worktree-scoped

The bundled coordination skill SHALL include a memory-isolation section instructing coding agents that: all persistent artifacts they create (memory files, notes, scratch state, configuration) SHALL live inside their own worktree; operator configuration directories (home-level CLI config dirs, the host repository's `.claude/` and `.git-paw/`) are off-limits for writes; and when a task appears to require writing outside the worktree, the agent SHALL publish `agent.question` and wait instead of writing. The section SHALL be spec-engine-agnostic and CLI-agnostic (export policy).

#### Scenario: Rendered coordination skill carries the section

- **WHEN** the coordination skill template is rendered for any consumer project
- **THEN** the output SHALL contain the memory-isolation guidance (worktree-scoped writes, off-limits operator dirs, question-instead-of-write)

#### Scenario: Guidance names no spec engine or CLI product

- **WHEN** the memory-isolation section is inspected
- **THEN** it SHALL NOT reference OpenSpec, Spec Kit, or any specific CLI product name

### Requirement: Supervisor treats out-of-worktree writes as violations

The bundled supervisor skill SHALL instruct the supervisor to treat an observed out-of-worktree write attempt by a coding agent as a boundary violation. Such an attempt is a permission prompt or escalation that targets a path outside the agent's worktree, or any other observation of such a write. The supervisor SHALL send the agent scoped `agent.feedback` naming the worktree boundary and the attempted path, SHALL relay any pending prompt for it to the operator instead of approving it, and SHALL escalate to the operator when the same agent repeats the attempt.

#### Scenario: Supervisor skill carries the violation procedure

- **WHEN** the supervisor skill template is rendered
- **THEN** the output SHALL contain the out-of-worktree write violation procedure (scoped feedback, escalate on repeat)
- **AND** it SHALL NOT describe the violation as detected by a git-paw classifier rule

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

### Requirement: An opted-out worker still never writes the shared auto memory

A worker whose CLI sets `inherit_operator_config = true` SHALL still launch with its family's auto memory turned off, so no worker writes the memory every worktree of the repository shares, whatever config folder its CLI uses.

#### Scenario: A worker-config-folder CLI keeps memory off

- **GIVEN** `[clis.claude-worker]` with `family = "claude"` and `inherit_operator_config = true`
- **WHEN** a worker using it is launched by `start`, `add` or recovery
- **THEN** its environment SHALL carry `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1`

Test: `tests/worker_operator_isolation_e2e.rs::an_opted_out_worker_keeps_auto_memory_off`
