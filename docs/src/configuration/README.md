# Configuration

git-paw uses TOML configuration files at two levels, with repo-level settings overriding global ones.

## Config File Locations

| Level | Path | Purpose |
|-------|------|---------|
| Global | `~/.config/git-paw/config.toml` | Default CLI, custom CLIs, global presets |
| Per-repo | `.git-paw/config.toml` (in repo root) | Repo-specific overrides |

Both files are optional. git-paw works with sensible defaults when no config exists.

## Full Config Example

```toml
# Default CLI used when --cli flag is not provided
default_cli = "claude"

# Enable mouse mode in tmux sessions (default: true)
mouse = true

# Custom CLI definitions
[clis.my-agent]
command = "/usr/local/bin/my-agent"
display_name = "My Agent"

[clis.local-llm]
command = "ollama-code"
display_name = "Local LLM"

# Named presets for quick launch
[presets.backend]
branches = ["feature/api", "fix/db"]
cli = "claude"

[presets.frontend]
branches = ["feature/ui", "feature/styles"]
cli = "codex"
```

## Settings Reference

### `default_cli`

The AI CLI to use when `--cli` is not passed and you want to skip the CLI picker.

```toml
default_cli = "claude"
```

### `mouse`

Enable or disable tmux mouse mode for git-paw sessions. When enabled, you can click panes to switch, drag borders to resize, and scroll with the mouse wheel. This is set per-session and does not affect your other tmux sessions.

```toml
mouse = true  # default
```

## Custom CLIs

Register custom AI CLIs that aren't in git-paw's built-in detection list.

### Via config file

```toml
[clis.my-agent]
command = "/usr/local/bin/my-agent"   # absolute path
display_name = "My Agent"              # optional, shown in prompts

[clis.local-llm]
command = "ollama-code"               # binary name (resolved via PATH)
display_name = "Local LLM"
```

### Via command line

```bash
# Add with absolute path
git paw add-cli my-agent /usr/local/bin/my-agent

# Add with binary name on PATH
git paw add-cli my-agent my-agent

# Add with display name
git paw add-cli my-agent my-agent --display-name "My Agent"

# Remove
git paw remove-cli my-agent
```

The `add-cli` and `remove-cli` commands modify the **global** config at `~/.config/git-paw/config.toml`.

### Listing CLIs

```bash
git paw list-clis
```

Shows both auto-detected and custom CLIs with their source:

```
Name       Path                         Source
claude     /usr/local/bin/claude        detected
codex      /usr/local/bin/codex         detected
my-agent   /usr/local/bin/my-agent      custom
```

## Presets

Presets save branch + CLI combinations for one-command launch.

### Defining presets

```toml
[presets.backend]
branches = ["feature/api", "fix/db-migration"]
cli = "claude"

[presets.full-stack]
branches = ["feature/api", "feature/ui", "feature/styles"]
cli = "gemini"
```

### Using presets

```bash
git paw start --preset backend
```

This skips all interactive prompts and launches with the preset's branches and CLI.

## Merging Rules

When both global and repo configs exist, they merge with these rules:

| Field | Merge behavior |
|-------|---------------|
| `default_cli` | Repo wins |
| `mouse` | Repo wins |
| `clis` | Maps merge (repo overrides per-key) |
| `presets` | Maps merge (repo overrides per-key) |

**Example:** If global config defines `[clis.my-agent]` and repo config defines `[clis.my-agent]` with a different command, the repo version wins. But a `[clis.other-tool]` in global config still appears — maps are merged, not replaced.

## Graceful Absence

If no config files exist, git-paw uses defaults:
- No default CLI (prompts for selection)
- Mouse mode enabled
- No custom CLIs
- No presets
