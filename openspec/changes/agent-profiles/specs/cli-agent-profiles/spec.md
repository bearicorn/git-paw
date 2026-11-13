## Purpose

Defines agent profiles: named configuration entries that state what an agent is for — a CLI pinned to its launch arguments (such as a model), a role skill, an approval level and a one-line purpose. A **worker** profile launches a normal coding agent (pane, branch, worktree) and is chosen by flag, configuration default, the interactive picker or spec frontmatter. A **delegate** profile runs headless and one-shot for the orchestrator at the repository root, under least-privilege bounds (default `manual`, never `full-auto`, root sessions only, no nesting, a time limit, a concurrency cap, no verdict authority), and returns a compact result. Profiles are configuration, never vendor agent-definition files.

## ADDED Requirements

### Requirement: Agent profiles are declared in `[profiles.<name>]`

The system SHALL accept an optional top-level `[profiles.<name>]` table. Each entry SHALL carry `kind` (`worker` or `delegate`), a one-line `description` and `cli` (a detected CLI id or `[clis.<name>]` key), and MAY carry `args`, `skill`, `approval` (`manual`, `auto` or `full-auto`) and, on a delegate, `timeout_seconds` (default 600). A delegate's approval SHALL default to `manual`. Whether `cli` and `skill` resolve SHALL be checked on use, not at load.

#### Scenario: A worker and a delegate profile load

- **GIVEN** `[profiles.routine]` with `kind = "worker"`, `description = "Routine implementation"`, `cli = "my-agent"`, `args = "--model small"`
- **AND** `[profiles.explorer]` with `kind = "delegate"`, `description = "Read-only codebase sweep"`, `cli = "my-agent"`, `skill = "explorer"`
- **WHEN** the config is loaded
- **THEN** both profiles SHALL be present with those values
- **AND** `explorer`'s effective approval SHALL be `manual` and its effective timeout 600 seconds

### Requirement: An invalid profile entry fails the configuration load

Configuration loading SHALL fail with an error naming the profile and the key when: `kind` is missing or not `worker` or `delegate`; `description` is missing, empty or contains a line break; `cli` is missing; a delegate omits `skill`, sets `approval = "full-auto"`, or carries in `args` a word that widens its CLI's permissions — any flag of that CLI's `auto` or `full-auto` approval flags (an `approval_args` override included), alone or as `flag=value`, a sandbox-disabling flag, or a permission-bypass value alone or as a `key=value` override; `timeout_seconds` is `0` or is set on a worker; `skill` names `orchestrator` or its alias `supervisor`; or the profile name does not match `[A-Za-z0-9][A-Za-z0-9._-]{0,63}`.

#### Scenario: Missing or unknown kind is rejected

- **GIVEN** `[profiles.x]` with no `kind`, and separately one with `kind = "helper"`
- **WHEN** each config is loaded
- **THEN** loading SHALL fail with an error naming `profiles.x` and `kind` and listing `worker` and `delegate`

#### Scenario: A multi-line description is rejected

- **GIVEN** `[profiles.x]` whose `description` contains a newline
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error naming `profiles.x` and `description`

#### Scenario: A full-auto delegate is rejected

- **GIVEN** `[profiles.fixer]` with `kind = "delegate"` and `approval = "full-auto"`
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error naming `profiles.fixer` and `approval`

#### Scenario: A delegate without a role skill is rejected

- **GIVEN** `[profiles.x]` with `kind = "delegate"` and no `skill`
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error naming `profiles.x` and `skill`

#### Scenario: The orchestrator skill is not a profile role

- **GIVEN** `[profiles.x]` with `skill = "orchestrator"` (or `"supervisor"`)
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error naming `profiles.x` and `skill`

#### Scenario: Timeout rules

- **GIVEN** a delegate profile with `timeout_seconds = 0`, and separately a worker profile with `timeout_seconds = 60`
- **WHEN** each config is loaded
- **THEN** loading SHALL fail with an error naming `timeout_seconds`

#### Scenario: An invalid profile name is rejected

- **GIVEN** a profile named `../x` (quoted TOML key)
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error naming the profile name

### Requirement: Profiles merge by name across configuration files

