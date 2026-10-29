## MODIFIED Requirements

### Requirement: List available log sessions (replay --list output)

The system SHALL display the available session records, newest first, when `--list` is passed:
each record's name and start time, its agents (agent id, branch, CLI), and which sources it holds
(manifest, broker events, terminal capture). A legacy record SHALL be marked as legacy and list the
branches derived from its capture filenames. With `--json`, the same listing SHALL be printed as one
JSON document carrying `schema_version` (`1`), under the `cli-machine-output` contract.

#### Scenario: Records available
- **WHEN** `git paw replay --list` is run and two session records exist
- **THEN** stdout SHALL list both, newest first, each with its start time, its agents and its available sources

#### Scenario: Legacy record marked
- **WHEN** `git paw replay --list` is run and `.git-paw/logs/paw-other/` holds only `feat--a.log`
- **THEN** `paw-other` SHALL be listed as legacy with branch `feat/a`

#### Scenario: Listing as JSON
- **WHEN** `git paw replay --list --json` is run and two session records exist
- **THEN** stdout SHALL be one JSON document with `schema_version` `1` listing both records with the same fields as the text listing

#### Scenario: No sessions available
- **WHEN** `git paw replay --list` is run and no session records exist
- **THEN** stdout SHALL display a message indicating no session records are available

### Requirement: Default to most recent session

When `--session` is not specified, the system SHALL replay the most recent session record. A
`--session <value>` SHALL select the record whose directory name equals the value. When no record
has that name, it SHALL select the most recent record of the session whose name equals the value.

#### Scenario: Most recent record selected
- **WHEN** `git paw replay` is run without `--session` and several records exist
- **THEN** the timeline SHALL be built from the record with the most recent start time

#### Scenario: Explicit record
- **WHEN** `git paw replay --session paw-myproject-20261017T080000Z` is run
- **THEN** the timeline SHALL be built from that record

#### Scenario: Session name selects its latest record
- **WHEN** `git paw replay --session paw-myproject` is run and `paw-myproject` has records from two runs
- **THEN** the timeline SHALL be built from the newer of the two

#### Scenario: Specified session does not exist
- **WHEN** `--session nonexistent` is passed and no record or session matches it
- **THEN** the system SHALL return an error mentioning `nonexistent` and suggesting `--list`

### Requirement: Fuzzy branch matching

The system SHALL match the `<agent>` argument against each recorded agent's branch name, its agent
id, and its sanitized capture-file stem. A partial name SHALL NOT match.

#### Scenario: Match by branch name
- **WHEN** `git paw replay feat/add-auth` is run and the record lists branch `feat/add-auth`
- **THEN** the timeline SHALL be filtered to that agent

#### Scenario: Match by agent id
- **WHEN** `git paw replay feat-add-auth` is run
- **THEN** the timeline SHALL be filtered to the agent whose agent id is `feat-add-auth`

#### Scenario: Match by sanitized name
- **WHEN** `git paw replay feat--add-auth` is run
- **THEN** the timeline SHALL be filtered to the agent whose capture-file stem is `feat--add-auth`

#### Scenario: No matching agent
- **WHEN** `git paw replay nonexistent` is run and no agent matches
- **THEN** the system SHALL return an error listing the record's agents

### Requirement: ANSI stripping correctness

The terminal-text sanitizer SHALL remove all CSI sequences (starting with `\x1b[`) and OSC
sequences (`\x1b]...\x07` and `\x1b]...\x1b\\`) and SHALL replace every remaining control character
(C0, DEL and C1, including a lone ESC, carriage return, line feed and tab) with a space, preserving
all other content. Every piece of untrusted text the timeline prints — broker message text, commit
subjects, transcript text — SHALL pass through this sanitizer before it is written in text output.

#### Scenario: Plain text passes through unchanged
- **WHEN** content has no escape sequences or control characters
- **THEN** the sanitized output SHALL be identical to the input

#### Scenario: Multiple sequences in one line
- **WHEN** a line contains `\x1b[1m\x1b[31mBold Red\x1b[0m Normal`
- **THEN** the sanitized output SHALL be `Bold Red Normal`

