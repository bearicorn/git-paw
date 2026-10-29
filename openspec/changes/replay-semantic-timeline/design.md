## Context

Today `commands/replay.rs::cmd_replay` resolves a session directory (`replay::resolve_session`: the
newest by mtime), finds `<branch>.log` (`replay::find_log`), and either strips ANSI codes
(`replay_stripped`) or pipes the bytes through `less -R` (`replay_colored_with_path`). Nothing it
prints is structured.

After `session-record-capture`, each run has a record under `.git-paw/logs/<session>-<started>/`
holding three things:
- `manifest.json`: agents, branches, CLIs, `cli_session_id`, start and end commits, and the attach
  and detach times.
- `events.jsonl`: broker message-log entries in the `GET /log` shape.
- per-agent terminal captures.

The dashboard already has a one-line summary per broker message
(`dashboard/broker_log.rs::derive_summary`, exhaustive over `BrokerMessage`), a type label
(`type_short`) and a heartbeat-run collapse (`collapse_status_runs` / `heartbeat_key`).

The roadmap leaves the core decision open: is replay an **animation** (screen recording) or a
**transcript** (audit log)? It recommends a layered answer: the semantic timeline by default, with
`--cast` / `--raw` for animation.

## Goals / Non-Goals

**Goals:**
- `git paw replay` answers "what did each agent do, and when" from structured data, with no
  escape-code problem by construction.
- A pure, fixture-testable timeline core. I/O stays in adapters.
- Per-CLI transcript support through a Strategy with a Null Object, never hard-wiring one CLI.
- A stable `--json` form for orchestrators and scripts.

**Non-Goals:**
- Animated playback and timed capture (`replay-terminal-cast`).
- Live tailing (`--follow`). Live viewing belongs to the dashboard / `GET /log?since=` and the v0.18
  dashboard-survival question.
- Rendering tool *results* or full assistant messages.
- Copying or retaining CLI transcripts.

## Decisions

### D1: Layered replay, with the timeline as the default
Take the roadmap's recommendation. The default is the timeline. `--raw` (here) and `--cast`
(`replay-terminal-cast`) are opt-in views of the terminal capture.
*Alternatives:*
- (a) Animation only. A faithful screen recording is not greppable and is dominated by
  spinner and thinking-counter noise, and it needs a player that may be absent.
- (b) The CLI transcript alone. It is per-CLI (only Claude-format today), and it misses
  commits, coordination and approvals.
- (c) Animation as the default. It inverts the value the dogfood found.
The code does not make the layered answer unworkable: every source exists or is added by
`session-record-capture`.

### D2: Functional core, imperative shell
- **core** (`src/replay/timeline.rs`, pure): `build(inputs: TimelineInputs, filter: AgentFilter)
  -> Timeline`. `TimelineInputs` holds the mapped manifest, the parsed events, the commits per
  agent, the transcript turns per agent and the pre-collected notes. `build` merges, orders, collapses
  and sanitizes. It does no I/O and takes no clock, so it is tested with fixtures alone.
- **app** (`git_paw::commands::replay`, per v0.18 `cmd-library-surface`): selects the record,
  reads the manifest and events through the record store, reads commits through the
  version-control port, resolves and reads transcripts through the transcript sources, and calls
  `build`. It returns `ReplayOutcome::{Timeline, Records, Raw(path)}`.
- **entry** (binary dispatch): renders text or JSON, prints notes on stderr in text mode, and
  streams `Raw` with `std::io::copy`. The text renderer takes a UTC offset parameter (production:
  local, tests: fixed) so output is deterministic.
- The JSON document is a wire type (`TimelineDoc`, `schema_version: 1` per v0.21's
  `cli-machine-output`) mapped from `Timeline`, so the internal model can change without breaking
  consumers.

### D3: One broker-message summary, shared
Move `type_short`, `derive_summary` and the heartbeat key from `dashboard/broker_log.rs` to
`src/broker/summary.rs`. The dashboard re-exports them, so its behaviour is byte-identical. The
timeline's collapse is its own pure function, because its run rule ("no other entry *for that agent*
between") differs from the dashboard's global adjacency. It reuses the same key.
*Alternatives:* import from `dashboard::` (rejected: replay would depend on a view module that the
v0.18 dashboard-survival question may delete); `BrokerMessage`'s `Display` (rejected: lossy, and it
embeds `[agent_id]`). AGENTS.md §7's ripple list is updated to the new file. No variant is added.

### D4: Commit source
`git -C <repo> log --reverse --format=%H%x1f%h%x1f%aI%x1f%s <from>..<to>`, run through
`CommandRunner`, where `from = start_commit` and `to = end_commit`, else the branch name. The
`%x1f` separators survive any subject text, and the parser splits each line on the first three.
Author date is used because orchestrator rebases rewrite committer dates. If `to` (or `from`) does
not resolve, the agent gets a note and no commit entries. This one read keeps commit history
available after `finish`/`purge` without snapshotting anything.