The global and repository configuration files SHALL merge `[profiles]` by name, a repository entry replacing a global entry of the same name whole, and SHALL merge `default_profile` the way they merge `default_cli`.

#### Scenario: The repository entry replaces the global entry

- **GIVEN** a global `[profiles.routine]` with `args = "--model a"` and a repository `[profiles.routine]` with no `args`
- **WHEN** the merged config is loaded
- **THEN** `routine` SHALL have no `args`

### Requirement: A configuration without profiles behaves as before

With no `[profiles]` table and no `default_profile`, pane commands, picker items, managed `AGENTS.md` blocks, session files and the human-readable status SHALL be identical to those produced before profiles existed, and `git paw status --json` SHALL differ only by the additive `profile` and `usage_by_profile` fields.

#### Scenario: No profiles leaves behavior unchanged

- **GIVEN** a config with no `[profiles]` table and no `default_profile`
- **WHEN** a session is started, recovered and reported with `git paw status` and `git paw status --json`
- **THEN** the pane commands, picker items, managed `AGENTS.md` blocks, session file and human-readable status SHALL be byte-identical to those produced before this requirement
- **AND** the status JSON SHALL differ only by `"profile": null` on each worktree and an empty `usage_by_profile`

### Requirement: A worker profile shapes its worker's launch command

When a worker is launched with a worker profile, the system SHALL resolve the profile's `cli` to its CLI record and SHALL build the pane command with the one launch-command builder, appending the profile's `args` immediately after the CLI's own `args`, ahead of the CLI's operator-isolation arguments, native sandbox arguments, approval flags and session arguments. A worker profile SHALL therefore receive its CLI's operator isolation exactly as a worker launched with that CLI alone does.

#### Scenario: Profile arguments follow the CLI's arguments

- **GIVEN** `[clis.agent] command = "/opt/agent"`, `args = "--quiet"` and `[profiles.routine]` with `kind = "worker"`, `cli = "agent"`, `args = "--model small"`
- **WHEN** a worker is launched with `routine` at `manual`
- **THEN** its pane command SHALL begin `/opt/agent --quiet --model small`

#### Scenario: A worker profile keeps its CLI's operator isolation

- **GIVEN** `[clis.opus] command = "claude"`, `family = "claude"`, `args = "--model opus"`, a worker approval of `manual`, and `[profiles.deep]` with `kind = "worker"`, `cli = "opus"`, `args = "--effort high"`
- **WHEN** the pane command of a worker launched with `deep` is built for a fresh launch assigned the session id `abc`
- **THEN** it SHALL be `env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_PLUGIN_DIRS= CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false claude --model opus --effort high --setting-sources project,local --session-id abc`

### Requirement: A worker profile's approval level applies to its worker only

A worker launched with a profile SHALL run at the profile's `approval` when set, otherwise at the configured worker default, and that level SHALL apply to that worker only. The native-sandbox refusal and the unsandboxed full-auto launch warning SHALL be evaluated at that worker's own level.

#### Scenario: Profile approval applies only to its worker

- **GIVEN** a default worker approval of `auto`, `[profiles.trusted]` with `approval = "full-auto"` on a CLI whose full-auto flags are `--yes`, and two workers — one launched with `trusted`, one with the plain CLI
- **WHEN** both pane commands are built
- **THEN** only the `trusted` worker's command SHALL contain `--yes`

#### Scenario: A full-auto profile on an unsandboxed CLI warns

- **GIVEN** a default worker approval of `manual` and `[profiles.trusted]` with `kind = "worker"`, `cli = "claude"`, `approval = "full-auto"`, and no `[clis.claude].sandbox`
- **WHEN** `git paw start --profile trusted --branches feat/a` runs
- **THEN** stderr SHALL name `claude` and say it runs at full-auto with no declared sandbox, once

### Requirement: A worker profile's role skill follows the coordination skill

When a worker profile's `skill` is set and is not `coordination`, the skill SHALL be resolved through the skill resolver against the repository root, rendered with the coordination skill's inputs, and written into the worker's managed `AGENTS.md` block after the coordination skill under a heading naming the role skill. The coordination skill SHALL be injected exactly as it would be without a profile.

