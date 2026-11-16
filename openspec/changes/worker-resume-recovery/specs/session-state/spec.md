## MODIFIED Requirements

### Requirement: Recovery relaunches a worker with its recorded session id

When `git paw start` recovers a stopped session, the system SHALL relaunch each worktree with the pane command built by the single launch-command builder (per `cli-provider`) for the entry's recorded CLI: the executable, the CLI's `args`, the approval flags for the configured `agent_approval`, and — when the entry carries a `cli_session_id`, its CLI resolves a resume template, AND the recorded conversation is not known to be absent — that template's arguments, so the worker's conversation context is restored. A worktree whose entry has no id, or whose CLI resolves no resume template, SHALL be relaunched without resume arguments. Recovery SHALL NOT fail because a resume template is absent.

#### Scenario: Recovered Claude worker is resumed

- **GIVEN** a stopped session whose worktree entry for `feat/x` has `cli = "claude"` and
  `cli_session_id = Some("<uuid>")`, no `[clis.claude]` override, and a Claude transcript for `<uuid>`
- **WHEN** the session is recovered
- **THEN** the pane command for `feat/x` SHALL contain `--resume <uuid>`

#### Scenario: Recovered worker keeps its approval flags

- **GIVEN** a stopped session whose entry for `feat/x` has `cli = "claude"` and
  `agent_approval = "auto"` in the current config
- **WHEN** the session is recovered
- **THEN** the pane command for `feat/x` SHALL contain `--permission-mode acceptEdits`

#### Scenario: Entry without an id launches fresh

- **GIVEN** a stopped session whose worktree entry has `cli_session_id = None`
- **WHEN** the session is recovered
- **THEN** the pane command SHALL contain no resume arguments
- **AND** it SHALL equal the command a fresh launch of that CLI at the same approval level would
  build, without session-id arguments

#### Scenario: CLI without a resume template launches fresh

- **GIVEN** a worktree entry with `cli = "my-agent"`, `cli_session_id = Some("<id>")`, and no
  `[clis.my-agent].resume_args`
- **WHEN** the session is recovered
- **THEN** the pane command SHALL contain no resume arguments
- **AND** recovery SHALL succeed

## ADDED Requirements

### Requirement: Worktree entry records whether its conversation started

`WorktreeEntry` SHALL carry an optional `conversation_started` flag, omitted from the file when unset. It SHALL be set to false when git-paw assigns the entry a session id, and to true when git-paw submits that worker's first prompt into a ready pane. A session file without the field SHALL load with it unset, meaning unknown.

#### Scenario: Assignment records not started

- **GIVEN** `git paw start` launching `feat/x` with `claude`
- **WHEN** the session file is written before the boot prompt is submitted
- **THEN** the `feat/x` entry SHALL carry a `cli_session_id` and `conversation_started` false

#### Scenario: Submitting the first prompt records started

- **GIVEN** the `feat/x` pane passed its readiness gate
- **WHEN** git-paw submits its boot prompt
- **THEN** the saved entry SHALL carry `conversation_started` true

#### Scenario: Older files read as unknown

- **GIVEN** a session file whose entries have no `conversation_started` key
- **WHEN** it is loaded
- **THEN** loading SHALL succeed and every entry's flag SHALL be unset

### Requirement: Recovery decides from the CLI's answer, then the marker

For an entry with a session id and a resume template, recovery SHALL resume when the CLI family reports the conversation exists. It SHALL relaunch fresh when the family reports it absent, or when the family cannot tell and `conversation_started` is false. In every other case it SHALL resume. This decision SHALL be a pure function of those facts, and the resume summary SHALL render it.

#### Scenario: Transcript absent relaunches fresh

- **GIVEN** a stopped session whose `claude` entry has id `abc` and no Claude transcript `abc.jsonl` exists
- **WHEN** the user runs `git paw start` or `git paw start --unattended`
- **THEN** the pane command SHALL contain `--session-id` with a new id and SHALL NOT contain `--resume abc`

#### Scenario: Unknown family uses the marker

- **GIVEN** an entry for a CLI that reports nothing about conversations, with a resume template, an id and `conversation_started` false
- **WHEN** the session is recovered
- **THEN** the worker SHALL be relaunched fresh

#### Scenario: Unknown everything resumes as before

