## 1. Verify external facts first

- [ ] 1.1 Verify against the asciinema v2 format documentation: header fields, `"o"` and `"r"` event shapes, UTF-8 data requirement; record the result in design D3
- [ ] 1.2 Verify `asciinema play <file>` plays v2 recordings with the installed 2.x / 3.x CLIs; record the result in design D5
- [ ] 1.3 Verify whether tmux `pipe-pane` expands `#{…}` formats in its shell command (and so whether a literal `#` must be doubled); pick D2's format-expansion or post-execute-query path accordingly

## 2. Pure encoder (core)

- [ ] 2.1 `src/logging/cast.rs`: header encode/parse, output-event encode, resize-event encode, UTF-8 carry-over state, output-data decode for `--raw`
- [ ] 2.2 Unit tests: "Header written first", "Escape sequences preserved", "Split UTF-8 character emitted whole", invalid bytes → U+FFFD, "Elapsed time is non-decreasing", continuation from a parsed header incl. "Size change recorded on reopen" and corrupt-header detection

## 3. Recorder subcommand (shell)

- [ ] 3.1 Hidden `Record` command in `src/cli.rs` (`hide = true`) and `src/commands/record.rs`: symlink refusal, `0600` append open, monotonic clock offset from the header timestamp, per-event flush, exit on EOF
- [ ] 3.2 `assert_cmd` tests: "Recorder produces a playable recording", "Recorder is hidden", "Recorder refuses a symlink", "Append after recovery", "Corrupt existing file left alone"

## 4. Capture sink

- [ ] 4.1 `TmuxSession::pipe_pane` emits the recorder command (quoted `current_exe`, quoted and `#`-escaped path, size per task 1.3); capture paths in `commands/helpers.rs` switch to `.cast` (the recorder now creates the file, so the launcher's `session-record-capture` pre-create step is dropped); the backend's output-stream operation passes the recorder sink; the fake backend records it
- [ ] 4.2 Update `src/tmux/tests.rs` pipe-pane tests: "pipe-pane queued in builder", "pipe-pane in dry-run output", "pipe-pane executed after pane creation"; quoting tests "Logging to a path with a space", "Binary path with a space", "Path containing a format character", "Plain path is behavior-equivalent"
- [ ] 4.3 `logging` listing: `.cast` + `.log` with a timed flag; tests "Record with timed captures", "Legacy record", "Session directory empty", "Session directory does not exist", "Simple log filename", "Sanitized cast filename", and the three path-derivation scenarios
- [ ] 4.4 Real-tmux e2e: "Live capture under tmux" (marker appears in an output event); recovery keeps one header and appends

## 5. Playback

- [ ] 5.1 `CastPlayer` port + `asciinema` adapter (`which`, argv, inherited stdio, exit status) + fake
- [ ] 5.2 `cli.rs`: `--cast` (requires agent; conflicts with `--raw`/`--json`/`--list`); parse tests "Replay with --cast", "--cast without an agent is rejected", "--cast with --raw is rejected", "Help describes --cast"
- [ ] 5.3 Replay command: `--cast` dispatch and `--raw` from a cast; tests "Played through asciinema" and "Player failure propagates" (fake player and a stub `asciinema` script on a temp `PATH`), "No player installed", "Legacy capture has no timing", "Raw from a cast"
- [ ] 5.4 `core-terminal-output`: add `replay <agent> --cast` to the standing escape test's exceptions; test "Cast playback is given the recording unmodified" (stub `asciinema` on a temp `PATH`, `NO_COLOR=1`)

## 6. Docs

- [ ] 6.1 `docs/src/user-guide/session-logging.md`: animated replay with `--cast`, installing asciinema, the terminal-size caveat, the playback trust note, legacy `.log` captures
- [ ] 6.2 `docs/src/cli-reference.md` replay block (`--cast`); `docs/src/architecture.md` (recorder, `logging::cast`); `--help` / `long_about` example

## 7. Quality gates

- [ ] 7.1 `just check` passes, verified by the real exit code
- [ ] 7.2 `just deny` passes; no new dependency
- [ ] 7.3 `mdbook build docs/` passes