#### Scenario: The role skill is injected after the coordination skill

- **GIVEN** the broker enabled, a repository skill `.agents/skills/reviewer/SKILL.md` with body `REVIEWER-ROLE`, and `[profiles.review]` with `kind = "worker"` and `skill = "reviewer"`
- **WHEN** a worker is launched with `review`
- **THEN** its managed `AGENTS.md` block SHALL contain the coordination skill followed by `REVIEWER-ROLE`

#### Scenario: Without a role skill the block is unchanged

- **GIVEN** a worker profile with no `skill`
- **WHEN** its managed `AGENTS.md` block is generated
- **THEN** it SHALL equal the block generated for the same CLI without a profile

### Requirement: A worker profile that cannot launch fails before any worktree exists

A worker profile whose `cli` is not available SHALL fail the launch with an error naming the profile and the CLI, and a role skill that does not resolve SHALL fail it with an error naming the profile and the skill, before any worktree is created. Naming a delegate profile where a worker profile is expected SHALL fail with an error stating the profile is a delegate.

#### Scenario: An unavailable profile CLI fails clearly

- **GIVEN** `[profiles.routine]` with `cli = "missing-agent"` and no such CLI
- **WHEN** `git paw start --profile routine --branches feat/a` runs
- **THEN** it SHALL fail with an error naming `routine` and `missing-agent`
- **AND** no worktree SHALL be created

#### Scenario: A delegate profile cannot launch a worker

- **GIVEN** `[profiles.explorer]` with `kind = "delegate"`
- **WHEN** `git paw add feat/a --profile explorer` runs
- **THEN** it SHALL fail with an error stating `explorer` is a delegate profile

### Requirement: Worker profile selection on launches without specs

The system SHALL accept a top-level `default_profile` naming a worker profile. On a launch that is not spec-driven, each branch's worker SHALL be chosen as: `--profile` or `--cli` (mutually exclusive) for every branch; otherwise a `--preset`'s CLI; otherwise the interactive selection, whose picker SHALL start on `default_profile` when set, else on `default_cli`. A session-wide fallback without a picker SHALL use `default_profile` before `default_cli`.

#### Scenario: --profile maps every branch

- **GIVEN** `[profiles.routine]` (worker)
- **WHEN** `git paw start --profile routine --branches feat/a,feat/b` runs
- **THEN** both workers SHALL launch with `routine` and no CLI prompt SHALL be shown

#### Scenario: --profile and --cli conflict

- **WHEN** `git paw start --profile routine --cli claude` is parsed
- **THEN** parsing SHALL fail naming both flags

#### Scenario: default_profile pre-selects in the picker

- **GIVEN** `default_profile = "routine"`, `default_cli = "beta"`, no `--cli` or `--profile`
- **WHEN** the user chooses uniform mode in the interactive selection
- **THEN** the CLI picker SHALL start on the `routine` entry

### Requirement: Worker selection for an added worker

For `git paw add` the worker SHALL be chosen as: `--profile` or `--cli`; with `--from-spec`, the spec's `paw_profile` or `paw_cli`; the session default — the first worktree's recorded profile, else its CLI; `default_profile`; `default_cli`. A `default_profile` that names no profile, or names a delegate profile, SHALL fail any launch that consults it with an error naming `default_profile`.

#### Scenario: add inherits the session's profile

- **GIVEN** a running session whose first worktree was launched with profile `routine`
- **WHEN** `git paw add feat/c` runs with no `--cli` or `--profile`
- **THEN** the new worker SHALL launch with `routine`

#### Scenario: default_profile naming a delegate fails

- **GIVEN** `default_profile = "explorer"` where `explorer` is a delegate profile
- **WHEN** `git paw add feat/c` runs in a session whose entries carry no profile, with no flags
- **THEN** it SHALL fail with an error naming `default_profile`

### Requirement: A delegate runs only where and when it is allowed

`git paw delegate <name>` SHALL run the named delegate profile once, non-interactively, and SHALL refuse, with an error and spawning nothing, when: run from a linked worktree; the environment carries `GIT_PAW_DELEGATE` (no nesting); `<name>` is not a delegate profile; the profile's CLI is unavailable; that CLI resolves no headless form, the error then naming `[clis.<cli>].headless_args`; or the native-sandbox rule would refuse a launch of that CLI at the profile's level.

