## Context

The `session-logging` change captures raw terminal output to `.git-paw/logs/<session>/<branch>.log`. These files contain ANSI escape codes (colors, cursor movement, clearing) from AI CLI output. Users need to read these logs in two modes: clean text (for searching, piping) and colored (for visual review).

The `logging.rs` module provides `list_log_sessions()` and `list_logs_for_session()` for discovery. This change adds the read/display layer.

## Goals / Non-Goals

**Goals:**
- List available sessions and branches with `--list`
- Display stripped log output by default (clean, pipeable text)
- Display colored output with `--color` via `less -R`
- Default to the most recent session when `--session` is not specified
- Handle missing logs gracefully with actionable errors

**Non-Goals:**
- Live/streaming replay (tailing active logs)
- Searching within logs (users pipe to `grep`)
- Log editing or deletion (users use `purge` or manual `rm`)
- Structured parsing of AI CLI output

## Decisions

### Decision 1: ANSI stripping without regex crate

Strip ANSI escape codes using a byte-by-byte state machine that detects `ESC[...m` (SGR), `ESC[...H` (cursor), and other CSI sequences. No regex dependency needed.

**Why:** The `regex` crate is not in the approved dependency list. ANSI CSI sequences follow a well-defined pattern (`\x1b[` followed by parameters and a final byte in `@`-`~` range). A state machine handles this reliably in < 50 lines.

**Alternative considered:** Add `regex` dependency. Rejected — not approved, and the problem is simple enough for direct parsing.

### Decision 2: `less -R` for colored output

When `--color` is passed, pipe the raw log content through `less -R` which interprets ANSI codes. This gives the user a scrollable, colored view.

**Why:** `less -R` is universally available on macOS and Linux, handles large files, and provides search (`/`), scrolling, and exit (`q`). Reimplementing a pager would be unnecessary.

**Alternative considered:** Print raw ANSI to stdout and let the user pipe to their preferred pager. Rejected — raw ANSI output is unusable without a pager, and most users expect `--color` to "just work".

### Decision 3: Most recent session as default

When `--session` is not specified, select the session directory with the most recent modification time. `list_log_sessions()` already returns directory names; this change sorts by `mtime`.

**Why:** Users almost always want to replay the most recent session. Requiring `--session` for the common case would be tedious.

### Decision 4: `--list` shows sessions and branches

`--list` output format:
```
paw-myproject (3 branches)
  feat--add-auth.log  →  feat/add-auth
  feat--fix-bug.log   →  feat/fix-bug
  main.log            →  main
```

Shows session name, log file count, and each branch with its sanitized filename → original branch name mapping.

**Why:** Users need to know which branch names to pass to `replay <branch>`. Showing the mapping helps when branch names were sanitized.

### Decision 5: Branch matching is fuzzy

When the user passes `<branch>` to `replay`, match against both the sanitized filename and the original branch name. So both `git paw replay feat/add-auth` and `git paw replay feat--add-auth` work.

**Why:** Users shouldn't have to remember the sanitization scheme. Matching both forms is forgiving.

## Risks / Trade-offs

**[less not available]** → `less` is missing on some minimal Docker containers. → Fallback: if `less` is not found, print raw output to stdout with a warning. This is an edge case.

**[Very large logs]** → Stripped mode reads the entire file into memory for ANSI processing. → Acceptable for v0.2.0 — logs are typically < 5MB. Streaming ANSI stripping could be added later if needed.

**[Multiple sessions with same name]** → tmux session names include collision suffixes (e.g., `paw-myproject-2`). Log directories mirror this. No ambiguity.
