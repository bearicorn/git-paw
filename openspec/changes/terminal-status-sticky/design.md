## Design: Terminal Status Protection

### Current Implementation

The terminal status sticky behavior is already implemented in `src/broker/delivery.rs` in the `update_agent_record` function (lines 48-54):

```rust
// Make terminal states sticky: only update status if the new status is also terminal
// or if the current status is not terminal
let is_terminal_status = |s: &str| matches!(s, "done" | "verified" | "blocked" | "committed");

if !is_terminal_status(&record.status) || is_terminal_status(&status) {
    record.status = status;
}
```

### Technical Approach

1. **Terminal State Definition**: The `is_terminal_status` closure identifies four terminal states:
   - `"done"` - Agent has completed its task
   - `"verified"` - Agent's work has been verified
   - `"blocked"` - Agent is blocked on dependencies
   - `"committed"` - Agent's changes have been committed

2. **Update Logic**: The conditional update logic ensures:
   - Non-terminal states can always be updated
   - Terminal states can only be updated by other terminal states
   - This prevents transient states like "working" from overwriting important terminal states

3. **Integration**: This logic is called in `publish_message` which updates the agent record whenever a message is published.

### Test Strategy

The implementation will be tested with four scenarios:

1. **Terminal state cannot be overwritten by non-terminal state**
   - Setup: Agent has status "done"
   - Action: Publish message with status "working"
   - Expected: Status remains "done"

2. **Terminal state can be overwritten by another terminal state**
   - Setup: Agent has status "done"
   - Action: Publish message with status "verified"
   - Expected: Status changes to "verified"

3. **Non-terminal state can be overwritten by terminal state**
   - Setup: Agent has status "working"
   - Action: Publish message with status "done"
   - Expected: Status changes to "done"

4. **All terminal states are protected**
   - Test each terminal state: "done", "verified", "blocked", "committed"
   - Verify none can be overwritten by "working" or other non-terminal states

### Implementation Plan

1. Add formal specification in `specs/terminal-status-protection/spec.md`
2. Add unit tests in `src/broker/delivery.rs`
3. Update audit documentation
4. Verify all tests pass

### Stall Detection Implementation Plan

1. Extend `AgentRecord` struct with `last_seen: Instant` field
2. Implement `update_last_seen()` method that updates the timestamp
3. Create `detect_stalled_agents()` function with configurable threshold
4. Implement `intervene_stalled_pane()` function with tmux integration
5. Add stall detection to broker's main polling loop
6. Add configuration options to broker configuration
7. Write comprehensive unit and integration tests

### Technical Details

#### Data Structures
```rust
struct AgentRecord {
    // ... existing fields
    last_seen: Instant,
    stall_interventions: Vec<StallInterventionLog>,
}

struct StallInterventionLog {
    timestamp: Instant,
    action_taken: String,
    success: bool,
}
```

#### Stall Detection Algorithm
```rust
fn detect_stalled_agents(
    agents: &HashMap<String, AgentRecord>,
    threshold: Duration,
    now: Instant
) -> Vec<String> {
    agents.iter()
        .filter(|(_, record)| {
            record.status == "working" &&
            now.duration_since(record.last_seen) > threshold &&
            !record.recent_activity
        })
        .map(|(agent_id, _)| agent_id.clone())
        .collect()
}
```

#### Automatic Intervention
```rust
fn intervene_stalled_pane(pane_index: usize) -> Result<(), PawError> {
    // Send BTab Down Enter Enter sequence to unblock permission prompts
    Tmux::send_keys_to_pane(pane_index, "BTab Down Enter Enter")?;
    
    // Log the intervention
    log::info!("Automatic stall intervention on pane {}", pane_index);
    
    Ok(())
}
```