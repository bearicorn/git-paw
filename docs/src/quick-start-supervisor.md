# Quick Start: Supervisor Mode

This walkthrough shows how to launch git-paw in **supervisor mode** — git-paw's hands-off, spec-driven workflow where a supervisor agent orchestrates several coding agents in parallel, runs tests between merges, and writes a final session report.

## What Supervisor Mode Does

Supervisor mode (`git paw start --supervisor`) launches every pending spec as its own coding agent in a background tmux pane and gives you an interactive supervisor agent in the foreground pane. The supervisor automatically launches all agents per spec, runs the configured `test_command` between merges, walks branches in topological order based on declared dependencies, writes a `.git-paw/session-summary.md` when the run is complete, and auto-approves common safe permission prompts (e.g. `cargo test`, `git commit`, broker `curl` calls) so the run can proceed unattended.

## Prerequisites

- The standard git-paw [prerequisites](quick-start-same-cli.md#prerequisites) (git-paw installed, tmux, an AI CLI on `PATH`, a git repo).
- `[broker] enabled = true` in your config — supervisor mode requires the coordination broker.
- A `[supervisor]` section is recommended but optional. If it is missing, git-paw prompts for the supervisor CLI, the test command, and the agent approval level on first run.
- A specs directory configured via `[specs]` (default: `specs/`) containing at least one pending spec.

## Smallest Runnable Example

### 1. Create one OpenSpec-format spec

```bash
mkdir -p specs/add-greeting
cat > specs/add-greeting/tasks.md <<'MD'
## Add a greeting helper

- [ ] Add `pub fn greet(name: &str) -> String` to `src/lib.rs`.
- [ ] Add a unit test asserting `greet("paw") == "hello, paw"`.
- [ ] Run `cargo test` and make sure it passes.
MD
```

### 2. Configure broker + supervisor

`.git-paw/config.toml`:

```toml
[broker]
enabled = true
port = 9119

[specs]
dir = "specs"
type = "openspec"

[supervisor]
enabled = true
cli = "claude"
test_command = "cargo test"
agent_approval = "auto"
```

### 3. Launch

```bash
git paw start --supervisor
```

git-paw creates a worktree per spec, opens a tmux session with the dashboard in pane 0, the supervisor in pane 1, and one coding agent per spec in the remaining panes. The supervisor injects its boot prompt automatically; you only need to chat with it for high-level approvals.

When every agent has reported `verified` and the supervisor has merged each branch in topological order, git-paw writes `.git-paw/session-summary.md` and exits.

## Key Config Knobs

The full reference lives in [Configuration → Supervisor](configuration/README.md#supervisor). The fields you will reach for most often:

| Field | What it controls |
|-------|------------------|
| `[supervisor].enabled` | Default to supervisor mode without passing `--supervisor`. |
| `[supervisor].cli` | Which AI CLI runs as the supervisor (falls back to `default_cli`). |
| `[supervisor].test_command` | Command run after each agent reports `done` and again after every merge. |
| `[supervisor].agent_approval` | `"manual"`, `"auto"`, or `"full-auto"` — translates into CLI permission flags. |
| `[supervisor.auto_approve].enabled` | Master switch for git-paw's safe-prompt auto-dismisser. |
| `[supervisor.auto_approve].safe_commands` | Project-specific command prefixes appended to the built-in safe list. |
| `[supervisor.auto_approve].approval_level` | `"off"`, `"conservative"`, or `"safe"` preset for the auto-approve whitelist. |

See [Configuration → Broker](configuration/README.md#broker) for `[broker]` settings and [Configuration → Dashboard](configuration/README.md#dashboard) if you want the live broker-message panel turned on.

## What's NOT Yet Supported in v0.4.0

A few items in the supervisor roadmap explicitly do **not** ship in v0.4.0. See [`MILESTONE.md`](https://github.com/bearicorn/git-paw/blob/main/MILESTONE.md) for the authoritative list.

- **Per-CLI hook providers** (e.g. `.claude/hooks.json`, `.gemini/settings.json`, `.codex/hooks.json`). v0.4.0 ships a single CLI-agnostic auto-approve mechanism that works against any CLI via `tmux capture-pane` + `tmux send-keys`. Per-CLI hook providers are deferred to v1.0.0 (see `MILESTONE.md` → "v0.4.0 Deviations from this Milestone" → "Scope reductions / deferrals").
- **Cross-agent conflict detection** (warn when two agents modify the same file). Deferred to v0.5.0 as part of Learnings Mode (see `MILESTONE.md` → "v0.5.0 — Supervisor Learnings + Governance Docs").
- **Learnings mode** — supervisor tracking of stuck patterns, ownership violations, and ADR / policy suggestions. Ships in v0.5.0 (see `MILESTONE.md` → "Feature: Learnings Mode").

## Where to Look When Things Go Wrong

When a supervisor run misbehaves, three places have everything you need:

1. **Dashboard pane (pane 0).** With `[broker] enabled = true` the dashboard runs in pane 0 and updates every second. Use it to spot which agent is `blocked`, which is `working`, and how long it has been since each agent's last status update. See the [Dashboard chapter](user-guide/dashboard.md) for status-symbol meanings and controls.
2. **`.git-paw/session-summary.md`.** Written by the supervisor when the run exits. Contains per-branch test results, merge order, and any agents that were skipped. If the supervisor exited early, this file tells you why.
3. **`.git-paw/broker.log`.** A JSONL audit trail of every message the broker handled — `agent.register`, `agent.done`, `agent.blocked`, `agent.verified`, `agent.feedback`, etc. Tail it live with `tail -f .git-paw/broker.log` to watch coordination as it happens. The file is flushed every five seconds; see [Agent Coordination → Audit Trail](user-guide/coordination.md) for the full schema.

If you need to inspect an individual agent's pane history, run `tmux capture-pane -p -t <session>:<window>.<pane>` against the session printed at launch.

## What's Next

- [User Guide → Agent Coordination](user-guide/coordination.md) — broker message types and how agents talk to each other.
- [User Guide → Dashboard](user-guide/dashboard.md) — pane 0 status table, controls, and configuration.
- [Configuration → Supervisor](configuration/README.md#supervisor) — every supervisor and `auto_approve` field with defaults.
