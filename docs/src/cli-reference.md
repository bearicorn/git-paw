# CLI Reference

git-paw is invoked as `git paw` (or `git-paw`). Below is the reference for all subcommands and flags.

## `git paw`

Running with no subcommand is equivalent to `git paw start`.

```
Parallel AI Worktrees — orchestrate multiple AI coding CLI sessions across git worktrees

Usage: git-paw [COMMAND]

Commands:
  start       Launch a new session or reattach to an existing one
  stop        Stop the session (kills tmux, keeps worktrees and state)
  purge       Remove everything (tmux session, worktrees, and state)
  status      Show session state for the current repo
  list-clis   List detected and custom AI CLIs
  add-cli     Register a custom AI CLI
  remove-cli  Unregister a custom AI CLI
  help        Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

## `git paw start`

Smart start: reattaches if a session is active, recovers if stopped/crashed, or launches a new interactive session.

```
Usage: git-paw start [OPTIONS]

Options:
      --cli <CLI>              AI CLI to use (skips CLI picker)
      --branches <BRANCHES>    Comma-separated branches (skips branch picker)
      --dry-run                Preview the session plan without executing
      --preset <PRESET>        Use a named preset from config
  -h, --help                   Print help
```

**Examples:**
```bash
git paw start
git paw start --cli claude
git paw start --cli claude --branches feat/auth,feat/api
git paw start --dry-run
git paw start --preset backend
```

## `git paw stop`

Kills the tmux session but preserves worktrees and session state on disk. Run `git paw start` later to recover the session.

```
Usage: git-paw stop

Options:
  -h, --help  Print help
```

**Example:**
```bash
git paw stop
```

## `git paw purge`

Nuclear option: kills the tmux session, removes all worktrees, and deletes session state. Requires confirmation unless `--force` is used.

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

Displays the current session status, branches, CLIs, and worktree paths for the repository in the current directory.

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

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Error (git, tmux, config, or other failure) |
| 2 | User cancelled (Ctrl+C or empty selection) |
