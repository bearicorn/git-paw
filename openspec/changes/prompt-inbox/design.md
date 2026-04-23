## Context

The v0.3.0 dashboard is a read-only status monitor. When an agent publishes a question, the user has to switch tmux panes to answer it — breaking the supervisor's flow. The prompt inbox converts pane 0 from a passive display into the primary interaction surface for the operator.

This change extends `src/dashboard.rs` significantly: the single-section ratatui layout becomes a three-section layout, and key handling expands from a single `q` check to a full input event loop.

## Goals / Non-Goals

**Goals:**

- Extend the dashboard with a prompts section and text input
- Add `agent.question` message type with routing to the supervisor inbox
- Implement Tab/Enter/q key handling for prompt navigation and reply
- Reply routing via `tmux send-keys` to the agent's pane

**Non-Goals:**

- Persistent question history across dashboard restarts
- Multi-line answers (single-line input is sufficient for v0.4.0)
- Question prioritization or filtering beyond FIFO queue
- Rendering on terminals narrower than 80 columns (out of scope for v0.4.0)

## Decisions

### Decision 1: Three-section ratatui layout using `Layout::vertical`

```
┌─────────────────────────────────┐
│ git-paw dashboard               │  title (1 line)
│ ─────────────────────────────── │
│ Agent │ CLI │ Status │ ...      │  status table (flexible)
│ ─────────────────────────────── │
│ Questions (2 pending)           │  prompts section (fixed height ~8)
│ > [feat-config] Should default… │
│   [feat-detect] Is PATH scan…   │
│ ─────────────────────────────── │
│ Reply to feat-config> _         │  input field (3 lines)
└─────────────────────────────────┘
```

The status table takes the remaining vertical space after the fixed-height bottom sections are allocated.

**Why:**
- ratatui `Layout::vertical` with `Constraint::Min(0)` for the table and `Constraint::Length(N)` for the bottom sections is idiomatic
- Fixed heights for prompts and input prevent layout thrashing as question count changes

**Alternatives considered:**
- *Overlay/popup for prompts.* More visually prominent but hides agent status rows. Rejected.
- *Separate TUI pane for prompts.* Requires tmux splits within pane 0. Too complex. Rejected.

### Decision 2: `agent.question` routes to a special `"supervisor"` inbox

The `agent.question` message carries `question: String` and is delivered to the `"supervisor"` inbox in the broker. The dashboard polls this inbox (in addition to `agent_status_snapshot`) to populate the prompts section.

**Why:**
- Avoids adding a separate "question queue" API to the broker — reuses the existing inbox mechanism
- The supervisor inbox is the natural destination for messages needing human attention
- The dashboard already runs the broker; polling an additional inbox adds minimal overhead

**Alternatives considered:**
- *Broadcast question to all agents.* Would pollute coding agent inboxes. Rejected.
- *Dedicated HTTP endpoint `GET /questions`.* Extra broker surface area with no reuse. Rejected.

### Decision 3: Reply is sent via `tmux send-keys` to the agent's numbered pane

The dashboard knows the session name (`paw-<project>`) and the mapping from agent branch to pane index (established at session creation time, stored in session state). Reply routing:

```
tmux send-keys -t paw-<project>:<pane_index> "<answer>" Enter
```

**Why:**
- The agent CLI is already running in its pane waiting for input — `send-keys` is the correct mechanism
- No additional broker protocol needed for answer delivery
- The answer appears as if the user had typed it directly in the agent's pane

**Alternatives considered:**
- *POST answer to broker, agent polls it.* Requires a new broker endpoint and agent polling logic. Rejected for v0.4.0.
- *Write answer to a file the agent watches.* Agents would need file-watching code. Rejected.

### Decision 4: `QuestionEntry` is a new struct, not reusing `AgentStatusEntry`

```rust
pub struct QuestionEntry {
    pub agent_id: String,
    pub pane_index: usize,
    pub question: String,
    pub seq: u64,
}
```

Questions are a fundamentally different data type from agent status records. They are transient (consumed when answered), have a pane index for routing, and are displayed in FIFO order.

**Alternatives considered:**
- *Extend `AgentStatusEntry` with an optional question field.* Conflates status monitoring with interaction. Rejected.

## Risks / Trade-offs

- **`tmux send-keys` timing** — if the agent's pane is mid-execution (not waiting for input), the answer is injected into the wrong context. Mitigation: questions are only surfaced when the agent publishes `agent.question`, implying it is awaiting input.
- **Pane index stability** — if a pane crashes and restarts, its index may change. Mitigation: pane indices are fixed at session creation and agents don't restart in v0.4.0.
- **Input focus confusion** — `Tab` cycles questions but the user is always in the dashboard pane. Mitigation: clear cursor in the input field shows which question is active.
