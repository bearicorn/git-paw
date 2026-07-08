## 1. Broker-bind failure

- [ ] 1.1 In the dashboard's broker startup, on bind failure emit a diagnostic to stderr and exit non-zero rather than looping/retrying

## 2. Lifecycle check on all paths

- [ ] 2.1 Hoist the `shutdown || orphaned() || tty_gone()` check so it guards every loop branch (normal poll arm AND any error/degraded arm)

## 3. Broaden orphan detection

- [ ] 3.1 Add a `tty_gone()` check — treat an `event::poll` error and a failed terminal write as terminal conditions — and exit on it alongside `orphaned()`; keep the non-Unix stub behavior for `orphaned()`

## 4. Tests

- [ ] 4.1 Behavioral test: `orphaned()` stays false when the parent is alive (unchanged); add a unit-level check that the lifecycle predicate returns "exit" when the tty-gone signal is set
- [ ] 4.2 A test/harness assertion that a bind-failure path exits (does not loop) — a smoke-level check is sufficient (TUI draw loop is coverage-exempt)

## 5. Docs

- [ ] 5.1 Note the broadened exit conditions in the dashboard chapter / architecture doc where the orphan-exit behavior is described
