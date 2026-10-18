## MODIFIED Requirements

### Requirement: No-orchestrator fallback preserves prior behavior

The drive loop SHALL record every judgment call it cannot make mechanically — a live permission prompt, an ambiguous `agent.question`, a `committed`/`done` artifact's merge decision, and a correction-exhausted branch — as a broker review item first, in every configuration, and SHALL NOT inject anything into any surface. When a headless orchestrator invocation is resolvable and `[supervisor.escalation] enabled` is true, the loop SHALL additionally hand every such call except a live permission prompt — that is, a question, a merge decision or a correction-exhausted branch — to a headless one-shot invocation (per "Judgment calls are handed to a headless orchestrator invocation"). The early correction heads-up (a slow-converging branch, which requires no action yet) SHALL stay a broker record only and SHALL NOT be handed to an invocation. A live permission prompt SHALL never be handed to an invocation: it stays on the broker, in the exit summary and in the manual-decision log (`supervisor-unattended-operation`, *Every live permission prompt is escalated without blocking the wave*). When no invocation is resolvable, or escalation is disabled, the loop SHALL take no further action — the broker record and heartbeat re-engage the human exactly as in the pre-relocation no-pane behaviour — and SHALL print one notice at launch stating why headless hand-off is unavailable.

#### Scenario: Escalation is always recorded on the broker

- **GIVEN** any `--unattended` run and a worker publishing an ambiguous `agent.question`
- **WHEN** the drive loop escalates it
- **THEN** the loop SHALL record the escalation as a broker review item before any invocation
- **AND** SHALL continue progressing the other workers without blocking

#### Scenario: A permission prompt stays on the broker with a resolvable orchestrator

- **GIVEN** a resolvable orchestrator CLI, `[supervisor.escalation] enabled = true`, and a worker showing a live permission prompt
- **WHEN** the drive loop escalates it
- **THEN** the escalation SHALL be recorded on the broker and in the manual-decision log
- **AND** no orchestrator process SHALL be spawned for it

#### Scenario: No resolvable invocation falls back to broker-only

- **GIVEN** a config with no `[supervisor].cli` and no `default_cli`
- **WHEN** an `--unattended` run starts and later escalates a judgment call
- **THEN** the launch output SHALL contain a notice that headless hand-off is unavailable and why
- **AND** the escalation SHALL be recorded on the broker
- **AND** no orchestrator process SHALL be spawned

#### Scenario: Escalation disabled falls back to broker-only

- **GIVEN** `[supervisor.escalation] enabled = false` and a resolvable orchestrator CLI
- **WHEN** the drive loop escalates a judgment call
- **THEN** no orchestrator process SHALL be spawned
- **AND** the escalation SHALL be recorded on the broker

## ADDED Requirements

### Requirement: Judgment calls are handed to a headless orchestrator invocation

The drive loop SHALL, for each question, merge decision and correction-exhausted branch it records, when a headless invocation is resolvable, spawn the orchestrator CLI once, non-interactively, with: working directory = the repository root; argv = the CLI command, then the approval flags resolved from `[supervisor].approval` (falling back to `agent_approval`), then the resolved headless arguments; the escalation context delivered on **stdin** and never on argv; the environment carrying `GIT_PAW_BROKER_URL`; no controlling terminal; and stdout/stderr redirected to `.git-paw/logs/<session>/orchestrator-escalations.log`. The loop SHALL NOT wait for the invocation before continuing its sweep. The invocation SHALL receive no authority credential in argv or environment; it publishes verdicts through the root-side helpers.

A live permission prompt SHALL NOT be handed to an invocation. The one-shot runs with no human to relay the prompt to, and no git-paw process answers a permission prompt (`approval-auto`, *Approval keystrokes are sent only by an explicit approve action*). The early correction heads-up SHALL NOT be handed to an invocation either.

#### Scenario: An ambiguous question spawns a one-shot with context on stdin

- **GIVEN** an `--unattended` run with `[supervisor].cli = "claude"`, no override, and a worker publishing an ambiguous `agent.question`
- **WHEN** the drive loop escalates it
- **THEN** exactly one process SHALL be spawned with argv beginning `claude` and containing `-p`
- **AND** its working directory SHALL be the repository root
- **AND** the escalation context SHALL be written to its stdin and SHALL NOT appear in its argv
- **AND** the sweep SHALL continue without awaiting the process