#### Scenario: Refused from a linked worktree

- **GIVEN** an agent worktree of the repository
- **WHEN** `git paw delegate explorer --task x` runs there
- **THEN** it SHALL exit non-zero stating delegates run only from the main working tree
- **AND** no process SHALL be spawned

#### Scenario: No nested delegates

- **GIVEN** `GIT_PAW_DELEGATE=explorer` in the environment
- **WHEN** `git paw delegate explorer --task x` runs
- **THEN** it SHALL exit non-zero and spawn nothing

#### Scenario: A CLI without a headless form is refused

- **GIVEN** a delegate profile whose CLI resolves no `headless_args`
- **WHEN** it is invoked
- **THEN** the error SHALL name `[clis.<cli>].headless_args` and nothing SHALL be spawned

#### Scenario: A worker profile is not a delegate

- **GIVEN** `[profiles.routine]` with `kind = "worker"`
- **WHEN** `git paw delegate routine --task x` runs
- **THEN** it SHALL exit non-zero stating `routine` is a worker profile

### Requirement: A delegate's argv is built from its CLI record without a shell

A delegate's argv SHALL be built from its CLI record with no shell, each part split on whitespace, in this order: the CLI's `command`; the CLI's `args`; the profile's `args`; the CLI's operator-isolation arguments unless the CLI sets `inherit_operator_config = true`; the native sandbox arguments the record declares; the approval flags for the profile's level; the session-id arguments with a fresh id when the CLI has a template; then the headless arguments. The task SHALL NOT appear in argv.

#### Scenario: A delegate runs at the root with its arguments in order

- **GIVEN** `[clis.agent] command = "/opt/agent"` with `headless_args = "--batch"` and `[profiles.explorer]` with `kind = "delegate"`, `cli = "agent"`, `args = "--model small"`, `skill = "explorer"`
- **WHEN** `git paw delegate explorer --task "map the config loader"` runs from a subdirectory of the main working tree
- **THEN** exactly one process SHALL be spawned with argv `/opt/agent --model small --batch`
- **AND** its working directory SHALL be the main working tree root
- **AND** its stdin SHALL contain `map the config loader` and its argv SHALL NOT

#### Scenario: A delegate gets its CLI's operator isolation

- **GIVEN** `[clis.opus] command = "claude"`, `family = "claude"` and `[profiles.explorer]` with `kind = "delegate"`, `cli = "opus"`, `args = "--model small"`, `skill = "explorer"`
- **WHEN** `git paw delegate explorer --task x` runs
- **THEN** its argv SHALL be `claude --model small --setting-sources project,local --session-id <id> -p`
- **AND** its environment SHALL contain `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1`

#### Scenario: An opted-out CLI's delegate keeps the operator configuration

- **GIVEN** the same profile and `[clis.opus] inherit_operator_config = true`
- **WHEN** `git paw delegate explorer --task x` runs
- **THEN** its argv SHALL NOT contain `--setting-sources` and its environment SHALL NOT contain `CLAUDE_CODE_DISABLE_AUTO_MEMORY`

### Requirement: A delegate runs at the root with a marked, credential-free environment

A delegate SHALL be spawned with: working directory = the repository's main working tree root; the delegate document on stdin; its own process group, with no terminal on its standard streams; and the inherited environment plus `GIT_PAW_DELEGATE=<name>` and its CLI's operator-isolation environment, without `GIT_PAW_BROKER_URL` and without any authority credential. A delegate SHALL never be driven by the drive loop, and git-paw SHALL answer none of its permission prompts.

#### Scenario: An interrupted delegate takes its process group with it

- **GIVEN** a running delegate whose CLI started a child process
- **WHEN** the `git paw delegate` process receives SIGINT, SIGTERM or SIGHUP
- **THEN** the delegate's whole process group SHALL be killed and its active-run record removed
- **AND** `git paw delegate` SHALL exit with 128 plus the signal number (130, 143 or 129)

#### Scenario: Environment marks the run and carries no broker URL or credential

