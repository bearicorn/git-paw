## Purpose

Defines agent profiles: named configuration entries that state what an agent is for — a CLI pinned to its launch arguments (such as a model), a role skill, an approval level and a one-line purpose. A **worker** profile launches a normal coding agent (pane, branch, worktree) and is chosen by flag, configuration default, the interactive picker or spec frontmatter. A **delegate** profile runs headless and one-shot for the orchestrator at the repository root, under least-privilege bounds (default `manual`, never `full-auto`, root sessions only, no nesting, a time limit, a concurrency cap, no verdict authority), and returns a compact result. Profiles are configuration, never vendor agent-definition files.

## ADDED Requirements

### Requirement: Agent profiles are declared in `[profiles.<name>]`

The system SHALL accept an optional top-level `[profiles.<name>]` table whose entries carry: `kind` (required; `"worker"` or `"delegate"`), `description` (required; non-empty, one line), `cli` (required; a detected CLI id or a `[clis.<name>]` key), `args` (optional string), `skill` (optional for a worker, required for a delegate; a skill name resolvable by the skill resolver), `approval` (optional; `"manual"`, `"auto"` or `"full-auto"`), and `timeout_seconds` (optional positive integer, delegate profiles only). A profile name SHALL match `[A-Za-z0-9][A-Za-z0-9._-]{0,63}`. Configuration loading SHALL fail with an error naming the profile and the key when: `kind` is missing or not one of the two values; `description` is missing, empty or contains a line break; `cli` is missing; a delegate profile omits `skill`; a delegate profile sets `approval = "full-auto"`; `timeout_seconds` is `0` or is set on a worker profile; `skill` names `orchestrator` or its alias `supervisor`; or the name is invalid. Whether `cli` and `skill` resolve SHALL be checked when the profile is used, not at load. The global and repository files SHALL merge `[profiles]` by name, a repository entry replacing a global entry of the same name whole. Absent `[profiles]` and `default_profile`, pane commands, picker items, managed `AGENTS.md` blocks, session files and the human-readable status SHALL be identical to those produced before this requirement; `git paw status --json` SHALL differ only by the additive `profile` and `usage_by_profile` fields.

#### Scenario: A worker and a delegate profile load

- **GIVEN** `[profiles.routine]` with `kind = "worker"`, `description = "Routine implementation"`, `cli = "my-agent"`, `args = "--model small"`
- **AND** `[profiles.explorer]` with `kind = "delegate"`, `description = "Read-only codebase sweep"`, `cli = "my-agent"`, `skill = "explorer"`
- **WHEN** the config is loaded
- **THEN** both profiles SHALL be present with those values
- **AND** `explorer`'s effective approval SHALL be `manual` and its effective timeout 600 seconds

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

#### Scenario: The repository entry replaces the global entry

- **GIVEN** a global `[profiles.routine]` with `args = "--model a"` and a repository `[profiles.routine]` with no `args`
- **WHEN** the merged config is loaded
- **THEN** `routine` SHALL have no `args`

#### Scenario: No profiles leaves behavior unchanged

- **GIVEN** a config with no `[profiles]` table and no `default_profile`
- **WHEN** a session is started, recovered and reported with `git paw status` and `git paw status --json`
- **THEN** the pane commands, picker items, managed `AGENTS.md` blocks, session file and human-readable status SHALL be byte-identical to those produced before this requirement
- **AND** the status JSON SHALL differ only by `"profile": null` on each worktree and an empty `usage_by_profile`

### Requirement: A worker profile shapes its worker's launch

When a worker is launched with a worker profile, the system SHALL resolve the profile's `cli` to its CLI record and SHALL build the pane command with the one launch-command builder, appending the profile's `args` immediately after the CLI's own `args`. The worker's approval level SHALL be the profile's `approval` when set, otherwise the configured worker default; the profile's level SHALL apply to that worker only. When the profile's `skill` is set and is not `coordination`, the skill SHALL be resolved through the skill resolver against the repository root, rendered with the same inputs as the coordination skill, and written into the worker's managed `AGENTS.md` block after the coordination skill under a heading naming the role skill; the coordination skill SHALL be injected exactly as it would be without a profile. The worktree entry SHALL record the profile name. A profile whose `cli` is not available SHALL fail the launch with an error naming the profile and the CLI; a role skill that does not resolve SHALL fail the launch with an error naming the profile and the skill. Naming a delegate profile where a worker profile is expected SHALL fail with an error stating the profile is a delegate.

