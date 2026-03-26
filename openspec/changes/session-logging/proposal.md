## Why

When running parallel AI coding sessions, there's no record of what each agent did. Debugging issues, reviewing agent output, or understanding what happened in a session requires scrollback through tmux history — which is limited and lost when the session ends. Session logging captures terminal output per pane to disk, giving users a persistent, reviewable record of each agent's work.

## What Changes

- New `src/logging.rs` module providing:
  - Log directory management: create `.git-paw/logs/<session-id>/` per session
  - Per-pane log path derivation: `.git-paw/logs/<session-id>/<branch>.log`
  - `enable_logging_for_pane()` — attaches `tmux pipe-pane` to a pane, capturing output to its log file
  - `list_log_sessions()` — lists available session log directories
  - `list_logs_for_session()` — lists log files within a session directory
- Logging is off by default, enabled via `[logging] enabled = true` in config
- When enabled, `pipe-pane` is attached to each pane immediately after the pane is created during `start`
- Log files contain raw terminal output including ANSI escape codes (stripping is done at read time by `replay-command`)

## Capabilities

### New Capabilities
- `session-logging`: Per-pane terminal output capture via `tmux pipe-pane`, log directory management, session/log enumeration

### Modified Capabilities
- `tmux-orchestration`: After pane creation, optionally attach `pipe-pane` for logging

## Impact

- **New files**: `src/logging.rs`
- **Modified files**: `src/tmux.rs` (add `pipe-pane` command to `TmuxSession` builder), `src/main.rs` or `src/lib.rs` (add `mod logging;`)
- **No new dependencies** — uses `std::fs` for directory management, tmux `pipe-pane` for capture
- **Depends on**: `init-command` (creates `.git-paw/logs/` directory and adds `[logging]` config section)