#### Scenario: A permission prompt spawns no one-shot

- **GIVEN** an `--unattended` run with a resolvable orchestrator CLI and a worker showing a live permission prompt
- **WHEN** the drive loop escalates it
- **THEN** no orchestrator process SHALL be spawned
- **AND** no keystroke SHALL be sent to the worker's pane

#### Scenario: The early correction heads-up spawns no one-shot

- **GIVEN** an `--unattended` run with a resolvable orchestrator CLI and a worker whose correction cycles reach the early heads-up threshold below its budget
- **WHEN** the drive loop records the heads-up
- **THEN** the heads-up SHALL be recorded on the broker
- **AND** no orchestrator process SHALL be spawned for it

#### Scenario: A correction-exhausted branch spawns one one-shot

- **GIVEN** an `--unattended` run with a resolvable orchestrator CLI and a worker whose correction budget is spent
- **WHEN** the drive loop applies the exhaustion policy and later sweeps observe the same branch still failing
- **THEN** exactly one one-shot SHALL be spawned for that exhaustion

#### Scenario: Approval flags precede headless arguments

- **GIVEN** `[supervisor].approval = "full-auto"` and `cli = "claude"`
- **WHEN** a one-shot is spawned
- **THEN** argv SHALL be `claude --dangerously-skip-permissions -p` (approval flags before `-p`)

#### Scenario: Merge decision spawns a one-shot

- **GIVEN** a resolvable orchestrator and a worker publishing an `agent.artifact` with status `committed`
- **WHEN** the drive loop processes it
- **THEN** a one-shot SHALL be spawned whose stdin names the agent, the artifact status and the verification-and-merge responsibility
- **AND** no one-shot SHALL be spawned for an artifact of any other status

#### Scenario: No credential in the invocation's argv or environment

- **WHEN** a one-shot is spawned
- **THEN** neither its argv nor its environment SHALL contain the session authority credential

### Requirement: Headless invocation resolution

The system SHALL resolve the headless orchestrator invocation as: CLI = `[supervisor].cli`, falling back to `default_cli`; headless arguments = the resolved `headless_args` template for that CLI (per `cli-resolution`); a headless invocation is resolvable only when both resolve. Resolution SHALL happen once at drive-loop start and SHALL be reported in the launch output (the CLI and its headless form, or the reason none resolved).

#### Scenario: Claude resolves through the built-in row

- **GIVEN** `[supervisor].cli = "claude"` and no `[clis.claude].headless_args`
- **WHEN** the drive loop starts
- **THEN** the launch output SHALL report headless hand-off as `claude -p`

#### Scenario: Unknown CLI without an override is unresolvable

- **GIVEN** `[supervisor].cli = "my-agent"` and no `[clis.my-agent].headless_args`
- **WHEN** the drive loop starts
- **THEN** the launch output SHALL state that `my-agent` has no headless form and name the `[clis.my-agent].headless_args` key

### Requirement: One-shot invocations are serialized, bounded and deduplicated

