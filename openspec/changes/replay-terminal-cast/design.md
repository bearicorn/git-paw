## Context

After `session-record-capture`, each coding agent's surface streams into
`<record>/<branch>.log` through the terminal backend's output-stream operation. The tmux adapter
implements that stream as `TmuxSession::pipe_pane` → `pipe-pane -o -t <pane> "cat >> <quoted path>"`.
The bytes are faithful, but they carry no timing. The roadmap's v0.18 note is explicit: owning PTYs
would not fix replay. Capture is fine; what is missing is timing plus a player.

Two facts shape the design:
- **The viewer's terminal is the emulator.** Re-running recorded output bytes at their recorded
  pace in a real terminal reconstructs the screen. That is what `asciinema play` does. Emulating a
  terminal inside git-paw is needed only to render frames somewhere other than the viewer's
  terminal (seeking, thumbnails, a TUI pane), and that would need a VT crate such as `vt100`,
  which is not approved.
- **asciinema v2 is a stable, widely supported format.** It has a JSON header line followed by
  `[time, code, data]` event lines. The asciinema CLI (`asciinema play <file>`) and the asciinema
  web player both play it.

## Goals / Non-Goals

**Goals:**
- A timed capture in a standard format, produced by git-paw itself, with zero new crates.
- `--cast` playback by shelling out to `asciinema` when present, and an actionable error when not.
- Recovery appends to the same recording.
- Closing the `cat >>` check-then-open window, because the recorder opens its own file.

**Non-Goals:**
- A built-in player (see Open Questions).
- Continuous resize tracking (only the size at each recorder start is recorded).
- Input (`"i"`) events, markers, or the asciinema v3 format.
- Capturing the dashboard or the broker host.

## Decisions

### D1: git-paw records the cast itself, via a hidden subcommand
`git paw __record <path> --cols C --rows R` is the imperative shell. It reads stdin in chunks,
stamps each chunk with a monotonic elapsed time, and hands it to a **pure encoder**
(`logging::cast`). The encoder turns `(chunk bytes, elapsed, carry-over state)` into event lines,
and parses an existing header for continuation. The encoder is tested with byte fixtures, with no
process and no clock. This is the code-standards hidden-subcommand IPC seam (`__dashboard`,
`__prompt-profile`).
*Alternatives:*
- Wrap the CLI in `asciinema rec` or `script -t`. Rejected: it changes how agents are
  launched, requires those tools at capture time, and `script`'s timing format differs between
  BSD and util-linux.
- Keep `cat >>` and add a timing sidecar (`scriptreplay` style). Rejected: it needs a second
  writer per pane and a custom player.

### D2: The sink is a backend concern
The terminal backend's output-stream operation (`stream_to` in `terminal-backend-seam`) takes the capture path and delivers the surface's output to the
recorder. On tmux this is
`pipe-pane -o -t <pane> "<exe> __record <path> --cols #{pane_width} --rows #{pane_height}"`,
with `<exe>` and `<path>` shell-quoted and tmux-`#`-escaped. A future PTY backend would feed the
same encoder in-process, and the fake backend records the request. No new port operation is
needed: the v0.18 requirement already covers "tapping a surface's output stream to a file".

**To verify** (tmux): that `pipe-pane` expands `#{…}` formats in its shell command. The tmux manual's
`pipe-pane -o 'cat >>~/output.#I-#P'` example implies it does. If formats are expanded, a literal
`#` in a path must be doubled (`##`), and that applies to today's `cat >>` command as well. If
formats are not expanded, the adapter instead queries each pane's size after the session executes
and issues `pipe-pane` per pane with literal numbers.

### D3: Encoding rules
- **Header:** `{"version": 2, "width": C, "height": R, "timestamp": <unix secs>}`. No `env` block,
  because `SHELL`/`TERM` add nothing for playback and leak environment.
- **Output events:** one per read, `elapsed` from a monotonic clock offset from the header
  timestamp, with microsecond precision.
