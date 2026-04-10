## Context

This is the final integration change for v0.3.0. All the pieces exist as standalone modules — `start_broker`, `run_dashboard`, `BrokerState`, skill templates, delivery logic — and this change connects them to the session lifecycle that `git paw start`/`stop`/`purge`/`status` manage.

The session launch flow in v0.2.0 is roughly:
1. Resolve branches (interactive or `--from-specs` or `--branches`)
2. Create worktrees
3. Generate per-worktree AGENTS.md (in `agents.rs`)
4. Build a `TmuxSessionBuilder` with one pane per worktree
5. Execute the tmux session
6. Save session state
7. Attach

This change adds a conditional broker path between steps 4 and 5: if `[broker] enabled = true`, pane 0 runs the dashboard+broker process instead of a coding CLI, and the broker URL is injected into the tmux environment.

## Goals / Non-Goals

**Goals:**

- Wire the broker+dashboard into pane 0 when `[broker] enabled = true`
- Inject `GIT_PAW_BROKER_URL` into the tmux session environment so all panes inherit it
- Persist broker config in session state for recovery and status display
- Ensure `git paw stop` → tmux kill → pane 0 exits → `BrokerHandle` drops → broker shuts down (zero additional stop logic)
- Ensure `git paw purge` cleans up `broker.log`
- Show broker state in `git paw status` output

**Non-Goals:**

- Starting the broker as a separate daemon or background process. It runs inside pane 0.
- Broker recovery on reattach — if the session is alive and pane 0 is running, the broker is running. If pane 0 died, the session is degraded; `git paw status` detects this but recovery requires a new `git paw start`.
- Configuring the broker from the CLI (flags like `--broker-port`). All broker config comes from `.git-paw/config.toml`.
- Dashboard-specific CLI flags. The `__dashboard` subcommand is hidden and accepts no user-facing flags — it reads config from the same sources the main process does.

## Decisions

### Decision 1: Hidden `__dashboard` subcommand instead of a separate binary

Pane 0 runs `git paw __dashboard`. This is a clap subcommand marked with `#[clap(hide = true)]` so it doesn't appear in `--help`.

```rust
#[derive(Subcommand)]
enum Command {
    // ... existing commands ...

    /// Internal: runs broker + dashboard in pane 0 (hidden from --help)
    #[clap(hide = true)]
    #[clap(name = "__dashboard")]
    Dashboard,
}
```

When dispatched, `main.rs` handles it:
```rust
Command::Dashboard => {
    let config = load_config()?;
    let broker_config = config.broker;
    let log_path = session_state_dir()?.join("broker.log");
    let state = BrokerState::new_with_log_path(Some(log_path));
    let handle = start_broker(broker_config, state.clone())?;
    run_dashboard(state, handle)?;
    Ok(())
}
```

**Why:**
- Single binary. No cargo workspace split, no separate install target, no PATH issues.
- `git paw __dashboard` is a valid shell command that tmux can execute via `send-keys`.
- The `__` prefix signals "internal, do not call directly" to any user who discovers it.
- The subcommand has full access to config, session state, and error handling — same as any other command.

**Alternatives considered:**
- *Separate binary `git-paw-dashboard`.* Requires a workspace or a second `[[bin]]` in Cargo.toml, complicates install, needs its own config loading. Rejected.
- *Inline the broker+dashboard startup in the `start` command's process.* Would mean the `start` command never returns (it blocks in `run_dashboard`). But `start` needs to build the tmux session and attach — it can't also be pane 0. Rejected.

### Decision 2: Pane 0 is conditionally the dashboard

The `TmuxSessionBuilder` in v0.2.0 creates pane 0 with the first worktree's CLI. When `[broker] enabled = true`, pane 0 is instead assigned to run `git paw __dashboard`:

```rust
if config.broker.enabled {
    // Pane 0: dashboard+broker
    builder.add_pane(PaneSpec {
        worktree: repo_root.to_path_buf(),  // dashboard runs from repo root
        command: format!("git paw __dashboard"),
        title: "dashboard".to_string(),
    });
}

// Remaining panes: coding agents (starting at pane 1 if broker enabled, pane 0 if not)
for worktree in &worktrees {
    builder.add_pane(PaneSpec {
        worktree: worktree.path.clone(),
        command: cli_command.clone(),
        title: format!("{} → {}", worktree.branch, cli_name),
    });
}
```

**Why:**
- The dashboard needs a dedicated pane with its own terminal (ratatui owns stdout in that pane)
- Pane 0 is the natural choice — it's the first thing the user sees, and the MILESTONE.md mockup puts it there
- When broker is disabled, pane 0 remains a normal coding agent — no behavior change for users who don't opt in

### Decision 3: Environment injection via `tmux set-environment`

Before executing the tmux session, the builder calls:

```
tmux set-environment -t <session-name> GIT_PAW_BROKER_URL http://127.0.0.1:9119
```

This sets a session-level environment variable that all panes in the session inherit. Unlike `send-keys "export ..."`, `set-environment` works before any pane shell starts and doesn't depend on the shell being bash/zsh.

