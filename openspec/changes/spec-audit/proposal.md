## Why

During the v0.3.0 build, a manual spec-to-code audit caught 4 critical issues that all 7 coding agents missed: wrong field names, incomplete slugify_branch, missing tests for spec scenarios. This audit was done by a human reading every spec scenario and grepping the codebase — exactly the kind of work an AI supervisor can automate. This change adds spec audit instructions to the supervisor's skill template so the supervisor performs this check automatically after each agent reports done.

## What Changes

- Add a "Spec Audit" section to `assets/agent-skills/supervisor.md` containing step-by-step instructions for the supervisor to:
  1. Locate the agent's spec files at `openspec/changes/<change-name>/specs/`
  2. For each `#### Scenario:` block, extract the WHEN/THEN assertions
  3. Search the codebase for a matching test (grep for key assertions from the scenario)
  4. Read the implementation file and verify struct fields, function signatures, and behavior match the spec's SHALL/MUST requirements
  5. Compile a list of gaps (untested scenarios, field mismatches, missing implementations)
  6. If gaps exist: publish `agent.feedback` with the gap list
  7. If no gaps: include "spec audit clean" in the `agent.verified` message

This is purely template content — no Rust code changes.

## Capabilities

### New Capabilities

<!-- None -->

### Modified Capabilities

- `agent-skills`: Extend supervisor.md with a spec audit workflow section

## Impact

- **Modified file:** `assets/agent-skills/supervisor.md` — add spec audit section
- **No Rust code changes, no new modules, no new dependencies.**
- **Depends on:** `supervisor-skill` (supervisor.md must exist), `supervisor-messages` (audit results published as verified/feedback)
- **Dependents:** `supervisor-agent` (the supervisor uses these instructions at runtime)
