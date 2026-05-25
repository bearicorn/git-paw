# Dashboard

When the broker is enabled, one tmux pane runs the dashboard instead of an
agent CLI. In **supervisor mode** the dashboard lives at **pane 1** (pane 0
hosts the supervisor CLI itself); in **broker-only mode** (broker on,
supervisor off) the dashboard lives at **pane 0**. Either way it renders the
same live status table that updates every second, giving you an at-a-glance
view of what each agent is doing.

The dashboard is **observation-only** — the only keystroke it handles is `q` to quit. Human input (questions, directives, replies to `agent.question` events) happens in the supervisor pane itself; the dashboard simply renders broker state.

## What the Dashboard Pane Shows

The dashboard renders a table with one row per agent. When a supervisor pane is running, its row is pinned to the top with a horizontal-line divider beneath it; coding-agent rows follow in alphabetical order:

```
┌────────────┬────────┬────────────┬─────────┬──────────────────────────────┐
│ Agent      │ CLI    │ Status     │ Time    │ Summary                      │
├────────────┼────────┼────────────┼─────────┼──────────────────────────────┤
│ supervisor │ claude │ 🔵 watching│ 12s ago │ session online               │
│ ────────── │ ────── │ ────────── │ ─────── │ ──────────────────────────── │
│ feat-api   │ claude │ 🟡 blocked │ 1m 05s  │ waiting for auth token format│
│ feat-auth  │ claude │ 🔵 working │ 3m 22s  │ implementing login endpoint  │
│ fix-typo   │ gemini │ 🟢 done    │ 8m 41s  │ done — all typos fixed       │
└────────────┴────────┴────────────┴─────────┴──────────────────────────────┘
```

When no supervisor pane is running (e.g. `--no-broker` is not in play but no `--supervisor` was passed, or during the boot window before the supervisor has finished self-registering), the divider is not rendered and coding agents fill rows alphabetically from the top.

### Status Symbols

| Symbol | Meaning |
|--------|---------|
| 🔵 | Working -- agent is actively processing |
| 🟢 | Done/verified -- agent has completed its task |
| 🟣 | Committed -- agent has committed work |
| 🟡 | Blocked -- agent is waiting on something |
| ⚪ | Idle / unknown phase -- agent has not reported a recognised status |

The **Time** column shows elapsed time since the agent's last status update. The **Summary** column shows the most recent status or blocked message body.

### Supervisor row, `cli` field, and `phase` field

The supervisor row's `Status` column does **not** show the wire-message type label that a coding-agent row shows. Instead the supervisor publishes a `phase` field on its `agent.status` messages (e.g. `baseline`, `watching`, `approving`, `answering`, `merging`, `summary`), and the dashboard prefers that label when rendering its row. This avoids the misleading `status=feedback` label the supervisor would otherwise show when it publishes `agent.feedback` to a coding agent.

The supervisor pane is also not a watch target, so the broker cannot infer its `cli` from the watch-target map. To populate the `CLI` column for the supervisor row, the supervisor self-registration `agent.status` includes a `cli` field (e.g. `"cli":"claude"`). The broker upserts that value into its internal CLI map when it receives the message. Coding agents do not need to publish `cli` — the broker populates their CLI from the watch-target map at startup.

### When the supervisor row appears

The supervisor row appears **after** the supervisor pane's CLI has booted and published its first self-registration `agent.status` — typically within 3-5 seconds of `git paw start --supervisor` returning. There is no phantom supervisor row at launch time; if the supervisor pane fails to start, the row simply never appears. Aborted launches (non-TTY skip, missing CLI on PATH, system-level pane spawn failure) leave the agent table free of a misleading supervisor entry.

## Controls

Press `q` to quit the dashboard. This shuts down the broker and terminates the dashboard process in the dashboard pane. The agent panes continue running -- they simply lose the ability to communicate via the broker.

## Broker Messages Panel

When enabled, the dashboard shows a broker messages panel at the bottom, displaying recent communication between agents and the broker for at-a-glance observability.

### Enabling the Panel

Add this to your `.git-paw/config.toml`:

```toml
[dashboard]
show_message_log = true
```

### Message Types

The panel shows six types of broker messages:

| Symbol | Type | Meaning |
|--------|------|---------|
| 📤 | Status | Agent status updates |
| 📦 | Artifact | Shared files/artifacts |
| 🚧 | Blocked | Agent blocked requests |
| ✅ | Verified | Supervisor verification |
| 💬 | Feedback | Supervisor feedback |
| ❓ | Question | Agent questions |

### Example Layout

```
┌──────────┬────────┬────────┬─────────┬──────────────────────────────┐
│ Agent    │ CLI    │ Status │ Time    │ Summary                      │
├──────────┼────────┼────────┼─────────┼──────────────────────────────┤
│ feat/auth│ claude │ 🔵     │ 3m 22s  │ implementing login endpoint  │
│ feat/api │ claude │ 🟡     │ 1m 05s  │ waiting for auth token format│
│ fix/typo │ gemini │ 🟢     │ 8m 41s  │ done — all typos fixed       │
└──────────┴────────┴────────┴─────────┴──────────────────────────────┘

[14:30:22] agent-0 📤 working on login endpoint
[14:29:45] agent-1 📦 shared auth_schema.json
[14:28:10] agent-2 🚧 blocked: need API spec format
```

Each message shows timestamp (HH:MM:SS), agent ID, message type symbol, and content. The panel shows the 20 most recent messages.

## Relationship to the Broker

The dashboard and broker run in the same process (`git paw __dashboard`). The dashboard reads from shared state that the broker's HTTP handlers (and the watcher, conflict detector, and learnings aggregator subsystems) write to. There is no separate broker process to manage.

## Replying to agent questions

Earlier dashboard versions included a "Questions" panel and a "Reply to" input field for human-typed answers to `agent.question` events. The panel was removed in v0.5.0 because the supervisor pane is the natural input surface — typed questions and replies go through `tmux send-keys` and the supervisor agent's own curl machinery, not through the dashboard.

`agent.question` messages still flow through the broker. The supervisor pane polls the supervisor inbox, reads incoming questions, and replies via `tmux send-keys` to the asking agent's pane (and via `agent.feedback` to the broker for the audit log). See the [Supervisor mode](../quick-start-supervisor.md) chapter and the embedded `supervisor.md` skill for the reply flow.

See the [v0.5.0 changelog](../changelog.md) for the removal note.