- **GIVEN** such an entry with `conversation_started` unset
- **WHEN** the session is recovered
- **THEN** the pane command SHALL carry the resume arguments for its id

### Requirement: A fresh relaunch records a new session id

When recovery relaunches a worker fresh and its CLI has a session-id template, it SHALL assign a new session id and launch with the session-id arguments. It SHALL then record the new id and `conversation_started` false in the session file, replacing the old id. The old id SHALL NOT be reused.

#### Scenario: New id replaces the old one

- **GIVEN** a `claude` entry with id `abc` whose conversation is absent
- **WHEN** the session is recovered
- **THEN** the saved entry SHALL carry a new id different from `abc`, and that id SHALL appear after `--session-id` in its pane command

### Requirement: A resume whose CLI exits falls back to a fresh launch once

After recovery relaunches a worker with resume arguments, git-paw SHALL gate its pane for readiness. When the pane is a bare shell because the CLI exited, git-paw SHALL relaunch it once fresh with a new session id, record that id, and print a warning naming the branch. A second exit SHALL be reported as the readiness gate reports it, without failing the recovery.

#### Scenario: "No conversation found" is recovered from

- **GIVEN** a recovered `claude` worker launched with `--resume abc` whose CLI exits immediately
- **WHEN** recovery gates its pane
- **THEN** the pane SHALL be relaunched with `--session-id <new id>`, the session file SHALL record the new id, and stderr SHALL name the branch

### Requirement: A worker relaunched fresh receives its launch prompt

When recovery relaunches a worker fresh, git-paw SHALL submit its launch prompt once its pane is ready: the held `pending_boot_prompt` when one exists, otherwise a prompt rebuilt from the boot block, the task prose of the entry's spec when it still resolves, and the worktree rule. It SHALL NOT send the resume nudge to that worker.

#### Scenario: Fresh worker gets its boot prompt

- **GIVEN** a stopped session whose `feat/x` conversation never started and whose broker is enabled
- **WHEN** the session is recovered
- **THEN** the `feat/x` pane SHALL receive a prompt containing the boot block and the worktree rule, and no resume nudge

#### Scenario: A held prompt is submitted on recovery

- **GIVEN** an agent added to a paused session (prompt held) whose session was then stopped
- **WHEN** the session is recovered
- **THEN** the held prompt SHALL be submitted to that agent's pane and cleared from the session file

### Requirement: A resumed worker receives a resume nudge

When recovery relaunches a worker with resume arguments and its pane passes the readiness gate, git-paw SHALL submit one resume message: the bundled text by default, or `[workers] resume_message` when set. `[workers] resume_nudge = false` SHALL turn the message off. A paused restart and a reattach to an active session SHALL send no resume message.

#### Scenario: Attended recovery nudges resumed workers

- **GIVEN** a stopped session with two resumable workers and default config
- **WHEN** the user runs `git paw start`
- **THEN** each worker pane SHALL receive the resume message exactly once before the terminal attaches

#### Scenario: Unattended recovery nudges resumed workers

- **GIVEN** a stopped coordinated session with a resumable worker
- **WHEN** the user runs `git paw start --unattended`
- **THEN** the worker pane SHALL receive the resume message before the drive loop's first tick

#### Scenario: Custom message and opt-out

- **GIVEN** `[workers] resume_message = "carry on"`
- **WHEN** a session is recovered
- **THEN** resumed panes SHALL receive `carry on`
- **AND** with `[workers] resume_nudge = false` they SHALL receive nothing

#### Scenario: Paused restart sends no nudge

- **GIVEN** a paused session with live worker panes
- **WHEN** the user runs `git paw start`
- **THEN** no resume message SHALL be typed into any worker pane

### Requirement: Recovery messages reach only a ready pane

Recovery SHALL submit a resume message or launch prompt only to a pane resolved live as owned by the worker's worktree and passing the readiness gate. A pane that cannot be resolved, or stays blocked, SHALL get nothing and SHALL be reported in one warning; the recovery SHALL NOT fail. A launch prompt that was not delivered SHALL stay held in the session file.

#### Scenario: A blocked pane is warned about, not typed into

- **GIVEN** a recovered worker whose pane stays at a CLI dialog the gate cannot answer
- **WHEN** recovery tries to deliver its message
- **THEN** nothing SHALL be typed into that pane, stderr SHALL name its branch, and `git paw start` SHALL still succeed
