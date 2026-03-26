## 1. Module Setup

- [ ] 1.1 Create `src/logging.rs` with module-level doc comment (`//! Session logging via tmux pipe-pane`)
- [ ] 1.2 Add `mod logging;` declaration in `src/lib.rs` or `src/main.rs`

## 2. Data Types

- [ ] 2.1 Define `LogEntry` struct with fields: `branch: String`, `path: PathBuf`
- [ ] 2.2 Add doc comments to `LogEntry`

## 3. Path Helpers

- [ ] 3.1 Implement `sanitize_branch_for_filename(branch: &str) -> String` — replace `/` with `--`
- [ ] 3.2 Implement `unsanitize_branch_from_filename(filename: &str) -> String` — replace `--` with `/`, strip `.log`
- [ ] 3.3 Implement `log_file_path(repo_root: &Path, session_id: &str, branch: &str) -> PathBuf` — returns `.git-paw/logs/<session-id>/<sanitized-branch>.log`

## 4. Directory Management

- [ ] 4.1 Implement `ensure_log_dir(repo_root: &Path, session_id: &str) -> Result<PathBuf, PawError>` — creates `.git-paw/logs/<session-id>/`, returns the path
- [ ] 4.2 No-op if directory already exists

## 5. Tmux Integration

- [ ] 5.1 Add `pipe_pane(&mut self, pane_target: &str, log_path: &Path) -> &mut Self` method to `TmuxSession` in `src/tmux.rs`
- [ ] 5.2 Queue `TmuxCommand` with args: `["pipe-pane", "-o", "-t", pane_target, &format!("cat >> {}", log_path.display())]`

## 6. Log Enumeration

- [ ] 6.1 Implement `list_log_sessions(repo_root: &Path) -> Result<Vec<String>, PawError>` — list subdirectories of `.git-paw/logs/`, return empty vec if dir doesn't exist
- [ ] 6.2 Implement `list_logs_for_session(repo_root: &Path, session: &str) -> Result<Vec<LogEntry>, PawError>` — list `.log` files, derive branch from filename
- [ ] 6.3 Return `PawError::SessionError` if session directory doesn't exist

## 7. Unit Tests

- [ ] 7.1 Test `sanitize_branch_for_filename`: simple name, single slash, multiple slashes
- [ ] 7.2 Test `unsanitize_branch_from_filename`: reverse of sanitize cases
- [ ] 7.3 Test `log_file_path`: produces correct path structure
- [ ] 7.4 Test `ensure_log_dir`: creates directory, idempotent on re-call
- [ ] 7.5 Test `list_log_sessions`: multiple sessions, empty, no logs dir
- [ ] 7.6 Test `list_logs_for_session`: multiple logs, empty session, nonexistent session
- [ ] 7.7 Test `LogEntry.branch` derivation from sanitized filename
- [ ] 7.8 Test `pipe_pane` queues correct TmuxCommand in builder
- [ ] 7.9 Test `pipe_pane` appears in dry-run command strings
- [ ] 7.10 Run `cargo clippy -- -D warnings` and `cargo fmt --check` — clean
