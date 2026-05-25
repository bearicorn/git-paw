# CLI Reference

git-paw is invoked as `git paw` (or `git-paw`). Below is the reference for all subcommands and flags.

## `git paw`

Running with no subcommand is equivalent to `git paw start`.

```
Parallel AI Worktrees — orchestrate multiple AI coding CLI sessions across git worktrees

Usage: git-paw [COMMAND]

Commands:
  start       Launch a new session or reattach to an existing one
  pause       Soft-stop: detach client, stop broker, keep CLIs running
  stop        Stop the session (kills tmux, keeps worktrees and state)
  purge       Remove everything (tmux session, worktrees, and state)
  status      Show session state for the current repo
  list-clis   List detected and custom AI CLIs
  add-cli     Register a custom AI CLI
  remove-cli  Unregister a custom AI CLI
  init        Initialize the repository for git-paw (creates .git-paw/)
  replay      Replay a captured pane log (requires session logging)
  help        Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

## `git paw init`

Initializes a repository for git-paw. Creates the `.git-paw/` directory, a default `config.toml`, the logs directory, and sets up `.gitignore`.

```
Usage: git-paw init

Options:
  -h, --help  Print help
```

Running `init` is idempotent — it's safe to run multiple times.

**What it creates:**
- `.git-paw/config.toml` — default configuration
- `.git-paw/logs/` — log directory (added to `.gitignore`)

**Example:**
```bash
git paw init
```

## `git paw start`

Smart start: reattaches if a session is active, recovers if stopped/crashed, or launches a new interactive session.

```
Usage: git-paw start [OPTIONS]

Options:
      --cli <CLI>              AI CLI to use (skips CLI picker)
      --branches <BRANCHES>    Comma-separated branches (skips branch picker)
      --from-all-specs         Launch from every discovered spec across all configured formats
      --specs [<NAMES>...]     Comma-separated spec names; bare flag opens picker (TTY required)
      --specs-format <FORMAT>  Override spec backend: openspec, markdown, speckit
      --dry-run                Preview the session plan without executing
      --preset <PRESET>        Use a named preset from config
      --supervisor             Run the session in supervisor mode (auto-start agents,
                                run test_command between merges, write session summary)
      --no-supervisor          Disable supervisor for this session, overriding any
                                `[supervisor] enabled = true` in config
      --force                  With `--from-all-specs`/`--specs`, bypass the uncommitted-spec warning
      --no-rebase              Skip rebasing existing agent branches onto the default branch
  -h, --help                   Print help
```

| Flag | Accepted values | Purpose |
|------|-----------------|---------|
| `--cli` | name of a detected or custom CLI | Skip the interactive CLI picker; assign this CLI to every agent that doesn't otherwise pin one. |
| `--branches` | comma-separated branches | Skip the interactive branch picker; launch one worktree per branch. |
| `--from-all-specs` | (flag) | Launch every discovered spec across the configured backend. Mutually exclusive with `--specs`. |
| `--specs` | comma-separated spec names; bare flag opens a multi-select picker (TTY required) | Narrow the session to named specs or open the picker. Mutually exclusive with `--from-all-specs`. |
| `--specs-format` | `openspec`, `markdown`, `speckit` | Override `[specs] type` in config and the `.specify/` auto-detection for this launch. |
| `--dry-run` | (flag) | Print the session plan; create no worktrees and run no tmux commands. |
| `--preset` | preset name from config | Use a named `[presets.<name>]` entry. |
| `--supervisor` | (flag) | Force supervisor mode on. Mutually exclusive with `--no-supervisor`. |
| `--no-supervisor` | (flag) | Force supervisor mode off (highest precedence in the resolution chain). Mutually exclusive with `--supervisor`. |
| `--force` | (flag) | Bypass the uncommitted-spec validation warning when launching from specs. |
| `--no-rebase` | (flag) | Skip the default-on rebase of existing agent branches onto the repository's default branch. |

`--from-all-specs` and `--specs` are mutually exclusive — one launches every
discovered spec, the other narrows to a subset or opens the picker.

**Examples:**
```bash
git paw start
git paw start --cli claude
git paw start --cli claude --branches feat/auth,feat/api
git paw start --dry-run
git paw start --preset backend

# Launch every discovered spec
git paw start --from-all-specs
git paw start --from-all-specs --cli claude
git paw start --from-all-specs --dry-run

# Narrow to specific specs or open the multi-select picker
git paw start --specs add-auth,fix-session
git paw start --specs   # interactive picker (requires a TTY)

