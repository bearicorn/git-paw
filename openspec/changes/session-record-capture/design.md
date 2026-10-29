## Context

The current capture code:

- `src/logging.rs` derives `.git-paw/logs/<session>/<sanitized-branch>.log`, creates the directory
  (`ensure_log_dir`, default permissions), and lists sessions and logs.
- `commands/helpers.rs::attach_session_logging` queues `TmuxSession::pipe_pane` (`cat >> <path>`) for
  each coding-agent pane. Its callers are `commands/start.rs` (bare and spec-driven launches) and
  `commands/supervisor.rs`. `commands/add.rs::cmd_add` and `commands/recover.rs::recover_session`
  never call it.
- The broker's `delivery::flush_loop` writes `BrokerMessage`'s `Display` text every ~5 s and at
  stop. It writes to `BrokerState::log_path`, which `main.rs::cmd_dashboard` (the `__broker` host
  after v0.18) sets to `session_state_dir()/broker.log`: one file for the whole machine.
  `cmd_purge` deletes it through `Session::broker_log_path`.
- `WorktreeEntry.cli_session_id` (from `worker-session-id-capture`) is the key to the agent CLI's own
  transcript, but it is persisted only in the global session file that `purge` deletes.
- `Session.created_at` is preserved across recovery (`recover_session` clones the saved session).

Constraints: no new crates; Unix permissions only (macOS, Linux, WSL); additive serialized
formats; behaviour with logging disabled must be byte-identical to today.

## Goals / Non-Goals

**Goals:**
- One self-contained, per-run session record that survives `purge` and holds everything the
  timeline replay needs (`replay-semantic-timeline`).
- Capture on every coding-agent surface path.
- Owner-only files, symlink refusal, opt-in retention.

**Non-Goals:**
- Changing the capture *format*. Each agent's file stays `cat >>` raw bytes in `<branch>.log`. A
  timed `.cast` capture is `replay-terminal-cast`.
- Changing, retiring or relocating `broker.log`. v0.21's wire-DTO split owns its format.
- Copying CLI transcripts into the record. They are read in place at replay time.
- Capturing the dashboard or the broker host.

## Decisions

### D1: The record's identity is derived, not stored
`record_dir_name(session_name, created_at) = "<session_name>-<YYYYMMDDTHHMMSSZ>"` is a pure function
in the logging core. Every writer (the launcher, `add`, `recover`, the broker host, `remove`/`purge`)
recomputes it from the saved session, so `Session` gains no field. Recovery clones `created_at`, so it
lands in the same record; a fresh launch gets a new `created_at` and a new record.
*Alternatives:* a `record_dir` field on `Session` (rejected: extra persisted state, and every reader
needs a back-compat path anyway); a random run id (needs persistence for the same reason); keeping
the per-session-name directory (rejected: runs interleave and retention becomes meaningless).
A session launched before this change (or with logging off at launch) has no record yet. Its
recovery creates the record and a manifest that starts at recovery time, with `start_commit` =
the worktree's current `HEAD`.

### D2: The manifest is a wire type, separate from `Session`
`manifest.json` has its own serde type with `version: 1`, mapped from the domain `Session` /
`WorktreeEntry` plus the commits read through the version-control port. It is never the global
`Session` struct, which is an internal model (the v0.21 DTO-split item). Only the launcher-side
commands write it: start, add, remove, purge, and recover when the manifest is missing. Writes go
to a temporary file in the record and are then renamed. The broker host and the drive loop never
write it. `start_commit` is `git rev-parse HEAD` in the new worktree, read through `CommandRunner`.
`end_commit` is `git rev-parse <branch>` before any branch deletion in `detach_worktree`.

