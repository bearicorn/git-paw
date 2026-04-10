## 1. Dependencies

- [ ] 1.1 Add `ratatui` to `[dependencies]` in `Cargo.toml` with default features
- [ ] 1.2 Add `crossterm` to `[dependencies]` in `Cargo.toml` with default features
- [ ] 1.3 Run `cargo build` and confirm both crates resolve and compile
- [ ] 1.4 Update `deny.toml` if `cargo deny check` flags any new transitive licenses; allow only OSI-approved permissive licenses

## 2. Module scaffolding

- [ ] 2.1 Create `src/dashboard.rs` with module-level doc comment explaining: pane 0 TUI, reads BrokerState, 1-second tick, v0.3.0 is read-only status table
- [ ] 2.2 Add `mod dashboard;` declaration in `src/main.rs`
- [ ] 2.3 Confirm `cargo build` succeeds with the empty module

## 3. Data types

- [ ] 3.1 Define `pub struct AgentRow` with fields: `agent_id: String`, `cli: String`, `status: String`, `age: String`, `summary: String`. Derive `Debug, Clone`
- [ ] 3.2 Add doc comments on the struct and each field explaining their role in the display table

## 4. Pure formatting functions

- [ ] 4.1 Implement `pub fn status_symbol(status: &str) -> &'static str` matching the mapping table in the spec: `"working"` → `"🔵"`, `"done"` → `"🟢"`, `"verified"` → `"🟢"`, `"blocked"` → `"🟡"`, `"idle"` or anything else → `"⚪"`
- [ ] 4.2 Implement `pub fn format_age(elapsed: Duration) -> String`:
  - `< 60s` → `"{secs}s ago"`
  - `1-59 min` → `"{mins}m ago"`
  - `≥ 60 min` → `"{hours}h {mins}m ago"`
- [ ] 4.3 Implement `pub fn format_agent_rows(agents: &[AgentStatusEntry], now: Instant) -> Vec<AgentRow>`:
  - For each agent, compute elapsed time from `last_seen` to `now`
  - Build `AgentRow` with `status_symbol` prepended to the status label, `format_age` for the age column, and `Display` of the last message for the summary
- [ ] 4.4 Implement `pub fn format_status_line(total: usize, working: usize, done: usize, blocked: usize) -> String` returning `"{total} agents: {working} working, {done} done, {blocked} blocked"`
- [ ] 4.5 Add doc comments on all four functions

## 5. Terminal lifecycle

- [ ] 5.1 Implement a helper `fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>, PawError>`:
  - `crossterm::terminal::enable_raw_mode()?`
  - `crossterm::execute!(stdout(), EnterAlternateScreen)?`
  - `Terminal::new(CrosstermBackend::new(stdout()))?`
- [ ] 5.2 Implement a helper `fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<(), PawError>`:
  - `crossterm::terminal::disable_raw_mode()?`
  - `crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)?`
  - `terminal.show_cursor()?`
- [ ] 5.3 Check if the pinned ratatui version provides `ratatui::init()` / `ratatui::restore()` convenience functions; if so, use them instead of hand-rolling steps 5.1/5.2
- [ ] 5.4 Install a custom panic hook that calls `restore_terminal`-equivalent cleanup before forwarding to the default panic handler. Use `std::panic::set_hook` and save the original hook for chaining.

## 6. Draw function

- [ ] 6.1 Implement `fn draw_frame(frame: &mut Frame, rows: &[AgentRow], status_line: &str)`:
  - Create a ratatui `Layout` splitting the terminal into: title area (1 line), table area (remaining), status line area (1 line)
  - Render the title `"git-paw dashboard"` in the title area using `Paragraph` or `Block`
  - Build a `Table` widget from `rows` with columns: Agent, CLI, Status, Last Update, Summary. Use `Constraint::Min` for flexible columns, `Constraint::Length` for fixed columns.
  - If `rows` is empty, render `"No agents connected yet"` as a centered `Paragraph` in the table area instead
  - Render `status_line` in the bottom area
- [ ] 6.2 Keep `draw_frame` under ~40 lines; if it grows, extract widget-building into helpers

## 7. Main loop

