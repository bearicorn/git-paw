## Why

The supervisor-agent change handles launching agents and the supervisor. This change handles the CLI integration: the `--supervisor` flag, the config-based resolution chain with interactive prompting, merge ordering, and purge safety. It's the user-facing entry point to supervisor mode.

Key learnings incorporated:
- `--supervisor` flag should be optional if `[supervisor] enabled = true` in config
- If no `[supervisor]` section, prompt the user "Start in supervisor mode?" during `git paw start`
- `git paw init` should ask about supervisor mode and write the section (preventing future prompts)
- `git paw purge` must warn about unmerged commits in worktree branches before destroying them
- Merge ordering: the supervisor determines safe order from agent dependency signals (merge no-dependents first)

## What Changes

- Add `--supervisor` flag to the `start` subcommand in `src/cli.rs`
- Implement the supervisor mode resolution chain in `src/main.rs`:
  1. `--supervisor` flag → enable (no prompt)
  2. `[supervisor] enabled = true` in config → enable (no prompt)
  3. `[supervisor] enabled = false` in config → disable (no prompt)
  4. No `[supervisor]` section → prompt "Start in supervisor mode? (y/n)"
  5. `--dry-run` → assume no supervisor (skip prompt)
- Update `git paw init` to prompt for supervisor configuration:
  - "Enable supervisor mode by default? (y/n)"
  - If yes: "Test command (e.g. 'just check', leave empty to skip):"
  - Write `[supervisor]` section to config
- Update `git paw purge` to check for unmerged commits:
  - For each worktree branch, check `git log <branch> --not main --oneline`
  - If any branch has unmerged commits: warn "N branches have unmerged commits. Purging is irreversible."
  - Require `--force` or confirmation to proceed
- Implement merge ordering logic in the supervisor's workflow:
  - Read dependency signals from broker (which agents published `agent.blocked` referencing which peers)
  - Build a dependency graph: agent A needs agent B → B merges first
  - Merge in topological order (no-dependents first)
  - After each merge: run test command, verify green

## Capabilities

### New Capabilities

- `supervisor-cli`: The `--supervisor` flag, resolution chain, init prompts, and purge safety

### Modified Capabilities

- `cli-parsing`: Add `--supervisor` flag to `start` subcommand
- `project-initialization`: Add supervisor prompts to `git paw init`

## Impact

- **Modified files:** `src/cli.rs`, `src/main.rs`, `src/init.rs`
- **Depends on:** `supervisor-config` (reads config), `supervisor-agent` (calls `cmd_supervisor`)
- **No new dependencies.**