**Why:**
- `set-environment` is the tmux-native way to inject env vars into a session
- All panes (including newly split ones) inherit session-level environment
- No shell dependency — works with any shell the agent CLI uses
- The value comes from `BrokerConfig::url()` which is already implemented by `http-broker`

**Why not `send-keys "export GIT_PAW_BROKER_URL=..."` per pane:**
- Shell-specific (`export` is bash/zsh; `setenv` is csh/fish)
- Races with the CLI launch command in the same pane
- Need to send it before the CLI command, complicating the builder's command ordering

### Decision 4: Session state gains optional broker fields

The `SessionData` struct (serialized to `~/.local/share/git-paw/sessions/<name>.json`) gains three new optional fields:

```rust
pub struct SessionData {
    // ... existing fields ...

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub broker_port: Option<u16>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub broker_bind: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub broker_log_path: Option<PathBuf>,
}
```

**Why:**
- `git paw status` needs to know the broker port to display it and optionally probe `/status`
- The log path is needed by `git paw purge` to clean up `broker.log`
- `serde(default)` ensures existing v0.2.0 session files load without error — missing fields default to `None`
- `skip_serializing_if = "Option::is_none"` keeps the JSON clean for non-broker sessions

### Decision 5: `stop` requires no additional broker logic

The shutdown path is:
1. `git paw stop` calls `tmux kill-session -t <session-name>`
2. tmux kills all panes, including pane 0
3. Pane 0's process (`git paw __dashboard`) receives SIGHUP (tmux default) or is killed
4. `run_dashboard` exits (either via signal or loop break)
5. `BrokerHandle` is dropped (it was moved into `run_dashboard`)
6. `BrokerHandle::drop` signals the flush thread, joins it, then drops the tokio runtime
7. Broker is cleanly shut down

No code change needed in the `stop` handler — it already calls `tmux kill-session`. The key insight is that `BrokerHandle` ownership flow (`main` → `run_dashboard` → dropped on exit) guarantees cleanup.

**Testing note:** need an integration test that starts a session with broker, calls `stop`, and verifies the broker port is freed.

### Decision 6: `purge` cleans up broker.log

The `purge` handler already removes worktrees, kills tmux, and deletes session state. This change adds one step: if the session state has a `broker_log_path`, delete that file too.

```rust
if let Some(log_path) = &session.broker_log_path {
    let _ = std::fs::remove_file(log_path);  // best-effort, ignore errors
}
```

**Why best-effort:** the log file may have already been deleted, or may never have been created (broker crashed before first flush). Either case is fine — purge is idempotent.

### Decision 7: `status` probes the broker when possible

When displaying session status with broker enabled, `git paw status` shows:

```
Session: paw-my-project (active)
Broker:  http://127.0.0.1:9119 (running, 3 agents)
Panes:   4 (1 dashboard + 3 agents)
```

The broker state is obtained by probing `GET /status` against `broker_bind:broker_port` from the session state file. If the probe fails (broker crashed, pane 0 dead), display `(not responding)` instead.

**Why probe instead of just showing session state fields:**
- The probe gives live data (agent count, uptime) instead of stale config
- It reuses the same probe logic from the stale-broker detection in `start_broker`
- Fallback to session state data if the probe fails — still show the port even if the broker is down

## Risks / Trade-offs

- **Pane 0 is not a coding agent when broker is enabled** → Users who had 4 agents now get 3 agents + 1 dashboard. The total pane count increases by 1. **Mitigation:** document clearly that broker mode adds a dashboard pane. The user's branch count determines agent count, not pane count.

- **`__dashboard` subcommand discoverability** → A user could accidentally run `git paw __dashboard` outside of tmux. It would try to start a broker and dashboard in their current terminal. **Mitigation:** detect when not running inside a tmux pane (check `$TMUX` env var) and refuse with a clear error: "this command is internal and should only be run by git-paw inside tmux."

- **SIGHUP behavior on tmux kill** → tmux sends SIGHUP to pane processes when the session is killed. The dashboard's SIGINT trap doesn't cover SIGHUP. If SIGHUP terminates the process before `BrokerHandle::drop` runs, the final flush is lost. **Mitigation:** either trap SIGHUP too (simple — add it alongside SIGINT), or accept losing ~5 seconds of broker log on `git paw stop`. Lean toward trapping SIGHUP.

- **Session state file backward compatibility** → Adding optional fields to `SessionData` with `serde(default)` is non-breaking for deserialization. But older git-paw binaries reading a new-format session file will silently ignore the broker fields. This is acceptable — older binaries don't know about the broker anyway.

- **`tmux set-environment` timing** → The env var must be set before panes start reading it (i.e., before `send-keys` launches CLIs in each pane). The builder should emit `set-environment` before any `send-keys` commands. Verify in the command ordering tests.

## Migration Plan

No data migration. Existing sessions without `[broker]` config continue to work unchanged — `broker.enabled` defaults to `false`, no dashboard pane is created, no env var is injected, session state has no broker fields.

For users upgrading from v0.2.0:
1. Add `[broker]\nenabled = true` to `.git-paw/config.toml`
2. Next `git paw start` creates a session with the dashboard in pane 0
3. No changes to existing worktrees, branches, or CLIs
