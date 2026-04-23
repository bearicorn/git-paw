## Why

When all agents are verified and merges complete, the session is over — but there's no record of what happened. The supervisor should write a summary documenting each agent's work, timing, test results, spec audit status, and the merge order. This is the "session learnings" foundation for v0.5.0.

## What Changes

- Add a `write_session_summary()` function to a new `src/summary.rs` module
- The supervisor calls this after all agents are verified and merges complete
- The summary is written to `.git-paw/session-summary.md` containing:
  - Session metadata: project name, date, duration, agent count
  - Per-agent details: branch, CLI, duration, files modified, exports, test results, spec audit status, blocked time
  - Merge order: the sequence in which branches were merged
  - Totals: total agents, total time, total tests, conflicts resolved
- Data sources: broker messages (agent records, message log), session state, git log
- The summary file is human-readable Markdown, suitable for commit messages or PR descriptions

## Capabilities

### New Capabilities

- `session-summary`: Generate and write `.git-paw/session-summary.md` from broker state and session data

## Impact

- **New file:** `src/summary.rs`
- **Modified files:** `src/lib.rs` (add `pub mod summary`), `src/main.rs` (supervisor calls `write_session_summary` at end)
- **No new dependencies.**
- **Depends on:** `supervisor-agent` (called at end of supervisor workflow), broker state (reads agent records and message log)