# Skip supervisor for this session even when `[supervisor] enabled = true` is set
git paw start --no-supervisor
git paw start --from-all-specs --no-supervisor
```

### Supervisor mode resolution chain

git-paw decides whether to enter supervisor mode using this order (first match wins):

1. `--no-supervisor` flag present → supervisor disabled (no prompt, regardless of config).
2. `--supervisor` flag present → supervisor enabled (no prompt).
3. `[supervisor] enabled = true` in config → supervisor enabled (no prompt).
4. `[supervisor] enabled = false` in config → supervisor disabled (no prompt).
5. No `[supervisor]` section + `--dry-run` → supervisor disabled (skip prompt).
6. No `[supervisor]` section + interactive TTY → prompt "Start in supervisor mode?".
7. No `[supervisor]` section + non-TTY → supervisor disabled (fallback).

`--supervisor` and `--no-supervisor` are mutually exclusive at parse time; passing both is rejected by clap before any command runs.

See [Spec-Driven Launch](user-guide/spec-driven-launch.md) for details on spec formats and configuration.

## `git paw pause`

Soft-stops the session: detaches the tmux client, stops the broker, and leaves every CLI pane running in the background. Preserves agent conversation state for instant resume via `git paw start`. RAM stays allocated (~300 MB per Claude pane).

Use pause for short breaks (lunch, meetings, end-of-day). For longer breaks, use `git paw stop` to kill the CLIs and release RAM. See [Pause and Resume](user-guide/pause.md) for the full trade-off discussion.

```
Usage: git-paw pause

Options:
  -h, --help  Print help
```

**Example:**
```bash
git paw pause
```

Idempotent: pausing an already-paused or already-stopped session is a friendly no-op.

## `git paw stop`

Kills the tmux session and every CLI pane process, but preserves worktrees and session state on disk. CLI conversation context is lost. Run `git paw start` later to recover the session with fresh CLI processes.

**v0.5.0 change:** `stop` now prompts for confirmation when stdin is a TTY. Pass `--force` to skip the prompt (scripts); non-TTY contexts (CI, pipes) bypass the prompt automatically for v0.4 back-compat.

```
Usage: git-paw stop [OPTIONS]

Options:
      --force  Skip confirmation prompt
  -h, --help   Print help
```

**Examples:**
```bash
git paw stop          # prompts in TTY, bypasses in non-TTY
git paw stop --force  # always bypasses the prompt
```

When the session is currently paused, the confirmation prompt additionally warns that continuing will kill the still-running CLIs.

## `git paw purge`

Nuclear option: kills the tmux session, removes all worktrees, and deletes session state. When the broker was enabled, also removes `broker.log`. Requires confirmation unless `--force` is used.

```
Usage: git-paw purge [OPTIONS]

Options:
      --force  Skip confirmation prompt
  -h, --help   Print help
```

**Examples:**
```bash
git paw purge
git paw purge --force
```

## `git paw status`

Displays the current session status, branches, CLIs, and worktree paths for the repository in the current directory. When the broker is enabled, also shows the broker URL and connected agent count.

```
Usage: git-paw status

Options:
  -h, --help  Print help
```

**Example:**
```bash
git paw status
```

## `git paw list-clis`

Shows all AI CLIs found on PATH (auto-detected) and any custom CLIs registered in your config.

```
Usage: git-paw list-clis

Options:
  -h, --help  Print help
```

**Example:**
```bash
git paw list-clis
```

## `git paw add-cli`

Adds a custom CLI to your global config (`~/.config/git-paw/config.toml`). The command can be an absolute path or a binary name on PATH.

```
Usage: git-paw add-cli [OPTIONS] <NAME> <COMMAND>

Arguments:
  <NAME>     Name to register the CLI as
  <COMMAND>  Command or path to the CLI binary

Options:
      --display-name <DISPLAY_NAME>  Display name shown in prompts
  -h, --help                         Print help
```

**Examples:**
```bash
git paw add-cli my-agent /usr/local/bin/my-agent
git paw add-cli my-agent my-agent --display-name "My Agent"
```

## `git paw remove-cli`

Removes a custom CLI from your global config. Only custom CLIs can be removed — auto-detected CLIs cannot.

```
Usage: git-paw remove-cli <NAME>

Arguments:
  <NAME>  Name of the custom CLI to remove

Options:
  -h, --help  Print help
```

**Example:**
```bash
git paw remove-cli my-agent
```

## `git paw replay`

Replay captured session logs. Requires [logging](user-guide/session-logging.md) to be enabled.

```
Usage: git-paw replay [OPTIONS] [BRANCH]

Arguments:
  [BRANCH]  Branch to replay (fuzzy-matched against log filenames)

Options:
      --list              List available log sessions and branches
      --color             Display with colors via less -R
      --session <SESSION> Session to replay from (defaults to most recent)
  -h, --help              Print help
```

**Examples:**
```bash
# List all logged sessions and branches
git paw replay --list

# Replay a branch (stripped of ANSI codes)
git paw replay feat/add-auth

# Replay with colors
git paw replay feat/add-auth --color

# Replay from a specific session
git paw replay feat/add-auth --session paw-my-project
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Error (git, tmux, config, or other failure) |
| 2 | User cancelled (Ctrl+C or empty selection) |
