## 1. CLI Subcommand

- [ ] 1.1 Add `Replay` variant to `Command` enum in `src/cli.rs` with fields: `branch: Option<String>`, `list: bool`, `color: bool`, `session: Option<String>`
- [ ] 1.2 Add `about`, `long_about` with examples, and `help` strings for all flags
- [ ] 1.3 Add validation: require either `branch` or `--list` (use clap `group` or `required_unless_present`)
- [ ] 1.4 Unit tests: parse with branch, parse with --list, parse with --color, parse with --session, parse with no args fails

## 2. Module Setup

- [ ] 2.1 Create `src/replay.rs` with module-level doc comment (`//! Replay captured session logs`)
- [ ] 2.2 Add `mod replay;` declaration in `src/lib.rs` or `src/main.rs`

## 3. ANSI Stripping

- [ ] 3.1 Implement `strip_ansi(input: &str) -> String` — byte-by-byte state machine removing CSI sequences (`\x1b[...` through final byte)
- [ ] 3.2 Handle SGR (`m`), cursor movement (`H`, `J`, `K`, `A`-`D`), and other CSI final bytes (`@`-`~`)
- [ ] 3.3 Handle incomplete escape sequences at end of input gracefully

## 4. Session Selection

- [ ] 4.1 Implement `resolve_session(repo_root: &Path, session_flag: Option<&str>) -> Result<String, PawError>` — returns session name
- [ ] 4.2 If `session_flag` is Some → validate it exists, return it
- [ ] 4.3 If None → call `list_log_sessions()`, sort by directory mtime, return most recent
- [ ] 4.4 Error if no sessions exist or specified session not found

## 5. Branch Matching

- [ ] 5.1 Implement `find_log(repo_root: &Path, session: &str, branch_query: &str) -> Result<PathBuf, PawError>` — fuzzy match against original branch name and sanitized filename
- [ ] 5.2 Call `list_logs_for_session()`, match `branch_query` against `LogEntry.branch` and sanitized filename
- [ ] 5.3 Error if no match found, listing available branches in the error message

## 6. Display

- [ ] 6.1 Implement `replay_stripped(log_path: &Path) -> Result<(), PawError>` — read file, strip ANSI, write to stdout
- [ ] 6.2 Implement `replay_colored(log_path: &Path) -> Result<(), PawError>` — pipe raw content through `less -R`
- [ ] 6.3 Fallback: if `less` not on PATH, print raw to stdout with warning to stderr
- [ ] 6.4 Handle empty log file gracefully (no output, no error)

## 7. List Display

- [ ] 7.1 Implement `display_list(repo_root: &Path) -> Result<(), PawError>` — enumerate sessions and branches, format output
- [ ] 7.2 Show session name, branch count, and filename → branch mapping for each session
- [ ] 7.3 Handle no sessions case with helpful message

## 8. Wire Up

- [ ] 8.1 Add `Command::Replay` match arm in `src/main.rs` dispatch
- [ ] 8.2 Route `--list` to `display_list()`, branch to `replay_stripped()` or `replay_colored()` based on `--color`

## 9. Unit Tests

- [ ] 9.1 Test `strip_ansi`: plain text unchanged, SGR removed, cursor sequences removed, multiple sequences per line, incomplete sequence handled
- [ ] 9.2 Test `resolve_session`: explicit session found, explicit not found → error, default to most recent
- [ ] 9.3 Test `find_log`: match by original branch name, match by sanitized name, no match → error with available list
- [ ] 9.4 Test `display_list`: formats sessions and branches correctly

## 10. Integration Tests

- [ ] 10.1 Create temp repo with log files, run `git paw replay --list` → shows sessions and branches
- [ ] 10.2 Run `git paw replay <branch>` → displays stripped content
- [ ] 10.3 Run `git paw replay <branch>` with no logs dir → error suggesting logging not enabled
- [ ] 10.4 Run `git paw replay nonexistent` → error listing available branches
- [ ] 10.5 Run `cargo clippy -- -D warnings` and `cargo fmt --check` — clean
