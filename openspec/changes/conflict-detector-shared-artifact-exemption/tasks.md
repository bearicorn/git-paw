## 1. Exempt the shared task-tracking artifact

- [ ] 1.1 In `src/broker/conflict.rs`, resolve the active spec backend's task-tracking file (the tasks file agents are instructed to tick) and exempt that path from ownership-violation detection
- [ ] 1.2 Keep source-file ownership violations firing (exemption scoped to the task artifact only)
- [ ] 1.3 Tests: a writeback to the shared `tasks.md` emits no ownership violation; a real `src/a.rs` ownership violation is still reported

## 2. Docs

- [ ] 2.1 Conflict Detection chapter notes the task-artifact exemption
- [ ] 2.2 `mdbook build docs/` succeeds

## 3. Gates

- [ ] 3.1 `just check` green
- [ ] 3.2 Every spec scenario maps to a test
