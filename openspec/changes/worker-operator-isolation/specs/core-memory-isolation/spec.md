## ADDED Requirements

### Requirement: Workers launch without the operator's personal extensions

By default, every coding-agent worker git-paw launches SHALL run with its CLI family's operator isolation applied. The worker SHALL NOT load the operator's user-level hooks, plugins or settings, and SHALL NOT write the auto memory the operator's sessions share, wherever the family encodes how. Project-level and local settings of the repository SHALL still apply.

#### Scenario: A worker does not run the operator's user plugins or hooks

- **GIVEN** an operator whose user-level Claude Code settings enable a plugin and define a hook
- **WHEN** a claude-family worker is launched by `git paw start` with default configuration
- **THEN** the worker's CLI SHALL load neither that plugin nor that hook

#### Scenario: A worker does not write the shared auto memory

- **GIVEN** a claude-family worker launched with default configuration
- **WHEN** the worker's CLI session runs
- **THEN** its auto memory SHALL be off, so nothing is written under the repository's shared memory directory

#### Scenario: Repository settings still apply to an isolated worker

- **GIVEN** an allow rule in the main checkout's project-local Claude Code settings
- **WHEN** a claude-family worker is launched with default configuration
- **THEN** that rule SHALL still apply to the worker

### Requirement: Operator isolation does not apply to the orchestrator

Operator isolation SHALL apply only to worker launches. The operator's own driving session and the drive loop's headless orchestrator one-shot SHALL launch exactly as before.

#### Scenario: The orchestrator one-shot is unchanged

- **GIVEN** an unattended session whose drive loop hands a judgment call to a headless orchestrator one-shot of a claude-family CLI
- **WHEN** the one-shot is spawned
- **THEN** its command SHALL contain neither `--setting-sources` nor `CLAUDE_CODE_DISABLE_AUTO_MEMORY`
