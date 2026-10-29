## Why

The semantic timeline (`replay-semantic-timeline`) answers *what* each agent did. It cannot show
*what the screen looked like*: a first-run dialog that never cleared, a permission prompt the drive
loop misread, a TUI that rendered garbage. Those are exactly the failures git-paw's drive loop
chases. The terminal capture already holds the bytes (`pipe-pane` → `cat >> <branch>.log`), but it
has no timing. Without timing, nothing can re-run the escape sequences at speed, and stripping or
paging them (the old `replay`) cannot reconstruct a cursor-addressed full-screen TUI.

The standard answer is a timed recording played back in a real terminal. An asciinema v2 `.cast`
is a header plus `[elapsed, "o", data]` output events. `asciinema play` writes those events to the
viewer's terminal at the recorded pace, so the viewer's terminal does the emulation and git-paw
needs no VT emulator.

This is the roadmap's optional second layer. It may slip past v0.22.0 without affecting the
v1.0.0 freeze, because `--cast` is additive.

## What Changes

- **Timed capture.** Each coding agent's capture becomes an asciinema v2 recording
  (`<record>/<sanitized-branch>.cast`) instead of `cat >> <branch>.log`. A hidden
  `git paw __record` subcommand reads the surface's output stream and writes the header (the
  surface's size, the start timestamp) and one output event per read. It buffers a UTF-8 sequence
  that a read splits, creates the file owner-only, refuses a symlink, and flushes every event.
- **Recording continues across recovery.** On reopen, the recorder appends to the existing cast,
  timing new events from the original header. It emits a resize event when the surface size has
  changed.
- **Capture sink.** The terminal backend's output-stream operation delivers to the recorder. The
  tmux adapter emits `pipe-pane -o -t <pane> '<quoted exe> __record … <quoted path>'`, quoting both
  paths and escaping tmux's `#` format character.
- **`git paw replay <agent> --cast`** plays the recording through `asciinema play <file>` when
  `asciinema` is on `PATH`. Without it, the command fails and names the `.cast` file, which any
  asciinema v2 player can play. A record captured before this change (`.log`) has no timing:
  `--cast` refuses and points at `--raw`.
- **`--raw` on a `.cast`** writes the concatenated output-event data, which is the captured bytes.
- **No built-in player in this change.** A player that emulates a terminal needs a new crate
  (`vt100`), which is not on the approved list. That is a user decision, recorded in design.md
  with a zero-crate alternative.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `session-logging`: capture files become timed `.cast` recordings. The capture-path derivation,
  the raw-output contract, the per-record file listing and the branch-from-filename rule change.
  New requirements cover the recorder subcommand, recovery continuation, `--cast` playback, and
  `--raw` from a cast.
- `tmux-orchestration`: the queued `pipe-pane` command runs the session recorder instead of `cat`.
- `safe-process-invocation`: the quoted shell body is the recorder command. The binary path is
  quoted too, and tmux's `#` format character is escaped.
- `cli-parsing`: `replay` gains `--cast`.
- `core-terminal-output`: `replay <agent> --cast` joins `--raw` as a named escape-sequence
  exception.

## Impact

- **Code:** new `src/logging/cast.rs`, a pure asciinema v2 encoder and decoder covering the header,
  output and resize events, UTF-8 carry-over and continuation. The hidden `Record` command lives in
  `src/cli.rs` and its handler in `src/commands/record.rs`. The `pipe_pane` command string changes
  in `src/tmux/command.rs::TmuxSession::pipe_pane`, behind the terminal backend's output-stream
  operation. `commands/helpers.rs` capture paths become `.cast`. In replay, `--cast` playback goes
  through a small `CastPlayer` port (adapter: `asciinema` via `which` and `std::process::Command`
  with inherited stdio; a fake for tests), and `--raw` decodes casts.
- **Persisted state:** none. The capture file format changes for new records, and legacy `.log`
  captures stay readable by `--raw`.
- **Enum-variant ripple:** none.
- **Tests:** `src/tmux/tests.rs` pipe-pane tests (the command string changes) and the
  `safe-process-invocation` quoting tests. New recorder unit tests, a recorder e2e under real tmux,
  and fake-player playback tests.
- **Docs:** `user-guide/session-logging.md` (animated replay, installing asciinema, the size
  caveat), `cli-reference.md` (`--cast`), `architecture.md` (recorder), `--help`.
- **Security / safety:** the recorder writes only the path it is given, refuses symlinks and creates
  files 0600, which closes the `cat >>` check-then-open window from `session-record-capture` D5. It
  never touches the network or the broker. `asciinema` is executed from `PATH` with argv only (no
  shell), the same trust model as the previous `less` invocation. Played-back bytes reach the
  viewer's terminal unsanitized, as with `--raw`, and the documentation says so.
- **Dependencies:** none added. The built-in player alternative would need `vt100`, which needs
  approval.
- **Cross-version dependencies:**
  - `session-record-capture` (this release): the record layout and capture-on-every-path. Its
    MODIFIED *Attach pipe-pane to capture output* is sink-agnostic, so this change does not
    modify it again.
  - `replay-semantic-timeline` (this release): `--raw`, the `Replay subcommand` parse and record
    selection. This change adds a separate *Replay --cast flag* requirement instead of modifying
    the `Replay subcommand` requirement a second time. It does modify `core-terminal-output`
    *Human-readable output carries no escape sequences when not on a terminal* a second time, on
    the text `replay-semantic-timeline` leaves (the `--raw` exception), adding `--cast` only.
  - v0.18 `terminal-backend-seam`: the output-stream operation (`stream_to`) gains the recorder
    sink, inside its "tap a surface's output stream to a file" contract.
  - Only this change modifies `tmux-orchestration` *TmuxSession supports pipe-pane command* and
    `safe-process-invocation` *Paths interpolated into shell contexts are quoted*. v0.18
    `supervisor-pane-removal` touches only *Supervisor-mode pane layout*.
