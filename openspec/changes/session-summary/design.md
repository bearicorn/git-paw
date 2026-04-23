## Context

At the end of a supervisor session, all context about what happened exists in transient state: the broker's in-memory message log, session state files, and git history. Without a summary, this context is lost when the session ends. The session summary captures it in a durable, human-readable Markdown file that serves as the foundation for v0.5.0 session learnings.

## Goals / Non-Goals

**Goals:**

- Write `.git-paw/session-summary.md` after all agents are verified and merges complete
- Include per-agent details, merge order, timing, and totals
- Source data from broker messages, session state, and git log
- The file must be human-readable and suitable for use as a commit message or PR description

**Non-Goals:**

- Machine-readable output (JSON/TOML summary) — v0.5.0 concern
- Automatic PR creation using the summary — v0.5.0 concern
- Summary of non-supervisor sessions — out of scope
- Real-time summary updates during the session — written once at end

## Decisions

### Decision 1: New `src/summary.rs` module with a single `write_session_summary()` function

The summary logic is self-contained and doesn't fit any existing module. A dedicated module keeps concerns separated and makes the code easy to test in isolation.

```rust
pub fn write_session_summary(
    state: &BrokerState,
    session: &PawSession,
    output_path: &Path,
) -> Result<(), PawError>
```

**Why:**
- Single responsibility — the function takes all needed data and produces one file
- Pure data extraction from `BrokerState` and `PawSession` — no side effects except the file write
- Easy to test with a temp directory for `output_path`

**Alternatives considered:**
- *Add `write_session_summary` to `src/session.rs`.* Session module handles persistence of live state, not post-session summaries. Different concern. Rejected.
- *Inline in `src/main.rs` supervisor handler.* Would make the main handler too long. Rejected.

### Decision 2: Data is sourced from three inputs

| Data | Source |
|------|--------|
| Per-agent: branch, CLI, status | `BrokerState.agents` records |
| Per-agent: modified files, exports | Last `agent.artifact` message per agent from message log |
| Timing: duration, blocked time | Message log timestamps |
| Merge order | Passed from supervisor merge logic |
| Test results | Supervisor's `test_command` run results (stored in session) |
| Spec audit status | `agent.artifact` payload `status` field |
| Session metadata | `PawSession` (project name, start time) |

**Why:**
- Broker state already accumulates all this data — no new tracking needed
- Merge order is determined by the supervisor, so it's passed as a parameter rather than re-derived
- Using the existing message log means no new data structures

**Alternatives considered:**
- *Write a separate `summary_state` struct during the session.* Duplicates data already in the broker. Rejected.
- *Re-run `git log` for timing data.* Git commit timestamps diverge from session timing. Broker message timestamps are more accurate. Rejected.

### Decision 3: Output format is Markdown, written once at session end

The summary is written to `.git-paw/session-summary.md`. It is **not** committed automatically — the user decides whether to include it in their merge commit or PR.

**Structure:**
```markdown
# Session Summary — <project> — <date>

## Overview
- **Duration:** ...
- **Agents:** N
- **Merge order:** branch-a, branch-b, branch-c

## Agents

### feat-config (claude)
- **Status:** verified
- **Duration:** Xm Ys
- **Files modified:** src/config.rs, src/init.rs
- **Exports:** SupervisorConfig, ApprovalLevel
- **Test result:** pass
- **Spec audit:** clean
- **Blocked time:** none

...

## Totals
- Total agents: N
- Total time: ...
- Tests run: N
- Conflicts resolved: N
```

**Why:**
- Markdown renders in GitHub PRs and most editors
- The structure maps 1:1 to fields in `BrokerState` and `PawSession`
- "Suitable for commit message or PR description" is satisfied by the plain-text headings

**Alternatives considered:**
- *TOML output for machine reading.* Adds a second output format with no v0.4.0 consumer. Rejected.
- *Write to session state JSON.* Not human-readable without tooling. Rejected.

### Decision 4: `.git-paw/session-summary.md` is added to `.gitignore` by `git paw init`

Like `.git-paw/logs/`, the session summary is a generated working file that should not be committed automatically. `git paw init` adds `.git-paw/session-summary.md` to `.gitignore`.

**Alternatives considered:**
- *Committed automatically.* The user may not want it in every repo's history. Rejected.
- *Written to `/tmp/`.* Not accessible after reboot. Rejected.

## Risks / Trade-offs

- **Timing data accuracy** — blocked time is derived from the gap between `agent.blocked` and the next `agent.status` or `agent.artifact` from the same agent. This is an approximation. Mitigation: label it "estimated blocked time" in the output.
- **Empty summary if broker had no messages** — if the session ended early (e.g., all agents failed immediately), the summary may have minimal content. Mitigation: write a summary regardless, noting which agents never published.