- **WHEN** a delegate is spawned in a repository with an active broker-enabled session
- **THEN** its environment SHALL contain `GIT_PAW_DELEGATE=explorer`
- **AND** SHALL NOT contain `GIT_PAW_BROKER_URL` or the session authority credential

### Requirement: Delegate document

The stdin document SHALL consist of, in order: framing prose from the bundled asset `assets/delegate-framing.md`, rendered with the profile name, the repository root and the effective approval level; the profile's role skill, resolved through the skill resolver against the repository root and rendered with the orchestrator skill's gate-command and spec-backend inputs; and the task.

#### Scenario: The document carries framing, role skill and task in order

- **GIVEN** the delegate profile `spec-audit` with `skill = "spec-auditor"`
- **WHEN** it is invoked with `--task "audit feat/x at abc123, change add-x"`
- **THEN** the stdin document SHALL contain the framing, then the rendered `spec-auditor` skill, then the task text, in that order

### Requirement: Delegate framing is an audited, vendor-neutral asset

The framing SHALL state that the agent is a one-shot delegate, SHALL instruct it to return a compact result on stdout as its role skill specifies, and SHALL instruct it not to publish broker messages, start or stop sessions, run other delegates, or modify files unless its approval level permits edits. The framing asset SHALL be covered by the no-language-leak audit and SHALL name no vendor product, model or project toolchain.

#### Scenario: The framing is an audited asset

- **WHEN** the delegate document renderer's source is inspected and the no-language-leak audit runs
- **THEN** the framing sentences SHALL come from `assets/delegate-framing.md` and the audit SHALL cover it

### Requirement: Delegate task input

The task SHALL come from `--task <text>` or, when `--task` is absent, from standard input read to end of file. An empty or whitespace-only task SHALL be refused with a non-zero exit and nothing spawned.

#### Scenario: The task may arrive on stdin

- **WHEN** `git paw delegate explorer` runs with no `--task` and `map the broker` on its standard input
- **THEN** the delegate's document SHALL end with `map the broker`

#### Scenario: An empty task is refused

- **WHEN** `git paw delegate explorer --task "  "` runs
- **THEN** it SHALL exit non-zero and spawn nothing

### Requirement: Delegate runs time out

A delegate run SHALL be terminated, together with its process group, when the profile's `timeout_seconds` (default 600) elapses, and SHALL then be reported with outcome `timeout`.

#### Scenario: Timeout kills the process group

- **GIVEN** a delegate profile with `timeout_seconds = 2` whose CLI never exits and spawns a child
- **WHEN** it is invoked
- **THEN** the process and its child SHALL be terminated after about 2 seconds
- **AND** the command SHALL exit non-zero reporting `timeout`

### Requirement: Delegate runs are admitted four readers or one writer at a time

Per repository, at most four delegate runs at level `manual` SHALL run at once, and a run at level `auto` SHALL run only when no other delegate run is active and SHALL exclude every other run while active. A run that cannot be admitted SHALL be refused immediately, never queued, with an error naming the active runs' profiles. Each active-run record SHALL carry the delegate's process group and its deadline (start + `timeout_seconds`) once spawned. A run SHALL count as active while its `git paw delegate` process or its delegate's process group is alive; a run with neither SHALL NOT count. A record whose process group is alive past its deadline (a delegate orphaned by a killed `git paw delegate`) SHALL have its group killed at the next admission and SHALL NOT count; past its deadline a record whose group is gone SHALL NOT count whatever its holder's state. A recorded process group SHALL be probed or signalled only when it is greater than 1 and is not git-paw's own group — any other value counts as no group — and SHALL be killed only after its leader is confirmed to still lead it and run the recorded delegate program, so a reused group id is never signalled. The post-spawn record SHALL be written atomically; if it cannot be written, the delegate's group SHALL be killed and the run SHALL fail. Admission SHALL be a pure function of the active runs and the requested level.

#### Scenario: An orphaned delegate is bounded by its deadline

- **GIVEN** an active-run record whose `git paw delegate` process has exited but whose delegate process group is alive
- **WHEN** admission is decided before the record's deadline
- **THEN** the run SHALL count as active
- **AND** when admission is decided after the deadline, its process group SHALL be killed and it SHALL NOT count

