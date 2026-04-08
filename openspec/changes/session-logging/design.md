## Context

tmux provides `pipe-pane` which pipes all output from a pane to a shell command. Running `tmux pipe-pane -o -t <pane> "cat >> <path>"` captures everything displayed in the pane — including AI CLI output, prompts, and tool results — to a file. This is append-mode (`-o` toggles on), so it can be attached at any time.

The `init-command` change creates `.git-paw/logs/` and adds the `[logging]` config section. This change implements the actual capture logic and log management.

## Goals / Non-Goals

**Goals:**
- Capture raw terminal output per pane to `.git-paw/logs/<session-id>/<branch>.log`
- Create session log directory at launch time
- Provide functions to enumerate sessions and logs (for `replay-command` to consume)
- Integrate with the `TmuxSession` builder so logging is applied after each pane is created
- No-op when logging is disabled in config

**Non-Goals:**
- ANSI stripping or formatting (that's `replay-command`)
- Log rotation or cleanup (users manage via `purge` or manually)
- Structured logging or JSON output (raw terminal capture only)
- Capturing stdin (only stdout/stderr via pipe-pane)

## Decisions

### Decision 1: Session ID from session name

The log directory uses the tmux session name (e.g., `paw-myproject`) as the session ID. This creates `.git-paw/logs/paw-myproject/<branch>.log`.

**Why:** The session name is unique per repo (enforced by tmux) and human-readable. Using it as the directory name makes log discovery intuitive.

**Alternative considered:** Timestamp-based session ID (e.g., `2026-03-27-143022`). Rejected — multiple sessions per day would be confusing, and the session name is already unique.

### Decision 2: Branch name sanitization for log filenames

Branch names like `feat/add-auth` contain `/` which can't be in filenames. Sanitize by replacing `/` with `--` (e.g., `feat--add-auth.log`).

**Why:** Double-dash is visually distinct from single-dash (which branches already use) and reversible. The same sanitization used in `worktree_dir_name()` in `git.rs`.

### Decision 3: `pipe-pane` via TmuxCommand

Add a `pipe_pane()` method to `TmuxSession` that appends a `pipe-pane` `TmuxCommand` to the command queue, targeting a specific pane.

```rust
pub fn pipe_pane(&mut self, pane_target: &str, log_path: &Path) -> &mut Self
```

**Why:** Fits the existing builder pattern. The command is queued alongside `split-window`, `send-keys`, etc. and executed in order. In dry-run mode, it's printed but not executed.

### Decision 4: Logging module owns directory management, hardcoded path

`logging.rs` provides `ensure_log_dir()` which creates the session-specific directory. The base log path is hardcoded to `.git-paw/logs/` — the `LoggingConfig` struct only has an `enabled: bool` field, with no configurable `log_dir`.

**Why:** The `init` command creates the top-level `.git-paw/logs/` directory. The logging module creates per-session subdirectories at launch time. This separation means `init` doesn't need to know about sessions. A configurable log directory was considered but deemed unnecessary for v0.2.0 — all logs live under `.git-paw/logs/` by convention.

### Decision 5: Enumeration functions for replay

```rust
pub fn list_log_sessions(repo_root: &Path) -> Result<Vec<String>, PawError>
pub fn list_logs_for_session(repo_root: &Path, session: &str) -> Result<Vec<LogEntry>, PawError>
```

Where `LogEntry` has `branch: String` and `path: PathBuf`.

**Why:** The `replay-command` change needs to discover and list logs. Putting enumeration in `logging.rs` keeps all log-related logic in one module.

## Risks / Trade-offs

**[Large log files]** → Long AI sessions can produce multi-MB logs with ANSI codes. → Acceptable — logs are gitignored and users can delete them. No rotation in v0.2.0.

**[pipe-pane portability]** → `pipe-pane` is a standard tmux feature available since tmux 1.0. No portability concern.

**[Concurrent writes]** → Each pane writes to its own file. No contention.

**[Logging after recovery]** → If a session is recovered via `start` (reattach), logging is not re-enabled for existing panes. → Acceptable for v0.2.0 — logging starts at session creation time.
