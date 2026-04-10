## Context

This is the fourth and final Wave 1 change. It runs in the same process as the HTTP broker in pane 0 — the two share a `BrokerState` handle. The dashboard is the user's primary visibility into what N parallel agents are doing without having to tab between tmux panes.

v0.3.0 scope is deliberately small: a read-only status table rendered on a periodic tick. No input handling beyond `q` to quit. No scrolling, no prompt inbox, no interactive elements. The ratatui + crossterm investment is justified by v0.4's prompt inbox (interactive input, multi-section layout, cursor management) which would be painful to build on raw crossterm.

The dashboard is the most "visual" module in the codebase. It has a unique testing challenge: the draw loop touches terminal state and is hard to unit-test meaningfully. The agreed convention is **80% coverage on logic, exempt draw/input loops**.

## Goals / Non-Goals

**Goals:**

- Display a live-updating status table in pane 0 showing all known agents and their current state
- Share `BrokerState` with the HTTP server without additional IPC or message passing
- Handle terminal lifecycle cleanly: alternate screen, raw mode, panic recovery
- Structure the code so state-formatting logic is fully unit-testable via pure functions
- Provide a `run_dashboard` entry point that `broker-integration` (Wave 2) can call from a hidden subcommand

**Non-Goals:**

- Interactive input beyond `q` to quit (v0.4 prompt inbox)
- Scrolling through agents or messages (v0.4)
- Color themes or user-configurable styles (v1.0 polish)
- Mouse support in the dashboard (tmux already has mouse mode; the dashboard doesn't need its own)
- Broker lifecycle management — `run_dashboard` receives a `BrokerHandle` and drops it on exit; it does not start or stop the broker itself
- Logging from the dashboard — the dashboard writes to the terminal (ratatui owns stdout); diagnostic output goes nowhere in v0.3.0. If needed, a future version can log to a file.

## Decisions

### Decision 1: Separation into pure formatting functions + thin draw loop

The module is split into two layers:

```rust
// Pure functions — 80% coverage target
pub fn format_agent_rows(agents: &[AgentStatusEntry], now: Instant) -> Vec<AgentRow>
pub fn format_header() -> Vec<&'static str>
pub fn format_status_line(total: usize, working: usize, done: usize, blocked: usize) -> String

// Thin draw loop — exempt from coverage
fn draw_frame(frame: &mut Frame, rows: &[AgentRow], status_line: &str)
pub fn run_dashboard(state: BrokerState, broker_handle: BrokerHandle) -> Result<(), PawError>
```

`AgentRow` is a plain struct with pre-formatted `String` fields ready for rendering:

```rust
pub struct AgentRow {
    pub agent_id: String,
    pub cli: String,
    pub status: String,
    pub age: String,        // e.g. "30s ago", "3m ago"
    pub summary: String,    // one-line from Display of last message
}
```

**Why:**
- Pure functions are trivially testable with table-driven tests
- The draw loop is ~30 lines of ratatui boilerplate (`Table::new`, `Row::new`, `frame.render_widget`); testing it adds fragile snapshot assertions without meaningful coverage
- v0.4 will add prompt inbox rendering as a second widget in `draw_frame`; the pure functions stay unchanged
- This split is standard ratatui practice (see the ratatui cookbook examples)

**Alternatives considered:**
- *ratatui `TestBackend` snapshot tests.* Possible but snapshots break on terminal width changes, ratatui version bumps, and Unicode width calculation differences across platforms. Rejected for v0.3.0; reconsider if the UI becomes more complex.
- *No separation (format inline in draw loop).* Makes formatting logic untestable. Rejected.

### Decision 2: Tick-based polling, not event-driven updates

The dashboard polls `BrokerState` on a fixed interval (default 1 second) rather than reacting to push notifications from the broker.

```rust
const TICK_INTERVAL: Duration = Duration::from_secs(1);

loop {
    // Poll for keyboard events (non-blocking, 0ms timeout)
    if crossterm::event::poll(Duration::ZERO)? {
        if let Event::Key(key) = crossterm::event::read()? {
            if key.code == KeyCode::Char('q') { break; }
        }
    }

    // Read state and format
    let agents = delivery::agent_status_snapshot(&state);
    let rows = format_agent_rows(&agents, Instant::now());
    // ... draw ...

    thread::sleep(TICK_INTERVAL);
}
```

**Why:**
- Agent status changes on the order of seconds to minutes; 1-second refresh is more than fast enough for human readability and avoids rendering identical frames twice per second
- No channel setup between broker handlers and dashboard; both just read `BrokerState` via `RwLock`
- Simpler to reason about: the dashboard is a single-threaded loop that reads shared state periodically
- v0.4's prompt inbox will need an event channel anyway (for incoming questions); at that point we can switch to a `select!` loop over events + tick timer. For v0.3.0, `sleep` is correct.

**Alternatives considered:**
- *`tokio::sync::watch` channel from broker to dashboard.* Adds complexity, couples the dashboard to the broker's internal publish path, and the dashboard would need to run on tokio (it currently runs on the main thread without a runtime). Rejected.
- *crossterm `EventStream` (async).* Requires tokio runtime in the dashboard loop. Rejected — the dashboard is sync.

### Decision 3: The dashboard runs on the main thread, the broker runs on a spawned tokio runtime

The process that pane 0 executes:

```
fn main() {
    let state = BrokerState::new();
    let handle = start_broker(config, state.clone())?;  // spawns tokio runtime in background
    run_dashboard(state, handle)?;                        // blocks main thread until 'q'
    // handle dropped here → broker shuts down
}
```

**Why:**
- ratatui's draw loop is naturally synchronous — it renders a frame, sleeps, renders the next
- The broker needs a multi-threaded async runtime for handling concurrent HTTP requests
- Giving each its own execution model avoids impedance mismatch: no `block_on` in the dashboard, no raw-mode terminal conflicts in the broker
- `BrokerState` is `Arc<std::sync::RwLock<...>>` precisely so both the sync dashboard and the async handlers can read it

**Alternatives considered:**
- *Run the dashboard as a tokio task alongside the broker.* Forces the draw loop to be async, complicates crossterm raw-mode management (which is global process state), and makes signal handling harder. Rejected.

### Decision 4: Terminal setup/teardown in `run_dashboard`, with panic hook

`run_dashboard` is responsible for:

1. `crossterm::terminal::enable_raw_mode()`
2. `crossterm::execute!(stdout, EnterAlternateScreen)`
3. Install a panic hook via `std::panic::set_hook` that calls `disable_raw_mode` + `LeaveAlternateScreen` before printing the panic message — this prevents a panic from leaving the terminal in a broken state
4. Build `Terminal::new(CrosstermBackend::new(stdout))`
5. Run the draw loop
6. On exit (or error): `disable_raw_mode()`, `LeaveAlternateScreen`, restore the original panic hook

**Why:**
- Standard ratatui pattern; all ratatui examples do this
- The panic hook is critical for developer UX — a raw-mode terminal after a panic is unusable until `reset` is typed blind
- `run_dashboard` owns the full lifecycle so there's one place to audit terminal state transitions

**Alternatives considered:**
- *Use ratatui's `init()` / `restore()` convenience functions.* These exist in newer ratatui versions and wrap the exact same sequence. Acceptable if available; otherwise hand-roll. The implementing agent should check which ratatui version we pin and use the convenience API if present.

### Decision 5: `run_dashboard` accepts `BrokerHandle` for ownership transfer

The function signature is:

```rust
pub fn run_dashboard(state: BrokerState, broker_handle: BrokerHandle) -> Result<(), PawError>
```

`BrokerHandle` is moved into the function. When the dashboard exits (user presses `q`, or an error occurs), the function returns and `broker_handle` is dropped by the caller. The `Drop` impl on `BrokerHandle` triggers graceful broker shutdown.

**Why:**
- Ties the broker's lifetime to the dashboard's — "close the dashboard = close the broker" with no explicit shutdown call
- `broker-integration` (Wave 2) doesn't need separate broker-stop logic for pane 0; it's automatic
- The dashboard never calls any methods on `BrokerHandle` — it just holds it alive

**Alternatives considered:**
- *Dashboard returns, caller drops handle separately.* Same effect but the intent is less clear. Rejected in favor of explicit ownership.
- *Dashboard calls `handle.shutdown()` explicitly.* Redundant with `Drop`. Rejected.

### Decision 6: Human-readable age formatting

The `age` field in `AgentRow` displays relative time since the agent's last update:

- `< 60s` → `"Xs ago"` (e.g. `"30s ago"`)
- `1-59 min` → `"Xm ago"` (e.g. `"3m ago"`)
- `≥ 60 min` → `"Xh Ym ago"` (e.g. `"1h 15m ago"`)

**Why:**
- Matches the mockup in MILESTONE.md
- Trivially testable as a pure function `fn format_age(elapsed: Duration) -> String`
- No timezone handling needed — `Instant::now() - last_seen` is a monotonic delta

**Alternatives considered:**
- *Absolute timestamps (HH:MM:SS).* Harder to scan at a glance. Rejected.
- *Pull in `humantime` or `chrono-humanize`.* New dep for one function. Rejected — hand-roll ~15 lines.

### Decision 7: Status emoji rendering

The status column uses Unicode symbols to match the MILESTONE.md mockup:

| Status value | Symbol | Example |
|---|---|---|
| `"working"` | 🔵 | `🔵 working` |
| `"done"` | 🟢 | `🟢 done` |
| `"verified"` | 🟢 | `🟢 verified` |
| `"blocked"` | 🟡 | `🟡 blocked` |
| `"idle"` | ⚪ | `⚪ idle` |
| anything else | ⚪ | `⚪ <status>` |

**Why:**
- Colored circles are universally supported in modern terminals (macOS Terminal, iTerm2, Alacritty, kitty, GNOME Terminal, Windows Terminal via WSL)
- Scannable at a glance
- Implemented as a pure function `fn status_symbol(status: &str) -> &'static str` — trivially testable

**Alternative considered:**
- *ANSI color codes on text instead of emoji.* Requires managing color state, doesn't look as clean. Rejected.

## Risks / Trade-offs

- **ratatui version churn** → ratatui is actively developed; API breaking changes between minor versions have happened in the past. **Mitigation:** pin to a specific minor version (e.g. `ratatui = "=0.29"`) and update deliberately. The dashboard is simple enough that API migration is ~1 hour of work.

- **Unicode width calculation for emoji** → Some terminals measure emoji as 1 column, others as 2. This can misalign the status column. **Mitigation:** ratatui uses the `unicode-width` crate for column calculations, which handles most cases correctly. Accept minor misalignment on exotic terminals as a known issue.

- **Pane 0 width/height too small** → If the user makes pane 0 very narrow, the table columns wrap or truncate. **Mitigation:** ratatui `Table` supports `Constraint::Min` and `Constraint::Percentage` for responsive column widths. Set agent_id and summary as flexible, status/age as fixed. Degrade gracefully.

- **`BrokerState` lock contention** → The dashboard reads `agent_status_snapshot` every 1 second while the broker handlers may be writing. **Mitigation:** `agent_status_snapshot` takes a read lock for a few microseconds (it clones a small `Vec`). At 500ms intervals, contention is negligible. If v0.4 increases the tick rate, revisit.

- **Coverage exemption abuse** → Exempting the draw loop from coverage could hide bugs if logic leaks into it. **Mitigation:** keep the draw function under ~30 lines; if it grows, extract pure functions. Code review gate.

## Migration Plan

No migration. New module, new dependencies. Rollback is `git revert`. Existing v0.2.0 sessions see no change — the dashboard only appears when `broker-integration` (Wave 2) wires it into pane 0.
