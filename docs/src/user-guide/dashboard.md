# Dashboard

When the broker is enabled, pane 0 displays a live dashboard instead of running an agent CLI. The dashboard shows a status table that updates every second, giving you an at-a-glance view of what each agent is doing.

## What Pane 0 Shows

The dashboard renders a table with one row per agent:

```
┌──────────┬────────┬────────┬─────────┬──────────────────────────────┐
│ Agent    │ CLI    │ Status │ Time    │ Summary                      │
├──────────┼────────┼────────┼─────────┼──────────────────────────────┤
│ feat/auth│ claude │ 🔵     │ 3m 22s  │ implementing login endpoint  │
│ feat/api │ claude │ 🟡     │ 1m 05s  │ waiting for auth token format│
│ fix/typo │ gemini │ 🟢     │ 8m 41s  │ done — all typos fixed       │
└──────────┴────────┴────────┴─────────┴──────────────────────────────┘
```

### Status Symbols

| Symbol | Meaning |
|--------|---------|
| 🔵 | Working -- agent is actively processing |
| 🟢 | Done/verified -- agent has completed its task |
| 🟡 | Blocked -- agent is waiting on something |
| ⚪ | Idle -- agent has not reported status yet |

The **Time** column shows elapsed time since the agent's last status update. The **Summary** column shows the most recent status or blocked message body.

## Controls

Press `q` to quit the dashboard. This shuts down the broker and terminates the dashboard process in pane 0. The agent panes continue running -- they simply lose the ability to communicate via the broker.

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

The dashboard and broker run in the same process (`git paw __dashboard`). The dashboard reads from shared state that the broker's HTTP handlers write to. There is no separate broker process to manage.

## Future Plans

v0.4 will add an interactive prompt inbox to the dashboard, allowing you to respond to agent questions directly from pane 0.