### D5: Transcript sources — Strategy + Null Object, read in place
`trait TranscriptSource { fn locate(&self, agent: &RecordedAgent, env: &TranscriptEnv) -> Located; }`
is the I/O adapter. Each format has a pure `parse(reader) -> ParsedTranscript { turns, skipped }`.
Resolution comes from the agent's resolved CLI profile (v0.19 per-CLI provider). Until that lands,
or for a profile with no transcript facet, resolution falls back by CLI name (`claude` →
Claude-format), mirroring `config::built_in_session_id_args`. Everything else resolves to
`NoTranscript`.

Claude-format facts. Not encoded in the code any more: `auto_approve.rs::ProtectedPaths`, which
knew the `~/.claude` / `CLAUDE_CONFIG_DIR` / `settings_path`-parent config dirs and their
`projects/*/` subtree, was deleted by v0.17.0's `classifier-retirement` (`settings_path` stays
parse-only), so the Claude-format source owns these facts. **To verify** against the shipped Claude Code version before implementing:
1. A session's transcript is `<config-dir>/projects/<per-project dir>/<session-uuid>.jsonl`.
   Searching `projects/*/<uuid>.jsonl` avoids depending on how the project directory name is
   encoded.
2. Each line is a JSON object. User and assistant records carry `type` (`user` / `assistant`), an
   ISO `timestamp` and `message.content`, which is a string or an array of blocks (`text`,
   `tool_use` with `name` and `input`, `tool_result`).
3. The CLI may delete old transcripts on its own retention schedule.

The parser keys off only these fields, ignores everything else, counts unparseable lines, and is
tested against a fixture captured from a real session (task 4.1). The tool call's "primary target
argument" is the first present of `command`, `file_path`, `path`, `pattern` or `url`. Otherwise
only the tool name is shown.

Security: a `cli_session_id` must match the canonical UUID shape (8-4-4-4-12 hex) before it is
used in a path, because a tampered manifest is otherwise a path-traversal vector. The search
lists only directly under each candidate's `projects/` (no recursion, no symlink following).

### D6: Sanitizing untrusted text
`strip_ansi` becomes `sanitize_terminal_text`: it removes CSI and OSC sequences as today, then
replaces the remaining C0/DEL/C1 controls with spaces. Every summary built from agent-controlled
text passes through it in the core, so text and JSON carry identical summaries. `--raw` is the
deliberate exception: it is unsanitized, the help text says so, and it replaces `--color` as the
named escape-sequence exception in v0.21's `core-terminal-output`.

### D7: CLI surface
`<agent>` is optional, and with no argument `replay` shows the whole session. `--json` is added and
also applies to `--list`. `--raw` requires an agent. `--color` is removed outright: this is
pre-1.0, it follows the `stop-without-confirmation` precedent, and `--raw | less -R` is an exact
migration. Record selection (`--session` = record name, else the latest record of that session
name) and agent matching (branch, agent id, capture stem) are pure functions over the record
enumeration.

## Risks / Trade-offs

- [Claude Code changes its transcript schema] → the parser reads few fields and ignores unknown
  ones. Breakage degrades to a note, not a failure, and the fixture test pins the shape we support.
- [Transcript text (prompts, assistant messages) can contain secrets] → tool results are never
  rendered, and the privacy section of the session-logging guide (from `session-record-capture`)
  covers replay output too. Nothing is written to disk.
- [Timestamp precision differs across sources (broker: seconds; git: seconds; transcripts:
  milliseconds)] → the deterministic tie-break order keeps output stable. Sub-second interleaving
  across sources is approximate.
- [Removing `--color` breaks scripts] → the breakage is pre-1.0 and documented, with an exact
  migration. See Open Questions for the softer option.
- [Very long timelines] → output is line-oriented, so users page or grep it. No pager is forced.

## Migration Plan

Old records (no manifest) are recognized and point the user at `--raw`. New records need
`session-record-capture`. Rollback by reverting both changes; the records are ignored by older
binaries.

## Open Questions

- **`--color` deprecation cycle.** Remove it now (recommended), or keep it hidden for one release
  as an alias of `--raw` piped through `less -R`, with a warning?
- **More sources.** Drive-loop applied effects (v0.18 "effects as data") would make the audit log
  complete. They are deferred until v0.18 settles where those are written. If v0.18 publishes them
  as broker messages, they appear here with no change. The manual-approvals JSONL needs no extra
  source: since v0.17.0 the drive loop writes it for each live permission prompt it escalates, and
  each of those escalations is also an `agent.question` in the event record.
- **Custom Claude variants before v0.19.** With the name fallback, a `[clis.claude-oss]` agent gets
  no transcript until v0.19's resolved CLI identity (GP-29) lands. Accept this, or add a
  `[clis.<name>].transcript_format` override now? Recommendation: accept, and avoid adding config
  surface the provider layer will supersede.
