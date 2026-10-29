## Why

`git paw replay` cannot reconstruct a session partly because git-paw records the wrong things, in
the wrong place, for the wrong lifetime:

- **Runs interleave.** The log directory is `.git-paw/logs/<session-name>/`, and the session name
  is stable per repository (`paw-<project>`). Every run of a repository appends to the same
  `<branch>.log` files (`cat >>`), so runs blur together and a record never ends.
- **Broker history is not a session artifact.** The broker flushes its message log to a single
  machine-wide `~/.local/share/git-paw/sessions/broker.log` shared by every repository and
  session, in `BrokerMessage`'s lossy `Display` form (a status loses its message and activity),
  and any `purge` deletes that shared file.
- **The keys to an agent's history are thrown away.** The facts needed later to find an agent's
  commits and its CLI's own transcript (the worktree's starting commit, the recorded
  `cli_session_id`) live only in the global session file, which `purge` deletes.
- **Capture is still missing on two paths.** The roadmap says logging is wired only on the
  `--from-specs` launch. That is stale: `attach_session_logging` (`commands/helpers.rs`, fix
  02649dc5) now covers bare, spec-driven and supervisor launches. Two surface-creating paths
  still capture nothing: hot-attach (`commands/add.rs::cmd_add`) and recovery
  (`commands/recover.rs::recover_session`). The user guide already documents this gap.
- **Captures are unprotected and unbounded.** Files are created with the default umask
  (typically world-readable), and nothing ever removes old logs. Terminal captures routinely
  contain secrets.

This change fixes what is captured and when. The replay rework that consumes it is
`replay-semantic-timeline`.

## What Changes

- **Session record per run.** Each fresh launch writes to `.git-paw/logs/<session>-<started>/`,
  where `<started>` is the session's `created_at` in UTC (`YYYYMMDDTHHMMSSZ`). Recovery and
  hot-attach reuse the record; a fresh launch after `purge` starts a new one. Legacy
  `.git-paw/logs/<session>/` directories stay listable.
- **Record manifest.** `manifest.json` (a versioned wire type) records the session, its start
  and end, and per agent: id, branch, CLI, `cli_session_id`, worktree, start commit,
  attach/detach times and end commit. It is written at launch and updated by `add`, `remove`
  and `purge`.
- **Broker event record.** When logging and the broker are both enabled, the broker host also
  appends every message-log entry to `events.jsonl` in the record, one JSON object per line in
  the `GET /log` entry shape, on the existing flush cadence and at shutdown. `broker.log` is
  unchanged.
- **Capture on every coding-agent surface.** Terminal capture is attached on fresh launch in
  every mode, on hot-attach and on recovery. It goes through the terminal backend's
  output-stream operation, and non-agent surfaces are not captured.
- **Privacy defaults.** Record directories are owner-only (0700) and record files owner-only
  (0600). git-paw refuses to write through a symlinked record path. `[logging] retain_sessions =
  N` prunes the oldest records at fresh launch (unset keeps everything).
  `[logging] capture_terminal = false` keeps the timeline sources but skips the terminal capture.
- **Config.** `[logging]` gains `capture_terminal` (default `true`) and `retain_sessions`
  (optional). The spec's `log_dir` field, which the code never implemented, is removed from the
  requirement.
- **Disabled means unchanged.** With `[logging]` absent or `enabled = false`, nothing new is
  written anywhere.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `session-logging`: the session log directory becomes a per-run session record with owner-only
  permissions. Capture is attached on every coding-agent surface path. The library enumeration
  covers per-run and legacy records. New requirements: record manifest, broker event record,
  owner-only files and symlink refusal, retention, and the privacy documentation.
- `core-configuration`: the `[logging]` section gains `capture_terminal` and `retain_sessions`,
  and drops the never-implemented `log_dir`.

## Impact

- **Code:** `src/logging.rs` becomes `src/logging/` (record naming, enumeration, manifest wire
  type, retention, owner-only file creation). Callers: `commands/helpers.rs::attach_session_logging`;
  `commands/{start,add,recover}.rs` for the attach paths; `main.rs::detach_worktree` and
  `cmd_purge` (in the `cmd_*` library after v0.18) for the manifest end-of-agent and
  end-of-session updates. In the broker, `src/broker/delivery.rs::flush_loop`/`flush_entries`
  gains a second, structured sink, configured by the broker host. `src/config/mod.rs::LoggingConfig`
  and the default-config `[logging]` comment block change, and `src/git.rs` gains a
  `rev-parse` helper through `CommandRunner` for the start and end commits.
- **Persisted state:** `Session` and `WorktreeEntry` are unchanged, because the record directory
  is derived from `session_name` and `created_at`. There are two new file formats under the
  record: `manifest.json` (`version: 1`) and `events.jsonl`.
- **Enum-variant ripple:** none. `BrokerMessage` is serialized with its existing wire form, and
  no variant is added.
- **Tests:** `tests/session_logging_capture_integration.rs` changes, because the capture paths now
  sit under the per-run directory. New tests cover add/recover capture, the manifest lifecycle,
  the event record, permissions, symlink refusal and retention.
- **Docs:** `user-guide/session-logging.md` (record layout, privacy, retention),
  `configuration/README.md` (`[logging]`), `architecture.md` (logging module, record layout) and
  the default config comments emitted by `git paw init`.
- **Security / safety:** this change reduces exposure: owner-only files, symlink refusal, and
  opt-in retention. Retention is the only deletion it adds. It deletes only real directories
  directly under `.git-paw/logs/` whose names parse as session records, and never the active
  record.
- **Cross-version dependencies:**
  - v0.18 `terminal-backend-seam`: capture goes through the backend's output-stream operation,
    `stream_to`.
  - v0.18 `supervisor-pane-removal`: there is no supervisor pane to capture. Whether
    `git paw add` survives on a pane-less layout (GP-49) decides whether the hot-attach path
    exists.
  - v0.18 `broker-host-separation`: the `__broker` host owns the flush loop that writes
    `events.jsonl`.
  - v0.18 `cmd-library-surface`: the manifest updates live in the library command functions.
  - v0.21 wire-DTO split: `manifest.json` and the `events.jsonl` line type adopt the shared
    `broker::wire` log-entry type. If that split slips, the line type is defined here.
  - v0.21 state-file locking: covers manifest read-modify-write.
  - No v0.17.0 or v0.18.0 change modified `session-logging` or the `[logging]` requirement.