#### Scenario: A fifth read-only delegate is refused

- **GIVEN** four active `manual` delegate runs
- **WHEN** a fifth `manual` delegate is invoked
- **THEN** it SHALL be refused immediately with an error naming the four active profiles

#### Scenario: A writing delegate runs alone

- **GIVEN** one active `manual` delegate run
- **WHEN** an `auto` delegate is invoked
- **THEN** it SHALL be refused
- **AND** while an `auto` delegate run is active, any further delegate SHALL be refused

#### Scenario: A dead run does not block admission

- **GIVEN** a recorded active run whose process has exited without cleaning up
- **WHEN** admission is decided for a new run
- **THEN** the dead run SHALL NOT count toward the limit

### Requirement: Delegate result

The delegate's standard output SHALL be the result: `git paw delegate` SHALL print it on its own standard output, truncated to at most 65,536 bytes with a final line stating that the output was truncated and naming the run log, and SHALL NOT return the delegate's standard error. `git paw delegate` SHALL exit `0` only when the delegate's process exited `0` within its timeout.

#### Scenario: A successful run prints its result

- **GIVEN** a delegate whose CLI prints `MAP: src/config` and exits 0
- **WHEN** it is invoked
- **THEN** `git paw delegate` SHALL print `MAP: src/config` and exit 0
- **AND** `runs.jsonl` SHALL gain one line with outcome `ok` and the profile name

#### Scenario: Oversized output is truncated

- **GIVEN** a delegate whose CLI prints 200,000 bytes
- **WHEN** it is invoked
- **THEN** the printed result SHALL be at most 65,536 bytes followed by a truncation line naming the run log
- **AND** the run log SHALL hold the full output

#### Scenario: A failing delegate exits non-zero

- **GIVEN** a delegate whose CLI exits 3
- **WHEN** it is invoked
- **THEN** `git paw delegate` SHALL exit non-zero
- **AND** the ledger line SHALL record outcome `failed` with exit code 3

### Requirement: Delegate run record

Every run SHALL append its standard output and standard error to a per-run log under `.git-paw/logs/delegates/`, and one JSON line to `.git-paw/logs/delegates/runs.jsonl` carrying the profile name, the CLI id, the assigned CLI session id or `null`, the start time, the duration in seconds, and the outcome (`ok`, `failed` with the exit code, or `timeout`). The JSON line SHALL NOT contain the task text or the result.

#### Scenario: The ledger holds metadata only

- **GIVEN** a delegate invoked with `--task "TASK-MARKER"` whose CLI prints `RESULT-MARKER` and exits 0
- **WHEN** the run ends
- **THEN** the new `runs.jsonl` line SHALL carry the profile name, the CLI id and outcome `ok`, and SHALL contain neither `TASK-MARKER` nor `RESULT-MARKER`
- **AND** the run's log SHALL contain `RESULT-MARKER`

### Requirement: Delegate profiles can be listed

`git paw delegate --list` SHALL print every configured delegate profile with its name, description, CLI, role skill, effective approval level and whether its CLI resolves a headless form, and SHALL exit `0` when none are configured, saying so. Worker profiles SHALL NOT be listed.

#### Scenario: Listing shows delegates only

- **GIVEN** one worker profile and two delegate profiles, one of whose CLIs has no headless form
- **WHEN** `git paw delegate --list` runs
- **THEN** it SHALL print the two delegates with their descriptions
- **AND** SHALL mark the one without a headless form
- **AND** SHALL NOT print the worker profile

### Requirement: Agent profiles are documented

The mdBook SHALL carry an agent-profiles chapter covering worker and delegate profiles, the selection order, the delegate bounds and privileges, the bundled delegate role skills and usage attribution; the configuration reference SHALL document `[profiles.<name>]` with every key and `default_profile`; the CLI reference SHALL document `--profile` and `git paw delegate`; and the orchestrator workflow guide SHALL describe delegating gates and exploration to delegate profiles.

#### Scenario: The configuration reference documents every profile key

- **WHEN** the configuration reference is checked against the profile keys the config parser accepts
- **THEN** every key and `default_profile` SHALL be documented
- **AND** `mdbook build docs/` SHALL succeed