### D3: The event record is a second sink of the existing flush loop
`BrokerState` gains an optional event-record path (builder method, like `log_path`). `flush_entries`
appends one JSON line per entry to it, in the same pass and from the same snapshot as `broker.log`,
so the two files never disagree about which entries were flushed. The line type is the
`GET /log` entry shape (`seq`, `timestamp_unix_secs`, `message`). The broker host resolves the path
with D1's function from the saved session and the loaded `[logging]` config. That needs no new IPC
and no new flag.
*Alternatives:* a separate subscriber polling `GET /log?since=` (rejected: another process to
supervise, and it adds a crash window); repointing `broker.log` (rejected: its format is a v0.21
contract decision, and other tooling may read it); persisting the drive loop's effects instead
(not available until v0.18's decision extraction lands, see Open Questions).
Sequence numbers restart when the host restarts (pause/resume). Consumers order by append order and
timestamp, never by `seq` alone.

### D4: One attach helper, called on every surface-creating path
`attach_session_logging` becomes "attach capture for these agents in this record". It is called
from the fresh-launch paths (unchanged), from `cmd_add` for the new surface, and from
`recover_session` for every recreated agent surface. It goes through the terminal backend's
output-stream operation (`stream_to` in `terminal-backend-seam`). The tmux adapter keeps
emitting `pipe-pane -o -t <target> "cat >> <quoted path>"`, so the existing quoting contract
(`safe-process-invocation`) is untouched. `cat >>` appending is what makes recovery append rather
than truncate.

### D5: Owner-only creation, symlink refusal, and residual risk
Directories are created with `DirBuilderExt::mode(0o700)`; files with `OpenOptionsExt::mode(0o600)`.
`cat >>` would otherwise create a capture file with the umask, so the launcher pre-creates each
capture file (0600) before attaching the stream; the shell then appends to an existing file. Before
creating or opening any record path, `symlink_metadata` refuses a symlink. No `libc` dependency is
available for `O_NOFOLLOW`, so a check-then-open window remains. It is mitigated because the record
lives under the main checkout's `.git-paw/`, outside every agent worktree: an agent write there is
denied by the documented sandbox, or is left to the agent CLI's own permission system, whose prompt
git-paw escalates to a human (git-paw has made no command-safety decision since v0.17.0's
`classifier-retirement` removed the protected-path set). `replay-terminal-cast` also moves the open
into git-paw's own recorder process.

### D6: Retention is a pure selection plus a guarded delete
`select_records_to_prune(records, keep, current) -> Vec<RecordName>` is pure and unit-tested over
fixtures. The shell enumerates `.git-paw/logs/` with `symlink_metadata`, considers only real
directories, and deletes with `std::fs::remove_dir_all`, which does not follow symlinks inside the
tree. Pruning runs only on a fresh launch, after the new record exists.

### D7: Configuration
`LoggingConfig` gains `capture_terminal: Option<bool>` and `retain_sessions: Option<NonZeroU32>`
(the zero check is the type's deserialization), both with `#[serde(default, skip_serializing_if)]`
so existing configs round-trip unchanged. Accessors resolve the defaults. The spec's `log_dir` is
reconciled to the shipped code: it was never a field.

## Risks / Trade-offs

- [Two persisted copies of broker history, `broker.log` and `events.jsonl`] → accepted for this
  cycle. `broker.log` is global and lossy; recommend that v1.0.0 decide whether to retire it (see
  Open Questions).
- [`events.jsonl` lags the live session by up to one flush interval (~5 s)] → acceptable for an
  after-the-fact record. Live viewing stays with the dashboard and `GET /log?since=`.
- [Manifest writers race (an `add` concurrent with a `remove`)] → atomic rename prevents torn
  files, and lost updates are possible only under concurrent launcher commands. v0.21's state-file
  locking covers the class.
- [Retention deletes user data] → off by default, only on fresh launch, only real record
  directories, never the active record, and the count is reported on stderr.
- [Existing `tests/session_logging_capture_integration.rs` paths change] → intentional, specified
  by the MODIFIED requirement.

## Migration Plan

Additive. Legacy `.git-paw/logs/<session>/` directories are listed as legacy records and never
rewritten. With logging disabled nothing changes. Rollback by reverting the change. Per-run
directories written in the meantime are ignored by older binaries, except `replay --list`, which
shows them as extra sessions.

## Open Questions

- **Retire `broker.log` at v1.0.0?** It is one machine-wide file in lossy `Display` form, and any
  purge deletes it. Recommendation: v0.21's freeze audit should mark it deprecated in favour of the
  per-session event record rather than freeze it.
- **Drive-loop effects as a source.** v0.18's decision extraction ("effects as data") could also
  persist applied effects (nudges, buffered-input submits). If v0.18 publishes them as broker messages they
  arrive in `events.jsonl` for free. If it keeps them in-process, a follow-up adds an `effects.jsonl`
  sink. Not committed here.
- **Manual-approvals JSONL** (`.git-paw/sessions/<session>.manual-approvals.jsonl`) is keyed by
  session name across runs. DS-4's premise is settled: since v0.17.0 (`classifier-retirement`) it is
  written only by the `--unattended` drive loop, one line per live permission prompt it escalates;
  the dashboard poll loop is deleted. Each of those escalations also reaches the broker as an
  `agent.question`, so it is already in `events.jsonl`. Recommendation: keep the JSONL out of the
  record. Not committed here.
