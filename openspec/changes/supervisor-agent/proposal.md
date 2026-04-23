## Why

The v0.4.0 Wave 1 changes added config, messages, skill template, and spec audit instructions. But nothing actually launches the supervisor or starts agents. This change implements the auto-start flow: the supervisor reads specs, creates the tmux session, launches all coding agents with approval flags, injects their prompts via `tmux send-keys`, and begins monitoring via the broker.

Key learnings from v0.3.0 and v0.4.0 Wave 1 dogfooding:
- Agents need permission-bypassing flags injected at launch (`agent_approval` config)
- Agents must be instructed to publish status proactively (when starting, editing files, committing) not just when blocked
- Inter-agent rules (file ownership, never push, match spec exactly) must be injected into each agent's AGENTS.md
- The coordination.md template must include cherry-pick instructions for inter-agent dependencies
- The supervisor monitors via broker polling, not tmux capture-pane

## What Changes

- Implement `cmd_supervisor()` in `src/main.rs` — the handler for `--supervisor` mode:
  1. Load config, resolve supervisor CLI from `[supervisor]` config
  2. Scan specs via `--from-specs` or resolve branches from flags
  3. Create worktrees for each branch (with `-b` fallback)
  4. Generate per-worktree AGENTS.md with: spec content + file ownership + coordination skill + inter-agent rules
  5. Build tmux session: pane 0 = dashboard, panes 1-N = coding agents
  6. Inject `GIT_PAW_BROKER_URL` via `tmux set-environment`
  7. For each agent pane: construct CLI launch command with approval flags from `approval_flags(cli, level)`
  8. Execute tmux session in detached mode (supervisor stays in foreground terminal)
  9. Wait for all panes to boot (~2s), then inject initial prompt via `tmux send-keys`
  10. Start the supervisor CLI in the foreground with the supervisor skill template as its AGENTS.md

- Add inter-agent rules to the generated AGENTS.md coordination section:
  - File ownership is exclusive — don't touch other agents' files
  - Commit when done, never push
  - Publish `agent.status` when starting work, editing/creating files, and after each commit
  - Check `modified_files` from peer status messages for conflict detection
  - When blocked on a peer, publish `agent.blocked` and cherry-pick the peer's commit when it arrives
  - Match spec field names exactly

- Update `coordination.md` to strengthen proactive status publishing:
  - "You MUST publish agent.status after each commit"
  - "You MUST publish agent.status when you start working on a new file"
  - Add cherry-pick instructions for receiving peer artifacts

## Capabilities

### New Capabilities

- `supervisor-launch`: The auto-start flow that creates the tmux session, launches agents with approval flags, injects prompts, and starts the supervisor CLI in the foreground.

### Modified Capabilities

- `agent-skills`: Update coordination.md with proactive status publishing, inter-agent rules, and cherry-pick instructions
- `worktree-agents-md`: Generated AGENTS.md now includes inter-agent coordination rules section

## Impact

- **Modified files:** `src/main.rs` (add `cmd_supervisor`), `src/agents.rs` (add inter-agent rules to generated section), `assets/agent-skills/coordination.md` (strengthen instructions)
- **Depends on:** `supervisor-config` (reads config), `supervisor-skill` (loads supervisor template), `supervisor-messages` (supervisor publishes verified/feedback)
- **No new dependencies.**
