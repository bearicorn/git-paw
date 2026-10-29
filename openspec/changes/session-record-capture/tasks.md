## 1. Seam: record naming and enumeration (pure core first)

- [ ] 1.1 Split `src/logging.rs` into `src/logging/` (`mod.rs` keeps the existing public API unchanged; no behaviour change), `just check` green
- [ ] 1.2 Add pure `record_dir_name(session_name, created_at)` and `parse_record_name(&str)`; unit tests for the `YYYYMMDDTHHMMSSZ` form, session names containing `-` and collision suffixes, and non-record names (→ legacy)
- [ ] 1.3 Replace `list_log_sessions` with a record enumeration returning name, session name, start time, legacy flag, newest first; tests for "Multiple records", "Legacy record recognized", "No records", "Logs directory does not exist"
- [ ] 1.4 Owner-only creation helpers (`0700` dirs, `0600` files) with a `symlink_metadata` refusal; tests for "Record files are owner-only" (unit level) and symlink refusal

## 2. Config

- [ ] 2.1 `LoggingConfig`: add `capture_terminal: Option<bool>` and `retain_sessions: Option<NonZeroU32>` with `#[serde(default, skip_serializing_if)]` and default-resolving accessors
- [ ] 2.2 Tests: "Logging section with all fields", "Logging section with only enabled", "Logging section defaults", "Zero retention rejected", "Legacy log_dir key is ignored"; existing round-trip test still passes
- [ ] 2.3 Extend the default-config `[logging]` comment block (`src/config/mod.rs`) with commented `capture_terminal` / `retain_sessions` examples (project-agnostic)

## 3. Record directory and manifest

- [ ] 3.1 Launch paths create the record via `record_dir_name` from the saved session (replacing `ensure_log_dir(repo_root, session_name)`); tests for "Record directory created on fresh launch", "Log directory already exists", "Logging disabled"
- [ ] 3.2 `src/git.rs`: `rev_parse(worktree, rev)` through `CommandRunner`; unit test with the fake runner
- [ ] 3.3 Manifest wire type (`version: 1`) + mapper from `Session`/`WorktreeEntry`; atomic temp-file + rename writer; tests for "Manifest with absent optional fields loads" and unknown-field tolerance
- [ ] 3.4 Write the manifest at fresh launch (`start_commit`, `attached_at`, `cli_session_id`); test "Manifest written at fresh launch"
- [ ] 3.5 `cmd_add` appends an entry; test "Hot-attach appends an entry"
- [ ] 3.6 `detach_worktree` records `detached_at` + `end_commit` before any branch deletion; `cmd_purge` sets `ended_at`; tests "Remove records the end of an agent", "Purge ends the record"
- [ ] 3.7 `recover_session` reuses the record and creates a manifest only when missing; tests "Recovery reuses the record", "Recovery does not duplicate entries", "Fresh launch after purge starts a new record"

## 4. Capture on every coding-agent surface

- [ ] 4.1 Generalize `attach_session_logging` to take the record and honour `capture_terminal`; pre-create each capture file `0600` and skip (with a stderr warning) a symlinked path
- [ ] 4.2 Call it from `cmd_add` (new surface) and `recover_session` (every recreated agent surface), routed through the terminal backend's output-stream operation
- [ ] 4.3 Real-tmux e2e in `tests/session_logging_capture_integration.rs`: update existing paths to the per-run record; add "Capture attached on hot-attach", "Capture attached on recovery" (content preserved and appended), "Non-agent surfaces are not captured", "Not attached when terminal capture is turned off"; keep "Capture attached on a bare fresh launch" and "Not attached when logging disabled"
- [ ] 4.4 e2e "Symlinked capture path is refused": recovery with a planted symlink leaves the target unchanged and captures the other agents

## 5. Broker event record

- [ ] 5.1 `BrokerState` builder for an optional event-record path; `flush_entries` appends JSON lines (`seq`, `timestamp_unix_secs`, `message`) from the same snapshot as `broker.log`, file `0600`
- [ ] 5.2 The broker host resolves the path from the saved session + `[logging]` (logging on → record path; off → none)
- [ ] 5.3 Tests: "Published messages are recorded in order" (lines round-trip into `BrokerMessage`), "Shutdown flushes pending entries", "A broker host restart appends", "No event record without logging", "No event record without the broker"; assert `broker.log` content is unchanged

## 6. Retention

- [ ] 6.1 Pure `select_records_to_prune(records, keep, current)`; fixture tests (current never selected, legacy ordered by mtime)
- [ ] 6.2 Guarded delete on fresh launch only (real directories, `remove_dir_all`), stderr count; tests "Oldest records are pruned", "Unset retention keeps everything", "Symlinks and files are never pruned", "Recovery never prunes"

## 7. Docs

- [ ] 7.1 `docs/src/user-guide/session-logging.md`: per-run record layout, manifest and event record, capture on add/recover (remove the "not logged" caveat), privacy section (secrets, owner-only, gitignored, never transmitted, `retain_sessions`, `capture_terminal`); doc test for "Privacy section present"
- [ ] 7.2 `docs/src/configuration/README.md` `[logging]` table: `enabled`, `capture_terminal`, `retain_sessions`
- [ ] 7.3 `docs/src/architecture.md`: logging module split and record layout
- [ ] 7.4 Add `Test:` pointers to the requirements touched by this change when archiving

## 8. Quality gates

- [ ] 8.1 `just check` (fmt + clippy + full suite) passes; verify with the real exit code, not piped output
- [ ] 8.2 `just deny` passes; no new dependency
- [ ] 8.3 `mdbook build docs/` passes