The drive loop SHALL run at most one headless invocation at a time per session, queueing further escalations FIFO; SHALL hand off each question or merge decision at most once per broker message observed (messages are consumed once by the loop's cursor) and each correction-exhausted branch at most once per correction episode; SHALL key each queued escalation by its kind, its agent id and its discriminator (the question text for a question, the artifact status for a merge decision, none for correction-exhausted) and SHALL NOT queue an escalation whose key is already queued or in flight, so a worker re-publishing the same question or status while its one-shot is pending does not spawn a second invocation; and SHALL kill the invocation's process group and record a timeout once `[supervisor.escalation] timeout_seconds` elapses. Wind-down (a completion condition reached) SHALL stop dequeuing new invocations but SHALL let an in-flight one finish or time out.

#### Scenario: Concurrent escalations are serialized

- **GIVEN** two workers escalating within the same sweep
- **WHEN** the drive loop hands them off
- **THEN** the second invocation SHALL NOT start until the first has exited or timed out

#### Scenario: A re-observed merge decision does not respawn

- **GIVEN** a merge decision whose one-shot is in flight
- **WHEN** the same agent publishes another `agent.artifact` with the same status `committed` and the next sweep observes it
- **THEN** no additional invocation SHALL be queued

#### Scenario: A repeated merge decision after completion is a new event

- **GIVEN** a merge decision for an agent whose one-shot has already exited
- **WHEN** the same agent publishes a later `agent.artifact` with status `committed`
- **THEN** one new invocation SHALL be queued for it

#### Scenario: Timeout kills the process group

- **GIVEN** `timeout_seconds = 5` and a one-shot that never exits
- **WHEN** 5 seconds elapse
- **THEN** the process and its children SHALL be terminated
- **AND** the outcome SHALL be recorded as `timeout`

### Requirement: Invocation outcomes are recorded

On completion or timeout of a headless invocation the drive loop SHALL publish an `agent.status` with `agent_id = "supervisor"`, `status = "working"`, a message naming the escalation key and outcome, and a `detail` object carrying `escalation_key`, `exit_code` (an integer, or the string `"timeout"`) and `duration_seconds`. The exit summary SHALL include the number of invocations attempted and a per-outcome count.

#### Scenario: Completed invocation is published

- **GIVEN** a one-shot that exits 0 after 42 seconds for escalation key `K`
- **WHEN** the drive loop observes the exit
- **THEN** it SHALL publish `agent.status` from `supervisor` whose `detail` contains `"escalation_key": "K"`, `"exit_code": 0` and a `duration_seconds` near 42

#### Scenario: Exit summary counts invocations

- **GIVEN** a drive loop that ran three one-shots (two exit 0, one timeout)
- **WHEN** the summary is printed
- **THEN** it SHALL state 3 invocations with 2 completed and 1 timed out

### Requirement: Escalation context is agnostic and points at the skill

The stdin document SHALL contain: the escalation kind; the agent id; the question text, artifact status, or branch detail as applicable; the repository root; the paths `.git-paw/skills/orchestrator/SKILL.md` and `.git-paw/AGENTS.local.md`; the broker URL; an instruction to act only through the bundled helpers or the MCP tools and to publish the outcome to the broker; and an instruction never to run `sweep.sh approve` or otherwise answer a permission prompt, because no human is present to decide one. Its body text SHALL be assembled from the retained pure renderers (`Escalation::handoff_text`, `question_handoff_text`, `merge_handoff_text`) and framing sentences authored in the bundled asset `assets/escalation-framing.md`; it SHALL contain no consumer-specific toolchain token. The framing SHALL override the renderers' "escalate to the human" wording: it SHALL state that, for the one-shot, escalating means publishing the reason on the broker and exiting, leaving the review item for the human. For a merge decision the framing SHALL carry the unattended merge rule of the exported orchestrator skill: verify, merge fast-forward only once every gate is verified, and escalate only on a merge conflict, an exhausted correction cycle or a regression — it SHALL NOT instruct the one-shot to ask a human before merging.

#### Scenario: Context names the skill, the sidecar and the broker

- **WHEN** an escalation context is rendered for any judgment call
- **THEN** it SHALL contain `.git-paw/skills/orchestrator/SKILL.md`, `.git-paw/AGENTS.local.md` and the broker URL

#### Scenario: Context forbids answering permission prompts

- **WHEN** an escalation context is rendered for any judgment call
- **THEN** it SHALL instruct the one-shot not to run `sweep.sh approve` and not to answer any permission prompt

#### Scenario: Context redefines escalation for a one-shot

- **WHEN** an escalation context is rendered for any handed-off kind
- **THEN** it SHALL state that escalating means publishing the reason on the broker and exiting
- **AND** it SHALL NOT leave the renderers' "escalate to the human" as the only instruction for an undecidable call

#### Scenario: Merge-decision context follows the unattended merge rule

- **WHEN** an escalation context is rendered for a merge decision
- **THEN** it SHALL instruct the one-shot to merge fast-forward only once every gate is verified
- **AND** to escalate only on a merge conflict, an exhausted correction cycle or a regression
- **AND** it SHALL NOT instruct the one-shot to ask a human before merging

#### Scenario: Question context carries the question

- **GIVEN** an ambiguous `agent.question` from `feat-x` reading "RS256 or HS256?"
- **WHEN** its context is rendered
- **THEN** it SHALL contain `feat-x` and `RS256 or HS256?`

#### Scenario: Framing prose lives in the bundled asset

- **WHEN** the context renderer's source is inspected
- **THEN** its framing sentences SHALL come from `assets/escalation-framing.md` via `include_str!`, not from compiled string literals