#### Scenario: OSC sequences stripped
- **WHEN** the content contains `\x1b]0;window title\x07` or `\x1b]8;;https://example.com\x1b\\`
- **THEN** the sanitized output SHALL not contain those sequences

#### Scenario: Incomplete escape sequence at end of input
- **WHEN** content ends with `\x1b[` (incomplete CSI)
- **THEN** the incomplete sequence SHALL be removed without error

#### Scenario: Control characters neutralised
- **WHEN** content is `line one\nline\rtwo\x07`
- **THEN** the sanitized output SHALL contain no control character

#### Scenario: Hostile broker text cannot reach the terminal
- **GIVEN** a recorded `agent.status` whose message contains the clipboard-write sequence `\x1b]52;c;ZXZpbA==\x07`
- **WHEN** the timeline is printed as text
- **THEN** stdout SHALL contain no ESC byte

### Requirement: Handle missing or empty logs

The system SHALL handle missing and sparse records gracefully. A record whose manifest lists agents
but which holds no broker events, no commits and no transcripts SHALL still produce a timeline of its
session and agent entries. When `.git-paw/logs/` does not exist, the system SHALL return an error
suggesting that logging may not be enabled.

#### Scenario: Sparse record
- **GIVEN** a record with a manifest listing two agents, no `events.jsonl`, no commits and no transcript source
- **WHEN** `git paw replay` is run
- **THEN** the timeline SHALL show the session start and both agents' attach entries
- **AND** the command SHALL succeed

#### Scenario: No logs directory
- **WHEN** `git paw replay` is run and `.git-paw/logs/` does not exist
- **THEN** the system SHALL return an error suggesting logging may not be enabled

## ADDED Requirements

### Requirement: Replay renders the session timeline by default

The system SHALL, for `git paw replay [<agent>]`, print the selected record's timeline to stdout. The
output SHALL start with a header line naming the record and its start time, followed by one line per
entry carrying the entry's local time (`HH:MM:SS`), its agent id (or `session` for session-level
entries), its kind, and its one-line summary. With `<agent>`, only that agent's entries and the
session-level entries SHALL be printed. Notes about unavailable sources SHALL be printed on stderr,
never on stdout.

#### Scenario: Whole-session timeline
- **GIVEN** a record for agents `feat/a` and `feat/b` holding broker events, commits and a transcript for `feat/a`
- **WHEN** `git paw replay` is run
- **THEN** stdout SHALL contain entries from every source for both agents in time order

#### Scenario: Timeline filtered to one agent
- **GIVEN** the same record
- **WHEN** `git paw replay feat/b` is run
- **THEN** stdout SHALL contain only `feat/b` entries and session-level entries

#### Scenario: Notes go to stderr
- **GIVEN** a record whose agent `feat/b` runs a CLI with no transcript source
- **WHEN** `git paw replay` is run
- **THEN** the note that `feat/b` has no transcript SHALL appear on stderr and not on stdout

### Requirement: Timeline sources

The system SHALL build the timeline from four sources:
- the manifest, which yields session started and ended entries and agent attached and detached
  entries;
- the broker event record, which yields one entry per broker message, with the message's type label
  as its kind and the shared broker message summary as its summary;
- each agent's commit history, which yields one entry per commit in `start_commit..end_commit`, or
  `start_commit..<branch>` when the agent has no `end_commit`, timed by author date and summarized as
  short SHA plus subject;
- each agent's CLI transcript, read through its transcript source.

When an agent's commit range cannot be resolved, the timeline SHALL carry a note and no commit
entries for that agent. A broker event line that does not parse SHALL be skipped and counted in a
note.

#### Scenario: Broker events become entries
- **GIVEN** an `events.jsonl` holding an `agent.status` and an `agent.artifact` for `feat/a`
- **WHEN** the timeline is built
- **THEN** it SHALL hold a `status` entry and an `artifact` entry for `feat/a` at the recorded timestamps

#### Scenario: Commits are bounded by the agent's start commit
- **GIVEN** agent `feat/a` with `start_commit` `S` and branch `feat/a` holding commits before `S` and two commits after it
- **WHEN** the timeline is built
- **THEN** it SHALL hold exactly the two commits after `S`, each summarized as short SHA plus subject

