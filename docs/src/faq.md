# FAQ

## General

### What does "paw" stand for?

**P**arallel **A**I **W**orktrees.

### Does git-paw work on Windows?

Only through [WSL (Windows Subsystem for Linux)](https://learn.microsoft.com/en-us/windows/wsl/install). git-paw requires tmux, which is not natively available on Windows. See the [Installation](installation.md) chapter for WSL setup instructions.

### Do I need tmux experience to use git-paw?

No. git-paw creates and manages tmux sessions for you. Mouse mode is enabled by default, so you can click to switch panes and drag to resize. The only tmux shortcut you might need is `Ctrl-b d` to detach.

### Can I use git-paw with AI CLIs not in the supported list?

Yes! Use `git paw add-cli` to register any CLI binary. See [Custom CLIs](configuration/README.md#custom-clis).

## Sessions

### What happens if I close my terminal?

The tmux session keeps running in the background. Run `git paw attach` (or `git paw` again) to reattach.

### What happens if tmux crashes or my machine reboots?

git-paw saves session state to disk. The next time you run `git paw`, it detects the saved state and automatically recovers: reuses existing worktrees, recreates the tmux session, and relaunches your AI CLIs.

### Can I run multiple git-paw sessions?

One session per repository. To work with multiple repos, open separate terminals and run `git paw` in each repo directory.

### How do I switch between branches in a session?

Click the pane you want (mouse mode is on by default), or use `Ctrl-b` followed by arrow keys to navigate between panes. Each pane's border label shows its pane index and branch.

## Worktrees

### What are git worktrees?

Git worktrees let you check out multiple branches simultaneously in separate directories. Each worktree is a fully functional working copy of the repository sharing the same `.git` data. Changes in one worktree don't affect others.

### Where does git-paw create worktrees?

It depends on `worktree_placement`. `git paw init` writes `"child"`, which keeps worktrees inside the repo; with no `worktree_placement` set, they are created as siblings of the repo. For a project at `~/projects/my-app` with branch `feat/auth`:

```
~/projects/my-app/                                ← your repo
~/projects/my-app/.git-paw/worktrees/feat-auth/   ← "child" (written by git paw init)
~/projects/my-app-feat-auth/                      ← "sibling" (field absent)
```

See [Worktree Placement](user-guide/worktree-placement.md).

### Does stopping a session delete my worktrees?

No. `git paw stop` kills the tmux session but keeps worktrees and any uncommitted work intact. Only `git paw purge` (or `git paw remove <branch>` for a single agent) removes worktrees.

### Can I manually work in a git-paw worktree?

Yes. Worktrees are regular git working directories. You can `cd` into them, edit files, commit, push — anything you'd do in a normal repo. When you restart the session, git-paw reuses the existing worktrees.

## Configuration

### Where are config files stored?

| Level | Path |
|-------|------|
| Global | `~/.config/git-paw/config.toml` |
| Per-repo | `.git-paw/config.toml` (in repo root) |

Both are optional. See [Configuration](configuration/README.md).

### How do I set a default CLI?

Add to your global or repo config:

```toml
default_cli = "my-cli"
```

It pre-selects that CLI in the interactive picker; the picker still appears, so
you can pick another. Pass `--cli` to skip the prompt entirely.

### How do I disable mouse mode?

```toml
mouse = false
```

This only affects git-paw's tmux sessions, not your other tmux usage.

## Troubleshooting

### "Not a git repository"

Run git-paw from inside a git repository. It needs to be anywhere within a repo's working tree.

### "tmux is required but not installed"

Install tmux:
- macOS: `brew install tmux`
- Ubuntu/Debian: `sudo apt install tmux`
- Fedora: `sudo dnf install tmux`

### "No AI CLIs found on PATH"

Install at least one AI coding CLI (see [Supported AI CLIs](supported-clis.md)), or register a custom one:

```bash
git paw add-cli my-tool /path/to/my-tool
```

### "no space for new pane" in tmux

This can happen with many branches on a small terminal. Make your terminal window larger before launching, or select fewer branches. In supervisor mode, git-paw caps each agent row at five panes and rebalances every row to equal width, so panes stay as wide as the layout allows; the hard cap is 25 agents per session.

### Session state seems stale

git-paw checks tmux liveness to determine effective status. `git paw status` shows a session whose receipt claims it is active but whose tmux session is gone as 🔴 stale, and `git paw start` self-heals such a receipt before launching. To clear stale receipts without touching a live session:

```bash
git paw purge --stale
```

### Repository names with dots or spaces, and install paths with spaces

Supported. git-paw derives the tmux session name from your repository's
directory name, and it sanitizes that name to a valid tmux target: everything
outside `A-Z a-z 0-9 _ -` becomes `-`, runs collapse, and leading or trailing
runs are dropped. So a repo in `my.app` gets the session `paw-my-app`, and
`My Project` gets `paw-My-Project`. Sanitizing matters because tmux reads `.`
and `:` as its own window and pane separators — left alone, a dotted name made
tmux parse `paw-my.app` as session `paw-my` plus pane `app`, and pane commands
failed with `can't find pane`. A name already made only of safe characters is
used unchanged, so existing sessions keep the names they have.

Paths with spaces are handled too: the per-pane log path passed to tmux's
`pipe-pane` and the dashboard command typed into its pane are both
shell-quoted, so a repository under `/Users/My User/code` logs and launches
normally.
