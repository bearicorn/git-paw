# Dashboard

When the broker is enabled, one tmux pane runs the dashboard instead of an
agent CLI. In **supervisor mode** the dashboard lives at **pane 1** (pane 0
hosts the supervisor CLI itself); in **broker-only mode** (broker on,
supervisor off) the dashboard lives at **pane 0**. Either way it renders the
same live status table that updates every second, giving you an at-a-glance
view of what each agent is doing.

The dashboard is **observation-only** — it never sends actions back to agents. Beyond `q` to quit, its keystrokes drive the [Broker log panel](#broker-log-panel) (toggle, filter, inspect). Human input (questions, directives, replies to `agent.question` events) happens in the supervisor pane itself; the dashboard simply renders broker state.

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

The supervisor pane is also not a watch target, so the broker cannot infer its `cli` from the watch-target map. To populate the `CLI` column for the supervisor row, the supervisor self-registration `agent.status` includes a `cli` field (e.g. `"cli":"claude"`). The broker upserts that value into its internal CLI map when it receives the message.

The `CLI` column is populated for **every** agent row, not just the supervisor. git-paw knows each pane's CLI at launch and pre-fills it authoritatively — coding agents from the watch-target map (the per-repo session JSON), the supervisor from `[supervisor].cli`/`default_cli`. Agents do **not** self-report their CLI (they would only be guessing). A row appears once its pane publishes a status, and its `CLI` column shows the pre-filled value; if a CLI somehow can't be resolved it shows a `?` placeholder rather than a blank cell.

### The roster holds only real agents

The `/status` agent roster — and therefore the dashboard table — is built **only** from agents that publish `agent.status`. The `from`/`target`/`verified_by` identity fields on `agent.feedback`, `agent.question`, and `agent.verified` messages are routed and stored but never mint a roster row. This means a human- or supervisor-originated feedback can no longer create a phantom `human` agent that never heartbeats. The roster is in-memory, so any pre-existing phantom from an older broker also clears on the next `git paw start`.

### When the supervisor row appears

The supervisor row appears **after** the supervisor pane's CLI has booted and published its first self-registration `agent.status` — typically within 3-5 seconds of `git paw start --supervisor` returning. There is no phantom supervisor row at launch time; if the supervisor pane fails to start, the row simply never appears. Aborted launches (non-TTY skip, missing CLI on PATH, system-level pane spawn failure) leave the agent table free of a misleading supervisor entry.

## Controls

| Key | Action |
|-----|--------|
| `q` | Quit the dashboard (shuts down the broker; agent panes keep running but lose broker communication) |
| `l` | Toggle the [Broker log panel](#broker-log-panel) on/off |
| `a` | Reset the Broker log filter to `All` |
| `1`–`9` | Toggle the individual filter chips (status / artifact / blocked / verified / feedback / question / intent / verify-now / advanced-main) |
| `↑`/`k`, `↓`/`j` | Move the highlight up/down the Broker log rows |
| `Enter` | Open the details overlay for the highlighted row |
| `Esc` | Close the details overlay |

## Broker log panel

The **Broker log** panel fills the screen region freed when v0.5.0 removed the
prompt inbox. It renders a scrolling, type-filterable list of the broker
messages observed during the current dashboard session, newest at the top, so
you can watch the session's wire-level activity at a glance instead of tailing
`/messages/<id>` over `curl`.

The log is **in-memory only** — closing the dashboard drops it — and bounded by
`[dashboard.broker_log] max_messages` (default 500). Older messages fall off the
top as new ones arrive. The panel is read-only: replying to a question or
directing an agent still happens in the supervisor pane.

### Showing and hiding the panel

The panel is visible by default. Toggle it with `l`, or set the launch default
in `.git-paw/config.toml`:

```toml
[dashboard.broker_log]
max_messages = 500
default_visible = true
```

When the panel is hidden the dashboard layout is identical to its v0.5.0
post-inbox-removal shape (title, agent table, status line) — the agent table
expands to fill the freed space. See the
[configuration reference](../configuration/README.md#broker-log-panel) for the
table's fields.

### Filter chips

A header row of chips sits above the message list, one per broker message type
plus an `All` reset. `All` is active by default (every message shows). Pressing
a digit hotkey (`1`–`9`) narrows the view to that type; pressing more digits adds
types inclusively; pressing an active chip again removes it (emptying the
selection returns to `All`). Press `a` to reset to `All` at any time. Filtering
is a view operation — the ring buffer always retains every message regardless of
the active chips.

| Hotkey | Chip | Matches |
|--------|------|---------|
| `1` | status | `agent.status` |
| `2` | artifact | `agent.artifact` |
| `3` | blocked | `agent.blocked` |
| `4` | verified | `agent.verified` |
| `5` | feedback | `agent.feedback` |
| `6` | question | `agent.question` |
| `7` | intent | `agent.intent` |
| `8` | verify-now | `supervisor.verify-now` |
| `9` | advanced-main | `agent.advanced-main` |

### Row format

Each row is a single line: `HH:MM:SS · type · agent · summary`, where the
summary is a per-type one-liner derived from the message body (the status
message, the first modified file of an artifact, the blocking need, the intent
summary, and so on). Summaries that overflow the panel width are truncated with
an ellipsis (`…`); the full body is available in the details overlay.

```
┌─ Broker log (7 shown / 7 held) — l hide · a all · 1-9 filter · ↵ details · Esc close ─┐
│  All  1:status 2:artifact 3:blocked 4:verified 5:feedback 6:question 7:intent 8:verify-now 9:advanced-main │
│ 14:35:09 · status   · feat-auth · working: rebasing onto main                          │
│ 14:34:58 · intent   · feat-auth · wire AuthClient                                       │
│ 14:34:12 · blocked  · feat-api  · needs auth token format from feat-auth               │
│ 14:33:40 · artifact · fix-typo  · done: src/typos.rs                                    │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

### Details overlay

Highlight a row with the arrow keys (or `j`/`k`) and press `Enter` to open a
modal overlay showing that message's full, pretty-printed JSON in a scrollable
view. Press `Esc` to close it and return to the panel. While the overlay is
open, `q` still quits the dashboard.

### Resilience across broker restarts

The panel never clears its buffer when the broker watcher restarts mid-session.
Historical messages stay visible across a transient outage, and new messages
resume appearing at the top once the watcher comes back. A gap in the timestamp
column is the only visible sign that messages briefly stopped flowing.

### Legacy messages panel

The earlier `[dashboard] show_message_log` flag rendered a simpler, unfiltered
messages list. It is superseded by the Broker log panel and retained only for
config compatibility.

## Relationship to the Broker

The dashboard and broker run in the same process (`git paw __dashboard`). The dashboard reads from shared state that the broker's HTTP handlers (and the watcher, conflict detector, and learnings aggregator subsystems) write to. There is no separate broker process to manage.

## Replying to agent questions

Earlier dashboard versions included a "Questions" panel and a "Reply to" input field for human-typed answers to `agent.question` events. The panel was removed in v0.5.0 because the supervisor pane is the natural input surface — typed questions and replies go through `tmux send-keys` and the supervisor agent's own curl machinery, not through the dashboard.

`agent.question` messages still flow through the broker. The supervisor pane polls the supervisor inbox, reads incoming questions, and replies via `tmux send-keys` to the asking agent's pane (and via `agent.feedback` to the broker for the audit log). See the [Supervisor mode](../quick-start-supervisor.md) chapter and the embedded `supervisor.md` skill for the reply flow.

See the [v0.5.0 changelog](../changelog.md) for the removal note.
