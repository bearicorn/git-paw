## Why

Session logging captures raw terminal output with ANSI escape codes. Users need a way to review these logs — listing available sessions, viewing a specific branch's output, and choosing between clean (stripped) or colored output. `git paw replay` provides this read-side interface to the logs captured by `session-logging`.

## What Changes

- New `replay` subcommand added to the CLI:
  - `git paw replay --list` — list available log sessions and their branches
  - `git paw replay <branch>` — display log output with ANSI codes stripped (clean text)
  - `git paw replay <branch> --color` — display log output with ANSI codes preserved, piped through `less -R`
  - `git paw replay <branch> --session <name>` — specify which session to replay (defaults to most recent)
- New `src/replay.rs` module providing:
  - ANSI escape code stripping
  - Log reading and display
  - Session selection logic (default to most recent)

## Capabilities

### New Capabilities
- `replay-command`: Read and display captured session logs with ANSI stripping, color passthrough, and session/branch listing

### Modified Capabilities
- `cli-parsing`: New `Replay` subcommand variant added to the `Command` enum

## Impact

- **New files**: `src/replay.rs`
- **Modified files**: `src/cli.rs` (new `Replay` subcommand), `src/main.rs` (wire replay)
- **No new dependencies** — ANSI stripping via regex-free byte scanning, output via `std::io::stdout`, colored output piped through `less -R` via `std::process::Command`
- **Depends on**: `session-logging` (provides `list_log_sessions()`, `list_logs_for_session()`, `LogEntry`)
