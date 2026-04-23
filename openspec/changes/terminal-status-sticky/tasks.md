## 1. Verify Existing Implementation

- [ ] 1.1 Review existing `is_terminal_status` implementation in `src/broker/delivery.rs`
- [ ] 1.2 Verify terminal state protection logic works correctly
- [ ] 1.3 Run existing tests to confirm no regressions

## 2. Add Stall Detection Functionality

- [ ] 2.1 Add `last_seen` timestamp tracking to agent records
- [ ] 2.2 Implement stall detection algorithm in broker
- [ ] 2.3 Add threshold configuration (default: 60 seconds)
- [ ] 2.4 Create automatic pane intervention mechanism

## 3. Update Specifications

- [ ] 3.1 Add stall detection requirements to terminal-status-protection spec
- [ ] 3.2 Add new scenarios for stall detection and recovery
- [ ] 3.3 Update design document with stall detection details

## 4. Implementation

- [ ] 4.1 Extend `AgentRecord` struct with `last_seen` field
- [ ] 4.2 Implement `detect_stalled_agents()` function
- [ ] 4.3 Add `intervene_stalled_pane()` function with tmux integration
- [ ] 4.4 Update broker polling loop to call stall detection

## 5. Testing

- [ ] 5.1 Add unit tests for stall detection logic
- [ ] 5.2 Add integration tests for pane intervention
- [ ] 5.3 Test with various stall thresholds
- [ ] 5.4 Verify no interference with normal operation

## 6. Documentation

- [ ] 6.1 Update AGENTS.md with stall detection behavior
- [ ] 6.2 Add configuration documentation for thresholds
- [ ] 6.3 Update user-facing docs with new feature description