#### Scenario: Profile arguments follow the CLI's arguments

- **GIVEN** `[clis.agent] command = "/opt/agent"`, `args = "--quiet"` and `[profiles.routine]` with `kind = "worker"`, `cli = "agent"`, `args = "--model small"`
- **WHEN** a worker is launched with `routine` at `manual`
- **THEN** its pane command SHALL begin `/opt/agent --quiet --model small`

#### Scenario: Profile approval applies only to its worker

- **GIVEN** a default worker approval of `auto`, `[profiles.trusted]` with `approval = "full-auto"` on a CLI whose full-auto flags are `--yes`, and two workers — one launched with `trusted`, one with the plain CLI
- **WHEN** both pane commands are built
- **THEN** only the `trusted` worker's command SHALL contain `--yes`

#### Scenario: The role skill is injected after the coordination skill

- **GIVEN** the broker enabled, a repository skill `.agents/skills/reviewer/SKILL.md` with body `REVIEWER-ROLE`, and `[profiles.review]` with `kind = "worker"` and `skill = "reviewer"`
- **WHEN** a worker is launched with `review`
- **THEN** its managed `AGENTS.md` block SHALL contain the coordination skill followed by `REVIEWER-ROLE`

#### Scenario: Without a role skill the block is unchanged

- **GIVEN** a worker profile with no `skill`
- **WHEN** its managed `AGENTS.md` block is generated
- **THEN** it SHALL equal the block generated for the same CLI without a profile

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

The system SHALL accept a top-level `default_profile` naming a worker profile. On a launch that is not spec-driven, the worker for each branch SHALL be chosen as: `--profile` (every branch) or `--cli` (every branch), which are mutually exclusive; otherwise a `--preset`'s CLI; otherwise the interactive selection, whose CLI picker SHALL start on `default_profile` when set, else on `default_cli`. Where a launch path uses a session-wide fallback without a picker, the fallback SHALL be `default_profile` before `default_cli`. For `git paw add` the order SHALL be: `--profile` or `--cli`; the spec's `paw_profile` or `paw_cli` with `--from-spec`; the session default — the first worktree's recorded profile, else its CLI; `default_profile`; `default_cli`. A `default_profile` that names no profile, or names a delegate profile, SHALL fail the launch that consults it with an error naming `default_profile`.

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

#### Scenario: add inherits the session's profile

- **GIVEN** a running session whose first worktree was launched with profile `routine`
- **WHEN** `git paw add feat/c` runs with no `--cli` or `--profile`
- **THEN** the new worker SHALL launch with `routine`

#### Scenario: default_profile naming a delegate fails

- **GIVEN** `default_profile = "explorer"` where `explorer` is a delegate profile
- **WHEN** `git paw add feat/c` runs in a session whose entries carry no profile, with no flags
- **THEN** it SHALL fail with an error naming `default_profile`

### Requirement: Delegate invocation contract

`git paw delegate <name>` SHALL run the named delegate profile once, non-interactively, and SHALL refuse — with an error, spawning nothing — when: run from a linked worktree; the environment carries `GIT_PAW_DELEGATE` (no nested delegates); `<name>` is not a delegate profile; the profile's CLI is unavailable; or that CLI resolves no headless form, in which case the error SHALL name `[clis.<cli>].headless_args`. The process SHALL be spawned with: working directory = the repository's main working tree root; argv = the prefix the one launch-command builder produces for the CLI record — the executable, its own `args` followed by the profile's `args`, any sandbox arguments the record declares, and the approval flags for the profile's effective level — then the session-id arguments when the CLI resolves a `session_id_args` template (with a freshly generated id), then the headless arguments; the delegate document (see *Delegate document*) on stdin; the inherited environment plus `GIT_PAW_DELEGATE=<name>` and without `GIT_PAW_BROKER_URL`; and no controlling terminal. No authority credential SHALL appear in its argv or environment. The task text SHALL NOT appear in argv. A delegate SHALL never be driven by the drive loop, and git-paw SHALL answer none of its permission prompts.

#### Scenario: A delegate runs at the root with its arguments in order

