# Customising Workers

By default a `claude`-family worker loads none of your personal Claude Code
setup: no user-level settings, hooks or plugins (see
[what a worker inherits](security-posture.md#what-a-worker-inherits-from-you)).
That keeps workers predictable, but some teams want workers to have a few
extensions of their own — a plugin, a user skill, a model provider in the
settings' `env` block.

The recommended route is a **worker config folder**: a separate config
directory for the agent CLI, used only by workers. You install there exactly
what workers should have, log in once, and point workers at it through a
wrapper CLI entry. Your own config directory stays out of the workers, and
the workers' stays out of your own sessions.

git-paw has no setting that picks individual plugins or skills from your own
config. Claude Code has no switch that hides only user-level skills, so a
separate folder is the only route that controls them exactly.

## What always applies

Whatever config folder a worker uses, git-paw keeps two **worker-safety**
settings on, because git-paw needs them to run workers safely:

| Variable | Why |
|---|---|
| `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1` | Claude Code shares one auto memory across every worktree of a repository; no worker writes it. |
| `CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false` | An idle worker's input box stays empty, so a lone Enter from git-paw (buffered-input recovery, `sweep.sh tell`, an approval) cannot submit a predicted message. |

The **operator-config** part of the isolation — `--setting-sources
project,local` and an emptied `CLAUDE_CODE_PLUGIN_DIRS` — is what
`inherit_operator_config = true` drops. With it set, the CLI's own config
folder decides which settings, plugins and skills load.

## Set up a claude worker folder

### 1. A wrapper script

Save this as `~/bin/claude-worker` (any directory on your `PATH` works) and
make it executable with `chmod +x ~/bin/claude-worker`:

```sh
#!/bin/sh
CLAUDE_CONFIG_DIR=~/.claude-worker exec claude "$@"
```

`exec` replaces the shell, so git-paw's environment prefix and arguments
reach Claude Code unchanged. Because the wrapper runs after git-paw's `env`
prefix, it can also decide `CLAUDE_CODE_PLUGIN_DIRS` for workers: add
`unset CLAUDE_CODE_PLUGIN_DIRS` before the `exec` line to keep plugins your
shell points at out of the workers, or set it to a directory of worker
plugins.

### 2. Log in once

Run the wrapper once from any directory and log in:

```bash
claude-worker
```

The login is stored in `~/.claude-worker` and is shared by every worker that
uses this entry, in every repository. You log in once per config folder, not
once per worker.

### 3. Install what workers should have

Everything the worker folder holds applies to workers only:

- **Plugins**: install them from a `claude-worker` session (`/plugin`), so
  they land in `~/.claude-worker`.
- **Skills**: put user-level skills in `~/.claude-worker/skills/<name>/`.
- **Settings**: put user-level settings, such as an `env` block for a proxy
  endpoint or model provider, in `~/.claude-worker/settings.json`.

Repository settings (`.claude/settings.json`, the main checkout's
`.claude/settings.local.json`), project skills and `CLAUDE.md`/`AGENTS.md`
apply as usual.

### 4. The `[clis.claude-worker]` entry

```toml
[clis.claude-worker]
command = "/home/you/bin/claude-worker"
family = "claude"
settings_path = "~/.claude-worker/settings.json"
inherit_operator_config = true
```

- `family = "claude"` gives the entry Claude Code's approval flags,
  session-id and resume templates, headless form and dialog handling.
- `settings_path` tells git-paw where the folder is, so
  [usage attribution](supervisor.md#usage-and-the-budget-guardrail) finds the
  workers' session reports there. git-paw never reads or writes the file.
- `inherit_operator_config = true` drops the operator-config isolation, so
  the worker folder's settings, plugins and skills load. Worker safety stays.

`git paw start --dry-run` prints the exact launch line, for example
`env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false /home/you/bin/claude-worker --permission-mode acceptEdits --session-id <id>`.

### 5. Point workers at it

Choose one:

- **Every worker by default**: `default_cli = "claude-worker"`.
- **Through a worker profile**, for example with a pinned model, made the
  default with `default_profile`:

  ```toml
  [profiles.custom]
  kind = "worker"
  description = "Workers with the team plugins"
  cli = "claude-worker"
  args = "--model sonnet"

  default_profile = "custom"
  ```

  See [Agent Profiles](agent-profiles.md) for the selection order.
- **For one launch**: `git paw start --cli claude-worker` or
  `git paw add <branch> --cli claude-worker` (or `--profile custom`).

## Codex (not yet verified)

> **Unverified.** This example has not been checked against a live Codex CLI
> yet; the v0.23.0 per-CLI audit verifies it. The `codex` family has no
> operator-isolation facts, so `inherit_operator_config` changes nothing for
> it today.

Codex keeps its config and login in `CODEX_HOME`. The same pattern applies:

```sh
#!/bin/sh
CODEX_HOME=~/.codex-worker exec codex "$@"
```

```toml
[clis.codex-worker]
command = "/home/you/bin/codex-worker"
family = "codex"
inherit_operator_config = true
```

Run `codex-worker` once to log in, put the workers' configuration in
`~/.codex-worker`, then select it with `default_cli`, a worker profile or
`--cli codex-worker` as above.
