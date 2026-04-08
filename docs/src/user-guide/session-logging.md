# Session Logging

git-paw can capture the full terminal output of each AI CLI pane to log files. This lets you review what happened in a session after it ends.

## Enabling Logging

Add the `[logging]` section to your `.git-paw/config.toml` (or global config):

```toml
[logging]
enabled = true
```

When enabled, git-paw uses `tmux pipe-pane` to capture each pane's output to a log file when a session starts.

## Log Storage

Logs are stored under `.git-paw/logs/` organized by session:

```
.git-paw/logs/
  paw-my-project/
    feat--add-auth.log
    fix--pagination.log
```

Branch names are sanitized for filenames — `/` becomes `--` (e.g., `feat/add-auth` becomes `feat--add-auth.log`).

The `git paw init` command automatically adds `.git-paw/logs/` to `.gitignore` so logs are never committed.

## Replaying Logs

Use `git paw replay` to view captured logs.

### List available sessions and branches

```bash
git paw replay --list
```

Shows all sessions and their logged branches.

### Replay a branch

```bash
git paw replay feat/add-auth
```

Branch names are fuzzy-matched — you can use the original branch name, the sanitized filename, or a partial match.

By default, ANSI escape codes are stripped for clean text output.

### Replay with colors

```bash
git paw replay feat/add-auth --color
```

Preserves ANSI colors and pipes through `less -R` for scrollable colored output.

### Replay from a specific session

```bash
git paw replay feat/add-auth --session paw-my-project
```

By default, the most recent session is used. Use `--session` to specify an older one.

### Flags

| Flag | Description |
|------|-------------|
| `--list` | List available log sessions and branches |
| `--color` | Display with ANSI colors via `less -R` |
| `--session <name>` | Session to replay from (defaults to most recent) |