- **GIVEN** `[clis.agent] command = "/opt/agent"` with `headless_args = "--batch"` and `[profiles.explorer]` with `kind = "delegate"`, `cli = "agent"`, `args = "--model small"`, `skill = "explorer"`
- **WHEN** `git paw delegate explorer --task "map the config loader"` runs from a subdirectory of the main working tree
- **THEN** exactly one process SHALL be spawned with argv `/opt/agent --model small --batch`
- **AND** its working directory SHALL be the main working tree root
- **AND** its stdin SHALL contain `map the config loader` and its argv SHALL NOT

#### Scenario: Environment marks the run and carries no broker URL or credential

- **WHEN** a delegate is spawned in a repository with an active broker-enabled session
- **THEN** its environment SHALL contain `GIT_PAW_DELEGATE=explorer`
- **AND** SHALL NOT contain `GIT_PAW_BROKER_URL` or the session authority credential

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

### Requirement: Delegate document

The stdin document SHALL consist of, in order: framing prose authored in the bundled asset `assets/delegate-framing.md`, rendered with the profile name, the repository root and the effective approval level; the profile's role skill resolved through the skill resolver against the repository root and rendered with the same gate-command and spec-backend inputs as the orchestrator skill; and the task. The framing SHALL state that the agent is a one-shot delegate, SHALL instruct it to return a compact result on stdout as its role skill specifies, and SHALL instruct it not to publish broker messages, start or stop sessions, run other delegates, or modify files unless its approval level permits edits. The task SHALL come from `--task <text>` or, when `--task` is absent, from standard input read to end of file; an empty or whitespace-only task SHALL be refused. The framing asset SHALL be covered by the no-language-leak audit and SHALL name no vendor product, model or project toolchain.

#### Scenario: The document carries framing, role skill and task in order

- **GIVEN** the delegate profile `spec-audit` with `skill = "spec-auditor"`
- **WHEN** it is invoked with `--task "audit feat/x at abc123, change add-x"`
- **THEN** the stdin document SHALL contain the framing, then the rendered `spec-auditor` skill, then the task text, in that order

#### Scenario: The task may arrive on stdin

- **WHEN** `git paw delegate explorer` runs with no `--task` and `map the broker` on its standard input
- **THEN** the delegate's document SHALL end with `map the broker`

#### Scenario: An empty task is refused

- **WHEN** `git paw delegate explorer --task "  "` runs
- **THEN** it SHALL exit non-zero and spawn nothing

#### Scenario: The framing is an audited asset

- **WHEN** the delegate document renderer's source is inspected and the no-language-leak audit runs
- **THEN** the framing sentences SHALL come from `assets/delegate-framing.md` and the audit SHALL cover it

### Requirement: Delegate runs are bounded

A delegate run SHALL be terminated, together with its process group, when the profile's `timeout_seconds` (default 600) elapses, and SHALL then be reported with outcome `timeout`. Per repository, at most four delegate runs at level `manual` SHALL run at once, and a run at level `auto` SHALL run only when no other delegate run is active and SHALL exclude every other run while it is active. A run that cannot be admitted SHALL be refused immediately — never queued — with an error naming the active runs' profiles; a run whose process no longer exists SHALL NOT count as active. Admission SHALL be decided by a pure function of the active runs and the requested level.

#### Scenario: Timeout kills the process group

- **GIVEN** a delegate profile with `timeout_seconds = 2` whose CLI never exits and spawns a child
- **WHEN** it is invoked
- **THEN** the process and its child SHALL be terminated after about 2 seconds
- **AND** the command SHALL exit non-zero reporting `timeout`

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

### Requirement: Delegate result and run record

The delegate's standard output SHALL be the result: `git paw delegate` SHALL print it on its own standard output, truncated to at most 65,536 bytes with a final line stating that the output was truncated and naming the run log. The delegate's standard error SHALL NOT be returned to the caller. Every run SHALL append its standard output and standard error to a per-run log under `.git-paw/logs/delegates/` and one JSON line to `.git-paw/logs/delegates/runs.jsonl` carrying the profile name, the CLI id, the assigned CLI session id or `null`, the start time, the duration in seconds, and the outcome (`ok`, `failed` with the exit code, or `timeout`). The task text and the result SHALL NOT be written to the JSON line. `git paw delegate` SHALL exit `0` only when the delegate's process exited `0` within its timeout.

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
