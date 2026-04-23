## Why

The v0.4.0 supervisor agent needs instructions telling it what to do — monitor agents, run tests, publish verified/feedback, detect file conflicts, determine merge order, escalate to the human. This is fundamentally different from the coding agents' `coordination.md` which teaches them curl commands. The supervisor's instructions are a separate skill template: `supervisor.md`.

## What Changes

- Add `assets/agent-skills/supervisor.md` containing the supervisor's instruction set:
  - Role definition: monitor and verify, do NOT write code
  - How to poll `/status` and `/messages/supervisor` for agent updates
  - How to capture a baseline test count before agents start (run test command on clean branch)
  - How to run the test command after an agent reports done and compare against baseline
  - How to detect regressions (previously-passing tests now failing → agent.feedback)
  - How to publish `agent.verified` (no regressions, all tests pass) and `agent.feedback` (regressions or failures)
  - How to detect file conflicts by checking `modified_files` overlap across agents
  - How to determine merge order (merge agents with no dependents first)
  - When to escalate to the human (conflicts, architectural decisions, ambiguous failures)
- Extend `embedded_default()` in `src/skills.rs` with a second match arm: `"supervisor"` → `include_str!("../assets/agent-skills/supervisor.md")`
- Add `{{PROJECT_NAME}}` as a new substitution placeholder in `render()` alongside `{{BRANCH_ID}}`
- User can override the supervisor skill by placing `supervisor.md` in `~/.config/git-paw/agent-skills/`

## Capabilities

### New Capabilities

<!-- None -->

### Modified Capabilities

- `agent-skills`: Add embedded `supervisor` skill alongside `coordination`. Add `{{PROJECT_NAME}}` placeholder substitution to `render()`.

## Impact

- **New file:** `assets/agent-skills/supervisor.md` (embedded skill content)
- **Modified file:** `src/skills.rs` — add match arm in `embedded_default()`, add `{{PROJECT_NAME}}` substitution in `render()`
- **No new modules, no new dependencies.**
- **Depends on:** `supervisor-messages` (the template references `agent.verified` and `agent.feedback` which must exist in the broker)
- **Dependents:** `supervisor-agent` (calls `skills::resolve("supervisor")` and renders it for the supervisor's AGENTS.md)
