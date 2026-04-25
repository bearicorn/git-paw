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
default_cli = "my-cli"

# Default CLI for --from-specs mode (bypasses picker when set)
# default_spec_cli = "my-cli"

# Prefix for spec-derived branch names (default: "spec/")
# branch_prefix = "spec/"

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

# Spec scanning configuration
# [specs]
# dir = "specs"
# type = "openspec"    # "openspec" or "markdown"

# Session logging
# [logging]
# enabled = false

# Agent coordination broker
# [broker]
# enabled = false
# port = 9119
# bind = "127.0.0.1"
```

## Settings Reference

### `default_cli`

The AI CLI to use when `--cli` is not passed and you want to skip the CLI picker.

```toml
default_cli = "my-cli"
```

### `default_spec_cli`

The AI CLI to use by default when launching with `--from-specs`. When set, skips the CLI picker for any specs that don't have a `paw_cli` override.

```toml
default_spec_cli = "my-cli"
```

See [Spec-Driven Launch](../user-guide/spec-driven-launch.md) for the full CLI resolution chain.

### `branch_prefix`

Prefix prepended to spec-derived branch names. Defaults to `"spec/"`.

```toml
branch_prefix = "spec/"
```

For example, a spec with ID `add-auth` produces branch `spec/add-auth`.

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

## Specs

Configure spec file scanning for `--from-specs` mode.

```toml
[specs]
dir = "specs"         # Directory containing spec files (relative to repo root)
type = "openspec"     # "openspec" (directory-based) or "markdown" (file-based)
```

| Field | Default | Description |
|-------|---------|-------------|
| `dir` | `"specs"` | Directory to scan for spec files |
| `type` | `"openspec"` | Spec format: `"openspec"` for directory-based, `"markdown"` for file-based |

See [Spec-Driven Launch](../user-guide/spec-driven-launch.md) for format details.

## Logging

Configure session output logging.

```toml
[logging]
enabled = true
```

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | `false` | Whether to capture pane output to log files |

When enabled, logs are written to `.git-paw/logs/<session>/` using `tmux pipe-pane`. See [Session Logging](../user-guide/session-logging.md) for details.

## Broker

Configure the HTTP broker for agent coordination. When enabled, git-paw starts a lightweight HTTP server that lets agents share status updates, artifacts, and blocked requests.

```toml
[broker]
enabled = true
port = 9119
bind = "127.0.0.1"
```

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | `false` | Whether to start the coordination broker |
| `port` | `9119` | HTTP port for the broker server |
| `bind` | `"127.0.0.1"` | Bind address -- never bind to `0.0.0.0` on shared machines |

When the broker is enabled, git-paw injects the `GIT_PAW_BROKER_URL` environment variable into each agent pane, pointing to `http://<bind>:<port>`. Agents use this URL to communicate with the broker.

### Multi-repo port assignment

If you run git-paw sessions for multiple repositories at the same time, each session needs a different port. Set a unique `port` in each repo's `.git-paw/config.toml`:

```toml
# Repo A
[broker]
enabled = true
port = 9119

# Repo B (in its own .git-paw/config.toml)
[broker]
enabled = true
port = 9120
```

See [Agent Coordination](../user-guide/coordination.md) for usage details.

## Supervisor

Configure the supervisor agent for orchestrating parallel coding sessions. When enabled, the supervisor monitors agents, runs tests, verifies work, and coordinates merges.

```toml
[supervisor]
enabled = true
cli = "claude"
test_command = "just check"
agent_approval = "auto"
```

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | `false` | Whether to use supervisor mode by default (can also use `--supervisor` flag) |
| `cli` | (uses `default_cli`) | CLI binary for the supervisor agent |
| `test_command` | (none) | Command to run after an agent reports done (e.g. `"just check"`, `"cargo test"`) |
| `agent_approval` | `"auto"` | Permission level for coding agents: `"manual"`, `"auto"`, or `"full-auto"` |

