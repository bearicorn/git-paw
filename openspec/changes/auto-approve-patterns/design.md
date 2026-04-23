## Context

Permission prompts are the single largest productivity bottleneck in supervisor operation. During dogfood testing, each Claude agent required 4-8 manual approvals per task for common commands like `cargo fmt`, `cargo test`, `git commit`, and `curl`. The supervisor spent more time approving prompts than coordinating actual work.

Current state:
- No automated permission prompt detection
- No safe command classification
- Manual intervention required for every prompt
- Compound problem with multiple agents (3 agents × 8 prompts = 24 interventions)
- Particularly severe with curl commands to broker endpoints

## Goals / Non-Goals

**Goals:**
- Automatically detect permission prompts in agent panes
- Classify common development commands as safe for auto-approval
- Implement automatic approval key sequences (BTab Down Enter)
- Create shared allowlist for broker curl commands
- Reduce manual intervention by 80%+ in typical workflows
- Maintain safety by only auto-approving known-safe commands

**Non-Goals:**
- Approving arbitrary or unknown commands
- Modifying agent CLI permission systems
- Creating agent-side approval logic
- Handling non-permission types of stalls

## Decisions

### 1. Permission Prompt Detection

**Decision**: Use `tmux capture-pane` to detect permission prompts by analyzing pane content.

**Rationale**:
- Non-invasive (no agent modification required)
- Works with any agent CLI type
- Can detect specific prompt patterns
- Proven effective in dogfood testing

**Implementation**:
```rust
fn detect_permission_prompt(pane_index: usize) -> Option<PermissionType> {
    let content = Tmux::capture_pane(pane_index)?;
    
    if content.contains("requires approval") {
        if content.contains("curl") {
            return Some(PermissionType::Curl);
        } else if content.contains("cargo") {
            return Some(PermissionType::Cargo);
        }
    }
    
    None
}
```

### 2. Safe Command Classification

**Decision**: Use pattern matching against known-safe command classes.

**Rationale**:
- Explicit whitelist is safer than blacklist
- Easy to audit and maintain
- Can be extended with new patterns
- Configurable by users

**Safe Command Classes**:
- `cargo fmt`: Code formatting
- `cargo clippy`: Linting
- `cargo test`: Testing
- `git commit`: Version control
- `git push`: Version control
- `curl http://127.0.0.1:9119/*`: Broker communication

### 3. Automatic Approval Sequence

**Decision**: Send `BTab Down Enter` sequence to approve and remember the decision.

**Rationale**:
- BTab focuses the permission prompt
- Down selects "Yes, don't ask again"
- First Enter confirms the selection
- Matches manual approval workflow
- Proven effective in dogfood testing

### 4. Curl Allowlist Implementation

**Decision**: Create shared allowlist file for broker curl commands.

**Rationale**:
- Prevents repeated curl permission prompts
- Centralized configuration
- Easy to update across sessions
- Can be pre-populated with common broker endpoints

**Implementation**:
```rust
fn setup_curl_allowlist(broker_url: &str) -> Result<(), PawError> {
    let allowlist = vec![
        format!("curl -s {}/publish", broker_url),
        format!("curl -s {}/status", broker_url),
        format!("curl -s {}/poll", broker_url),
    ];
    
    // Write to .claude/settings.json or equivalent
    // This prevents first curl prompt from appearing
    
    Ok(())
}
```

### 5. Integration with Stall Detection

**Decision**: Trigger auto-approve when stall detection identifies stuck agents.

**Rationale**:
- Stall detection already identifies problematic panes
- Natural integration point
- Prevents false positives from approval
- Leverages existing infrastructure

## Risks / Trade-offs

**[Risk] False positives in prompt detection** → Mitigation:
- Use conservative pattern matching
- Require exact phrase matches
- Log detection events for debugging
- Allow manual override

**[Risk] Approving unsafe commands** → Mitigation:
- Use explicit whitelist only
- Never approve arbitrary commands
- Log all auto-approvals
- Make allowlist user-configurable

**[Risk] Agent CLI changes break detection** → Mitigation:
- Use multiple detection patterns
- Make patterns configurable
- Provide override mechanism
- Document pattern format

**[Risk] Performance impact from frequent capture-pane** → Mitigation:
- Only capture when stall detected
- Limit capture frequency
- Use efficient string matching
- Cache detection results

## Migration Plan

1. **Phase 1: Foundation**
   - Implement `detect_permission_prompt()` function
   - Add safe command classification
   - Create approval sequence logic
   - Add configuration options

2. **Phase 2: Curl Allowlist**
   - Implement `setup_curl_allowlist()`
   - Integrate with session startup
   - Test with various broker URLs

3. **Phase 3: Integration**
   - Connect to stall detection system
   - Add logging and monitoring
   - Test end-to-end workflow

4. **Phase 4: Rollout**
   - Enable by default in supervisor mode
   - Add user documentation
   - Monitor effectiveness in dogfood

**Rollback Strategy**: Feature is configurable. If issues arise, disable via `[supervisor] auto_approve = false` configuration.

## Open Questions

1. Should auto-approve be enabled by default? (Current: yes, conservative patterns)
2. What's the optimal balance between safety and automation? (Current: safety first)
3. Should we add user notification when auto-approval occurs? (Current: log only)
4. How to handle CLI-specific permission prompt variations? (Current: multiple patterns)