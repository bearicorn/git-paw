## Why

Agents repeatedly reached for shell pipelines to read their own source when their CLI has
native file-read and search tools that need no approval at all. Observed across all three
dogfood agents:

- `grep -n "^}" src/main.rs | awk -F: '$1 > 3040 && $1 < 3120'`
- `grep -n "missing field" <(cargo test --lib --no-run 2>&1)`
- `awk '/AgentPane {/{start=NR}…' src/supervisor/drive.rs`

Each stalled the agent on a permission prompt. Each auto-approval wrote a permanently dead
allowlist entry — `awk -F: '$1 > 3040 && $1 < 3120'` will never match another command. A
second habit compounds it: agents wrote numbered scratch files (`build_errors.txt`,
`build_errors2.txt`, … `build_errors5.txt`), and because a grant is keyed on the exact
path, every iteration re-prompted and added another dead grant. One agent took five
consecutive approvals for the same build-and-count loop before being redirected.

The bundled worker-orientation guidance covers permissions and worktree orientation but
says nothing about *how to read a file*. This is the one finding in the v0.17.0 dogfood
that is genuinely skill-shaped: no code change can stop an agent choosing `awk` over its
own read tool, and the fix is a short paragraph.

## What Changes

- The embedded coordination skill SHALL instruct the agent to prefer its own CLI's
  file-read and search facilities over shell text-extraction pipelines when inspecting
  source, and SHALL explain why: shell pipelines stall on approval and produce allowlist
  entries that never match again.
- It SHALL instruct the agent to reuse a single scratch path when capturing command output
  across iterations, rather than numbering a fresh file each time.
- The guidance SHALL be CLI-agnostic. It SHALL NOT name any specific CLI's tools, because
  the skill is an exported asset and naming one vendor's tooling breaks it for every other
  consumer.

## Capabilities

### New Capabilities

<!-- none -->

### Added Capabilities

- `skill-agent`: a requirement that the embedded coordination skill teaches file-reading
  and scratch-file discipline, CLI-agnostically.

## Impact

- `assets/agent-skills/coordination.md` — one new section.
- `tests/` — the skill-content assertions that pin the embedded skill's required sections
  (the same pattern used for the existing coordination-skill requirements).
- **Export-agnosticism**: this is a bundled asset, so AGENTS.md's export rule applies in
  full. The wording must describe a *capability class* ("your CLI's own read and search
  tools"), never an instance. Naming Claude Code's `Read`/`Grep` would ship a
  vendor-specific instruction to every consumer.
- **Delta coordination**: `skill-agent` is not touched by any other in-flight v0.17.0
  change.
