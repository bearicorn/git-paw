## 1. Create src/summary.rs module

- [ ] 1.1 Create `src/summary.rs` with a module-level doc comment (`//!`)
- [ ] 1.2 Declare `pub mod summary` in `src/lib.rs`
- [ ] 1.3 Define `pub fn write_session_summary(state: &BrokerState, session: &PawSession, merge_order: &[String], output_path: &Path) -> Result<(), PawError>`
- [ ] 1.4 Add doc comment on the function describing parameters and output format

## 2. Extract per-agent data from BrokerState

- [ ] 2.1 Collect `agent_status_snapshot` for all known agents
- [ ] 2.2 For each agent, scan the message log for the last `agent.artifact` message from that agent
- [ ] 2.3 Extract `modified_files` and `exports` from the last artifact payload (default to empty if none)
- [ ] 2.4 Compute blocked time: find pairs of `agent.blocked` → next `agent.status`/`agent.artifact` for the same agent, sum the gaps
- [ ] 2.5 Use `(None, None)` for files and exports when no artifact message exists

## 3. Generate session metadata section

- [ ] 3.1 Format: `# Session Summary — <project_name> — <YYYY-MM-DD>`
- [ ] 3.2 Include `**Duration:**` computed from `session.started_at` to now
- [ ] 3.3 Include `**Agents:** N` from agent count
- [ ] 3.4 Include `**Merge order:**` from `merge_order` parameter (comma-separated)

## 4. Generate per-agent section

- [ ] 4.1 Format: `## Agents` then one `### <branch> (<cli>)` subsection per agent
- [ ] 4.2 Include `- **Status:**` from agent's `status` field
- [ ] 4.3 Include `- **Files modified:**` as comma-separated list, or `(none)` if empty
- [ ] 4.4 Include `- **Exports:**` as comma-separated list, or `(none)` if empty
- [ ] 4.5 Include `- **Estimated blocked time:**` formatted as human-readable duration, or `none` if zero

## 5. Generate totals section

- [ ] 5.1 Format: `## Totals`
- [ ] 5.2 Include `- Total agents: N`
- [ ] 5.3 Include `- Total time: <duration>`

## 6. Write summary to file

- [ ] 6.1 Assemble full Markdown string from all sections
- [ ] 6.2 Write to `output_path` using `std::fs::write`
- [ ] 6.3 Map write errors to `PawError` (use `PawError::IoError` or equivalent)
- [ ] 6.4 Overwrite if file already exists (no append)

## 7. Update git paw init to add session-summary.md to .gitignore

- [ ] 7.1 In `src/init.rs`, add `.git-paw/session-summary.md` to the gitignore entries alongside `.git-paw/logs/`
- [ ] 7.2 Apply the same idempotency check (don't add if already present)

## 8. Call write_session_summary from supervisor handler

- [ ] 8.1 In `src/main.rs` `cmd_supervisor()`, after all agents are verified and merges complete, call `write_session_summary`
- [ ] 8.2 Pass `output_path = repo_root.join(".git-paw/session-summary.md")`
- [ ] 8.3 Log a message to stdout: "Session summary written to .git-paw/session-summary.md"
- [ ] 8.4 Summary write failure SHALL NOT crash the supervisor — log warning and continue

## 9. Unit tests

- [ ] 9.1 Test: `write_session_summary` with two agents creates a file at the specified path
- [ ] 9.2 Test: output contains the project name from `PawSession`
- [ ] 9.3 Test: output contains "Agents" with the correct count
- [ ] 9.4 Test: output contains merge order in the correct sequence
- [ ] 9.5 Test: per-agent section shows `(none)` for files when no artifact message exists
- [ ] 9.6 Test: per-agent section shows `modified_files` from the last artifact message
- [ ] 9.7 Test: per-agent section shows `exports` from the last artifact message
- [ ] 9.8 Test: existing file at output path is overwritten
- [ ] 9.9 Test: write to read-only path returns `Err`
- [ ] 9.10 Test: `git paw init` adds `.git-paw/session-summary.md` to `.gitignore`
- [ ] 9.11 Test: `git paw init` does not duplicate `.git-paw/session-summary.md` in `.gitignore`

## 10. Quality gates

- [ ] 10.1 `cargo fmt` clean
- [ ] 10.2 `cargo clippy --all-targets -- -D warnings` clean
- [ ] 10.3 `cargo test` — all tests pass (new + existing)
- [ ] 10.4 `cargo doc --no-deps` — no warnings
- [ ] 10.5 `just check` — full pipeline green
- [ ] 10.6 Verify `src/lib.rs` compiles with `pub mod summary`

## 11. Handoff readiness

- [ ] 11.1 `write_session_summary` is a public function accessible as `git_paw::summary::write_session_summary`
- [ ] 11.2 The function signature matches the spec (`state`, `session`, `merge_order`, `output_path`)
- [ ] 11.3 New files: `src/summary.rs` only; modified files: `src/lib.rs`, `src/main.rs`, `src/init.rs`
- [ ] 11.4 Summary failure is non-fatal (logged warning, supervisor continues)
- [ ] 11.5 Commit with message: `feat(summary): add session summary generation to .git-paw/session-summary.md`