- [ ] 7.1 Implement `pub fn run_dashboard(state: BrokerState, broker_handle: BrokerHandle) -> Result<(), PawError>`:
  - Call `setup_terminal()`
  - Define `const TICK_INTERVAL: Duration = Duration::from_secs(1);`
  - Enter loop:
    - Poll crossterm events with `Duration::ZERO` timeout (non-blocking)
    - If `Event::Key(KeyCode::Char('q'))` → break
    - Read agents via `delivery::agent_status_snapshot(&state)` — acquire read lock, clone data, release lock immediately
    - Call `format_agent_rows`, `format_status_line`
    - Call `terminal.draw(|f| draw_frame(f, &rows, &status_line))?`
    - `thread::sleep(TICK_INTERVAL)`
  - After loop: call `restore_terminal()`
  - `drop(broker_handle)` is implicit on return
  - Return `Ok(())`
- [ ] 7.2 Wrap the loop body in a match or use `?` to ensure `restore_terminal` is called even on errors (consider a guard struct that calls `restore_terminal` on `Drop` for safety)
- [ ] 7.3 Ensure no `BrokerState` lock guard is held across the `terminal.draw` call or the `thread::sleep`

## 8. Unit tests — pure functions

- [ ] 8.1 Add `#[cfg(test)] mod tests` block at the bottom of `src/dashboard.rs`
- [ ] 8.2 Test `status_symbol`: `"working"` → `"🔵"`, `"done"` → `"🟢"`, `"verified"` → `"🟢"`, `"blocked"` → `"🟡"`, `"idle"` → `"⚪"`, unknown → `"⚪"`
- [ ] 8.3 Test `format_age`: 0s → `"0s ago"`, 30s → `"30s ago"`, 180s → `"3m ago"`, 3600s → `"1h 0m ago"`, 4500s → `"1h 15m ago"`
- [ ] 8.4 Test `format_agent_rows`: 3 agents in → 3 rows out, correct `agent_id` and `age` fields populated
- [ ] 8.5 Test `format_agent_rows`: single agent with status `"done"` and 180s elapsed → row has `"3m ago"` in age field and `"done"` in status field
- [ ] 8.6 Test `format_agent_rows`: empty input → empty output
- [ ] 8.7 Test `format_status_line(4, 2, 1, 1)` → `"4 agents: 2 working, 1 done, 1 blocked"`
- [ ] 8.8 Test `format_status_line(3, 0, 3, 0)` → `"3 agents: 0 working, 3 done, 0 blocked"`
- [ ] 8.9 Test `format_status_line(0, 0, 0, 0)` → `"0 agents: 0 working, 0 done, 0 blocked"`

## 9. Quality gates

- [ ] 9.1 `cargo fmt` clean
- [ ] 9.2 `cargo clippy --all-targets -- -D warnings` clean (all public items documented, no `unwrap`/`expect` outside tests)
- [ ] 9.3 `cargo test` — all new tests pass
- [ ] 9.4 `cargo doc --no-deps` builds without warnings for the new module
- [ ] 9.5 `just deny` clean (license checks pass with ratatui + crossterm tree)
- [ ] 9.6 `just check` — full pipeline green
- [ ] 9.7 Verify coverage on pure formatting functions meets 80% threshold (use `cargo llvm-cov` on `src/dashboard.rs` excluding the `run_dashboard` and `draw_frame` functions if tooling supports it; otherwise verify manually that pure function tests cover all branches)

## 10. Handoff readiness

- [ ] 10.1 Confirm `src/dashboard.rs` exposes `run_dashboard`, `format_agent_rows`, `format_age`, `format_status_line`, `status_symbol`, `AgentRow` as public API
- [ ] 10.2 Confirm the draw loop is ≤ 40 lines and contains no formatting logic (all formatting in pure functions)
- [ ] 10.3 Confirm no changes outside `src/dashboard.rs`, `src/main.rs` (mod declaration), `Cargo.toml` (deps), `deny.toml` (if needed)
- [ ] 10.4 Confirm no `#[tokio::main]` or tokio runtime usage in `src/dashboard.rs` — the dashboard is synchronous
- [ ] 10.5 Commit with message: `feat(dashboard): add ratatui status table for pane 0`