- **UTF-8:** an incomplete trailing sequence is carried into the next read. Truly invalid bytes
  become U+FFFD, because v2 event data must be UTF-8 text. Agent TUIs emit UTF-8, so in practice
  `--raw` from a cast equals the captured bytes.
- **Continuation:** if `<path>` starts with a valid v2 header, events are appended with `elapsed`
  measured from that header's timestamp, so the idle gap across a stop/recover is preserved, and a
  resize event is written if C×R changed. A corrupt existing file is left alone. A non-empty file
  without a header is never appended to.
- **Durability:** each line is flushed when written, so a killed recorder loses at most one event.

**To verify** (format): the field names above and the `"r"` resize event code (`"COLSxROWS"`)
against the published asciinema v2 format documentation. Players ignore event codes they do not
know, so an unsupported `"r"` degrades harmlessly.

### D4: File safety
The recorder checks `symlink_metadata(path)` and refuses a symlink, then opens with
`OpenOptionsExt::mode(0o600)` in append mode. The window between the check and the open is now
inside git-paw's own process, with nothing in between, which narrows the gap that `cat >>` left
(`session-record-capture` D5). Without a `libc` dependency `O_NOFOLLOW` is not used. That residual
is accepted and documented.

### D5: Playback shells out; no built-in player
A `CastPlayer` port has one production adapter. It resolves `asciinema` with `which` and runs
`asciinema play <path>` through `std::process::Command` with inherited stdio, as an argument
vector with no shell. Its exit status is propagated. A fake player records invocations for tests.
Without `asciinema`, the command fails with the `.cast` path, since any v2 player (CLI or web) can
play it. **To verify:** `asciinema play <file>` plays v2 files with both the 2.x (Python) and 3.x
(Rust) CLIs.

### D6: Legacy captures
`.log` captures (legacy records, and records written between `session-record-capture` and this
change) stay readable by `--raw` byte-for-byte. `--cast` refuses them: there is no timing to
reconstruct.

## Risks / Trade-offs

- [The header size is the pane size when the recorder starts, which can be the pre-attach detached
  size. Panes resize when a client attaches.] → Full-screen TUIs repaint on resize, so the stream
  stays self-consistent. Playback in a terminal at least as large as the session's attached size
  is faithful. The size is re-recorded at every recorder start (recovery). Continuous resize
  sampling is an Open Question. Documented in the user guide.
- [One recorder process per agent for the session's lifetime] → the same process count as today's
  `cat`. Per-read JSON encoding is cheap next to TUI output rates.
- [Lossy U+FFFD replacement for invalid UTF-8] → documented. Agent CLIs emit UTF-8.
- [`asciinema` absent on most machines] → the error names the file and the install route. The
  timeline (the default) needs nothing extra.
- [Playback writes untrusted bytes to the viewer's terminal] → this is inherent to any terminal
  recording, and the same as `--raw`. The docs say to play only recordings of your own sessions.

## Migration Plan

New records get `.cast` captures. Old `.log` captures keep working with `--raw`. Rollback by
reverting. `.cast` files left behind remain playable by any asciinema player, and older binaries
simply do not list them as captures.

## Open Questions

- **Built-in player — user decision.**
  - (a) Shell-out only (this change; recommended).
  - (b) A zero-crate pass-through player that writes output events to stdout at their recorded
    pace with an idle-time cap. It is feasible because the viewer's terminal is the emulator, but
    it duplicates `asciinema play` and needs careful terminal restoration on interrupt (alternate
    screen, cursor).
  - (c) A true VT player (seek, render frames, play inside a TUI). Needs a new crate (`vt100` or
    `wezterm-term`), which needs approval; it is only worth it if the v0.18 dashboard question
    lands on "the TUI survives and should embed playback".
- **Continuous resize tracking.** The recorder could sample the surface size (a new backend
  operation, "report a surface's size") at most once per second while output flows and emit
  `"r"` events. Deferred until real recordings show the startup-size limitation matters.
- **Playback controls.** Pass-through of `asciinema play` options (`--speed`,
  `--idle-time-limit`)? Not added. Users can run `asciinema play` on the path directly.
