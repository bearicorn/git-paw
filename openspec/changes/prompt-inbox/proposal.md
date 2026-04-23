## Why

The v0.3.0 dashboard is read-only — it shows agent status but the user can't interact with agents through it. When an agent asks a question (publishes a message needing human input), the user has to switch to the agent's tmux pane to answer. The prompt inbox extends the dashboard with a text input field so the user can see agent questions and reply directly from pane 0.

## What Changes

- Extend the dashboard TUI (`src/dashboard.rs`) with two new sections below the status table:
  - **Prompts section**: shows messages from agents that need human attention (questions, blocked requests)
  - **Input field**: text input with cursor, focused on a specific agent pane for reply routing

- Add a new message type to the broker: `agent.question`
  - Payload: `question: String` — the question the agent is asking
  - Routing: delivered to the `supervisor` inbox (the supervisor or dashboard shows it)
  - Display: `[feat-config] question: Should default_cli show "(default)" label?`

- Dashboard key handling (extending beyond just `q`):
  - `Tab` to cycle through pending questions
  - `Enter` to send the reply to the focused agent's pane via `tmux send-keys`
  - `q` still quits

- Requires ratatui multi-section layout:
  - Top: status table (existing)
  - Middle: prompts list (new)
  - Bottom: input field (new)

## Capabilities

### New Capabilities

- `prompt-inbox`: Interactive prompt section in the dashboard TUI with question display and answer routing

### Modified Capabilities

- `dashboard`: Extend with prompts section, input field, and key handling beyond `q`
- `broker-messages`: Add `agent.question` variant
- `message-delivery`: Route question messages to supervisor inbox

## Impact

- **Modified files:** `src/dashboard.rs` (major — multi-section layout, input handling, key events), `src/broker/messages.rs` (add Question variant), `src/broker/delivery.rs` (add routing)
- **New dependencies:** none (ratatui already supports input widgets)
- **Depends on:** `supervisor-messages` (broker message infrastructure), `dashboard-tui` from v0.3.0
