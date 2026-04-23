## 1. Foundation Setup

- [ ] 1.1 Create boot block template file at `assets/boot-block-template.md`
- [ ] 1.2 Add `build_boot_block()` function to `src/skills.rs`
- [ ] 1.3 Implement template substitution logic with slugification
- [ ] 1.4 Add unit tests for `build_boot_block()` function
- [ ] 1.5 Add paste handling instructions to boot block template

## 2. Supervisor Mode Integration

- [ ] 2.1 Modify `cmd_supervisor()` to call `build_boot_block()` for each agent
- [ ] 2.2 Prepend boot block to agent prompts before `tmux send-keys`
- [ ] 2.3 Ensure supervisor self-registration publishes boot status
- [ ] 2.4 Add integration tests for supervisor boot block injection

## 3. Manual Mode Integration

- [ ] 3.1 Modify `cmd_start()` for broker-enabled sessions
- [ ] 3.2 Pre-fill boot blocks without Enter key using `tmux send-keys -l`
- [ ] 3.3 Add user documentation for manual mode workflow
- [ ] 3.4 Test manual mode boot block pre-fill

## 4. Supervisor Self-Registration

- [ ] 4.1 Add supervisor registration to `cmd_supervisor()` startup
- [ ] 4.2 Publish `agent.status` with `agent_id = "supervisor"`
- [ ] 4.3 Ensure supervisor appears in dashboard agent table
- [ ] 4.4 Test supervisor self-registration flow

## 5. Configuration and Documentation

- [ ] 5.1 Add boot block configuration options (default: enabled)
- [ ] 5.2 Update supervisor skill template with boot instructions
- [ ] 5.3 Add help text documentation
- [ ] 5.4 Update AGENTS.md with boot-prompt standard

## 6. Testing and Validation

- [ ] 6.1 Test with multiple agent types (Claude, etc.)
- [ ] 6.2 Verify boot blocks work in both supervisor and manual modes
- [ ] 6.3 Test template substitution edge cases
- [ ] 6.4 Validate no regressions in existing functionality
- [ ] 6.5 Run full test suite and fix any failures