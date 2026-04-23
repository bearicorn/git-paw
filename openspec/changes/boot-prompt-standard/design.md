## Context

The boot-prompt standard addresses a critical reliability gap in agent coordination. During dogfood testing, agents frequently failed to self-report their status, making it impossible for the supervisor to track progress or detect stalled agents. The boot instruction block pattern (register/done/blocked/question) was proven effective but needs standardization and automatic injection.

Current state:
- Agents receive task prompts without boot instructions
- Supervisor must manually intervene to get agents to self-report
- No consistency between supervisor and manual broker modes
- No template substitution for broker URLs and branch IDs

## Goals / Non-Goals

**Goals:**
- Establish standardized boot instruction format that all agents understand
- Automate boot block injection in both supervisor and manual modes
- Provide template substitution for dynamic values (branch ID, broker URL)
- Ensure consistency across all agent launch paths
- Reduce manual intervention required from supervisor

**Non-Goals:**
- Changing agent CLI behavior (agents already support the curl-based reporting)
- Modifying existing agent skill templates beyond adding boot instructions
- Adding new message types (using existing agent.status/artifact/blocked/question)
- Creating agent-side enforcement (rely on skill template instructions)

## Decisions

### 1. Standard Boot Block Format

**Decision**: Use a fixed four-event boot block covering register, done, blocked, question.

**Rationale**:
- Proven effective in dogfood testing (all 3 agents followed instructions)
- Covers the essential lifecycle events needed for supervision
- Simple enough for agents to understand and follow
- Extensible if additional events are needed later

**Format**:
```
## BOOT INSTRUCTIONS - DO NOT REMOVE

1. REGISTER: Immediately publish your status:
   curl -s "{{GIT_PAW_BROKER_URL}}/publish" \
     -H "Content-Type: application/json" \
     -d '{"type":"agent.status","agent_id":"{{BRANCH_ID}}","status":"working","message":"booting"}'

2. DONE: When you complete your task, publish:
   curl -s "{{GIT_PAW_BROKER_URL}}/publish" \
     -H "Content-Type: application/json" \
     -d '{"type":"agent.artifact","agent_id":"{{BRANCH_ID}}","status":"done","message":"Task completed"}'

3. BLOCKED: When waiting on dependencies, publish:
   curl -s "{{GIT_PAW_BROKER_URL}}/publish" \
     -H "Content-Type: application/json" \
     -d '{"type":"agent.blocked","agent_id":"{{BRANCH_ID}}","needs":"dependency","from":"other-agent"}'

4. QUESTION: When uncertain, ASK FIRST then WAIT for answer:
   curl -s "{{GIT_PAW_BROKER_URL}}/publish" \
     -H "Content-Type: application/json" \
     -d '{"type":"agent.question","agent_id":"{{BRANCH_ID}}","question":"Your question here"}'
   
   DO NOT continue until you receive an answer!
```

### 2. Template Substitution Strategy

**Decision**: Pre-expand all templates at render time using Rust string replacement.

**Rationale**:
- Prevents shell expansion permission prompts in agent CLIs
- Ensures consistent formatting across all agents
- Simpler than runtime template processing in agents
- Matches existing pattern used in skill templates

**Implementation**:
```rust
fn build_boot_block(branch_id: &str, broker_url: &str) -> String {
    let template = include_str!("../assets/boot-block-template.md");
    template
        .replace("{{BRANCH_ID}}", &slugify_branch(branch_id))
        .replace("{{GIT_PAW_BROKER_URL}}", broker_url)
}
```

### 3. Dual Injection Paths

**Decision**: Implement separate injection mechanisms for supervisor and manual modes.

**Rationale**:
- Supervisor mode: Full control over prompt construction (prepend)
- Manual mode: Cannot modify user's paste, but can pre-fill input line
- Maintains flexibility for both usage patterns
- Shared helper ensures consistency

**Supervisor Mode Path**:
```rust
// In cmd_supervisor() before tmux send-keys
let boot_block = build_boot_block(&branch_id, &broker_url);
let full_prompt = format!("{}\n\n{}", boot_block, task_prompt);
Tmux::send_keys_to_pane(pane_index, &full_prompt)?;
```

**Manual Mode Path**:
```rust
// In cmd_start() after tmux session creation
let boot_block = build_boot_block(&branch_id, &broker_url);
Tmux::send_keys_to_pane(pane_index, &boot_block, /* no_enter= */ true)?;
// User then pastes their actual task and presses Enter
```

### 4. Shared Helper Function

**Decision**: Create `build_boot_block()` in `src/skills.rs` for maximum reusability.

**Rationale**:
- Skills module already handles template rendering
- Central location accessible from both main.rs and agents.rs
- Can leverage existing slugify_branch() function
- Easy to test in isolation

## Risks / Trade-offs

**[Risk] Agents ignore boot instructions** → Mitigation: 
- Include "DO NOT REMOVE" header in bold
- Add to supervisor skill template as mandatory pattern
- Document in agent coordination guidelines

**[Risk] Template substitution errors** → Mitigation:
- Validate broker URL format before substitution
- Use slugify_branch() to ensure valid agent IDs
- Add unit tests for edge cases

**[Risk] Boot block too verbose** → Mitigation:
- Keep instructions concise and action-oriented
- Use clear section headers (REGISTER, DONE, BLOCKED, QUESTION)
- Test with actual agents to validate comprehension

**[Risk] Manual mode users confused** → Mitigation:
- Document the pre-fill behavior in help text
- Add visual indicator that boot block is present
- Provide example of expected workflow

## Migration Plan

1. **Phase 1: Foundation**
   - Create `build_boot_block()` function with tests
   - Add boot block template file
   - Update supervisor skill template

2. **Phase 2: Supervisor Integration**
   - Modify `cmd_supervisor()` to prepend boot blocks
   - Add configuration option (default: enabled)
   - Test with supervisor dogfood session

3. **Phase 3: Manual Mode Integration**
   - Modify `cmd_start()` for broker-enabled sessions
   - Add user-facing documentation
   - Test manual workflow

4. **Phase 4: Rollout**
   - Update all example skill templates
   - Add to documentation site
   - Monitor adoption in dogfood sessions

**Rollback Strategy**: Feature is purely additive with no breaking changes. If issues arise, disable via configuration or remove boot block prepending.

## Open Questions

1. Should boot blocks be configurable/disableable? (Current decision: always enabled)
2. What's the optimal boot block length? (Current: ~20 lines)
3. Should we add agent-side validation of boot block presence? (Current: no, rely on skill template)
4. How to handle boot block updates when skill templates change? (Current: versioned templates)