#### Scenario: Merged and deleted branch keeps its commits
- **GIVEN** agent `feat/a` whose manifest entry has `end_commit` `E` and whose branch has been merged and deleted
- **WHEN** the timeline is built
- **THEN** it SHALL hold the commits in `start_commit..E`

#### Scenario: Unresolvable commit range
- **GIVEN** agent `feat/a` with no `end_commit` whose branch no longer exists
- **WHEN** the timeline is built
- **THEN** it SHALL hold no commit entries for `feat/a`
- **AND** it SHALL carry a note that `feat/a`'s commit history is unavailable

#### Scenario: Malformed event line skipped
- **GIVEN** an `events.jsonl` with one valid line and one truncated line
- **WHEN** the timeline is built
- **THEN** it SHALL hold the valid line's entry and a note counting one skipped line

### Requirement: Timeline ordering and heartbeat collapse

The system SHALL order timeline entries by timestamp, breaking ties by source (session, then broker,
then commit, then transcript) and then by their order within the source. A status entry SHALL
collapse into the preceding entry when four conditions hold: the preceding entry is a status entry
for the same agent, both have the same status, activity and message, and no other entry for that
agent lies between them. The surviving entry SHALL carry the run length.

#### Scenario: Sources interleave by time
- **GIVEN** a commit at 10:00:05, a broker status at 10:00:01 and a transcript turn at 10:00:03 for the same agent
- **WHEN** the timeline is built
- **THEN** the entries SHALL appear in the order status, transcript turn, commit

#### Scenario: Ties keep source order
- **GIVEN** a broker entry and a commit entry with the same timestamp
- **WHEN** the timeline is built
- **THEN** the broker entry SHALL precede the commit entry

#### Scenario: Identical heartbeats collapse
- **GIVEN** five consecutive identical `working` status entries for `feat/a` with no other `feat/a` entry between them
- **WHEN** the timeline is built
- **THEN** it SHALL hold one `working` status entry with a run length of 5

#### Scenario: A different entry ends the run
- **GIVEN** two identical `working` statuses for `feat/a` separated by a `feat/a` commit
- **WHEN** the timeline is built
- **THEN** both status entries SHALL remain

### Requirement: CLI transcript source is resolved per CLI

The system SHALL resolve a transcript source for each agent from the agent's resolved CLI profile. A
CLI with no transcript source SHALL resolve to the no-transcript source, which yields no entries and
one note. The built-in Claude-format source SHALL attempt a lookup only when the agent's
`cli_session_id` is a canonical UUID. It SHALL search `projects/*/<cli_session_id>.jsonl` under, in
order, the parent directory of the CLI's configured `settings_path`, the directory named by
`CLAUDE_CONFIG_DIR`, and `~/.claude`. The source SHALL read the transcript in place and never copy it.
It SHALL yield one entry per user prompt (first line), per assistant text block (first line) and per
tool call (tool name plus its primary target argument). It SHALL NOT render tool results. Lines it
cannot parse SHALL be skipped and counted in a note.

#### Scenario: Transcript rendered
- **GIVEN** agent `feat/a` with CLI `claude`, a UUID `cli_session_id`, and a matching transcript fixture under `~/.claude/projects/<dir>/`
- **WHEN** the timeline is built
- **THEN** it SHALL hold the fixture's user prompt, assistant message and tool-call entries at their recorded times

#### Scenario: Tool results are not rendered
- **GIVEN** a transcript whose tool result contains the text `SECRET_TOKEN=abc`
- **WHEN** the timeline is printed as text or JSON
- **THEN** the output SHALL NOT contain `SECRET_TOKEN=abc`

#### Scenario: Non-UUID session id is never used as a path
- **GIVEN** a manifest whose `feat/a` entry has `cli_session_id` `../../etc/passwd`
- **WHEN** the timeline is built
- **THEN** no file outside the candidate `projects/` directories SHALL be opened
- **AND** the timeline SHALL carry a note that `feat/a` has no usable transcript id

#### Scenario: CLI without a transcript source
- **GIVEN** agent `feat/b` whose CLI resolves no transcript source
- **WHEN** the timeline is built
- **THEN** it SHALL hold no transcript entries for `feat/b` and one note naming the CLI