**Approval levels:**

| Level | Behavior |
|-------|----------|
| `manual` | Agents prompt for every action (safest, slowest) |
| `auto` | CLI default behavior — some prompts, some auto-approved |
| `full-auto` | Skip all permission prompts (fastest, agents run unattended) |

The supervisor translates the approval level into CLI-specific flags at launch (e.g. `--dangerously-skip-permissions` for Claude in full-auto mode).

### Auto-approve safe permission prompts

When supervisor mode is enabled, git-paw can automatically approve common,
known-safe permission prompts (`cargo test`, `git commit`, broker `curl` calls, etc.)
in stalled agent panes so the supervisor does not have to dismiss every prompt by hand.

```toml
[supervisor.auto_approve]
enabled = true
safe_commands = ["just lint", "just test"]
stall_threshold_seconds = 30
approval_level = "safe"
```

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | `true` | Master switch for auto-approval. Set to `false` to disable. |
| `safe_commands` | `[]` | Project-specific command prefixes appended to the built-in defaults. |
| `stall_threshold_seconds` | `30` | Seconds an agent's `last_seen` must lag before its pane is polled (minimum `5`). |
| `approval_level` | `"safe"` | Coarse preset: `"off"`, `"conservative"`, or `"safe"`. |

**Built-in safe commands:** `cargo fmt`, `cargo clippy`, `cargo test`, `cargo build`,
`git commit`, `git push`, `curl http://127.0.0.1:`.

**Approval-level presets:**

| Preset | Behavior |
|--------|----------|
| `off` | Forces `enabled = false`. No detection or approval runs. |
| `conservative` | Drops `git push` and `curl` from the effective whitelist. |
| `safe` (default) | Approve every entry in the built-in whitelist plus configured extras. |

**How it works:** when an agent's status is non-terminal (`done`, `verified`, `blocked`,
`committed` are skipped) and its `last_seen` exceeds the threshold, git-paw runs
`tmux capture-pane`, classifies the pending command, and either dispatches
`BTab Down Enter` (if safe) or publishes an `agent.question` to the supervisor inbox
(if not).

git-paw also seeds `.claude/settings.json::allowed_bash_prefixes` with the broker
endpoints (`/publish`, `/status`, `/poll`, `/feedback`) so the first broker call
never hits a permission prompt. Existing entries in that file are preserved.

## Dashboard

Configure the dashboard TUI rendered in pane 0 when the broker is enabled.

```toml
[dashboard]
show_message_log = true
```

| Field | Default | Description |
|-------|---------|-------------|
| `show_message_log` | `false` | When `true`, the dashboard renders a scrolling broker-message panel above the prompts section. Useful for watching live agent traffic; leave `false` for a more compact layout. |

See [Dashboard](../user-guide/dashboard.md) for details.

### Multi-repo configuration

Each repository can have its own dashboard settings in `.git-paw/config.toml`. The repo-level config overrides the global config.

## Merging Rules

When both global and repo configs exist, they merge with these rules:

| Field | Merge behavior |
|-------|---------------|
| `default_cli` | Repo wins |
| `default_spec_cli` | Repo wins |
| `branch_prefix` | Repo wins |
| `mouse` | Repo wins |
| `clis` | Maps merge (repo overrides per-key) |
| `presets` | Maps merge (repo overrides per-key) |
| `specs` | Repo wins |
| `logging` | Repo wins |
| `broker` | Repo wins |
| `supervisor` | Repo wins |
| `dashboard` | Repo wins |

**Example:** If global config defines `[clis.my-agent]` and repo config defines `[clis.my-agent]` with a different command, the repo version wins. But a `[clis.other-tool]` in global config still appears — maps are merged, not replaced.

## Graceful Absence

If no config files exist, git-paw uses defaults:
- No default CLI (prompts for selection)
- Mouse mode enabled
- No custom CLIs
- No presets