#### Scenario: Transcript no longer present
- **GIVEN** agent `feat/a` whose transcript file has been deleted by its CLI
- **WHEN** the timeline is built
- **THEN** the timeline SHALL still hold `feat/a`'s broker and commit entries
- **AND** it SHALL carry a note that the transcript was not found

#### Scenario: Unparseable transcript lines are counted
- **GIVEN** a transcript with one unparseable line among valid ones
- **WHEN** the timeline is built
- **THEN** the valid lines SHALL yield entries and a note SHALL count one skipped line

### Requirement: Timeline JSON output

The system SHALL, when `--json` is passed, print the timeline to stdout as exactly one JSON document
under the `cli-machine-output` contract, with `schema_version` (`1`), `record`, `session`,
`started_at`, `entries` and `notes`. Each entry SHALL carry
`at` (RFC 3339, UTC), `agent` (agent id, or `null` for session-level entries), `source`
(`session`, `broker`, `commit` or `transcript`), `kind`, `summary` (sanitized as for text output)
and `repeat`. Notes SHALL appear only in the document's `notes` and SHALL NOT also be printed on
stderr.

#### Scenario: JSON document parses
- **WHEN** `git paw replay --json` is run on a record with entries from every source
- **THEN** stdout SHALL parse as one JSON object with `schema_version` `1`, and its `entries` SHALL match the text timeline's entries in order and content

#### Scenario: Notes inside the document
- **GIVEN** a record with an agent that has no transcript source
- **WHEN** `git paw replay --json` is run
- **THEN** the note SHALL appear in `notes` and stderr SHALL be empty

### Requirement: Raw terminal output

The system SHALL, for `git paw replay <agent> --raw`, write the agent's captured terminal output from
the selected record to stdout exactly as captured (no sanitizing, no paging) and nothing else. When the record
holds no terminal capture for that agent, the system SHALL return an error that names the agent and
says terminal capture was off or missing.

#### Scenario: Raw bytes are unmodified
- **GIVEN** a capture file for `feat/a` containing `\x1b[31mred\x1b[0m`
- **WHEN** `git paw replay feat/a --raw` is run
- **THEN** stdout SHALL be byte-identical to the capture file

#### Scenario: Raw works on a legacy record
- **GIVEN** a legacy record `paw-other` holding `feat--a.log`
- **WHEN** `git paw replay feat/a --raw --session paw-other` is run
- **THEN** stdout SHALL be byte-identical to `feat--a.log`

#### Scenario: No capture for the agent
- **GIVEN** a record created with `capture_terminal = false`
- **WHEN** `git paw replay feat/a --raw` is run
- **THEN** the command SHALL fail with an error naming `feat/a`

### Requirement: Legacy session records

The system SHALL NOT build a timeline from a legacy record (one without a manifest). For such a
record, `git paw replay` without `--raw` SHALL fail with an error that names the record and suggests
`git paw replay <branch> --raw --session <record>` with one of its branches.

#### Scenario: Timeline requested on a legacy record
- **GIVEN** the most recent record is a legacy record `paw-other` holding `feat--a.log`
- **WHEN** `git paw replay` is run
- **THEN** the command SHALL fail with an error naming `paw-other` and suggesting `git paw replay feat/a --raw --session paw-other`

## REMOVED Requirements

### Requirement: Display stripped log output

**Reason**: Stripping ANSI codes from a capture of a full-screen agent TUI (alternate screen,
cursor-addressed redraws, spinners) produces unreadable repaint fragments. The peer-4-poker dogfood
confirmed it unusable. The semantic timeline replaces it as the default output.
**Migration**: Run `git paw replay [<agent>]` for the timeline. Run `git paw replay <agent> --raw`
for the captured bytes; pipe them through an external stripper if plain text is wanted.

### Requirement: Display colored log output

**Reason**: `less -R` renders colour but not cursor addressing, so the capture of a full-screen
TUI is not reconstructed. The `--color` flag is removed.
**Migration**: `git paw replay <agent> --raw | less -R` reproduces the old behaviour exactly. For a
faithful animated replay, use `--cast` (change `replay-terminal-cast`).
