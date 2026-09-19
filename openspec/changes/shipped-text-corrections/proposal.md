## Why

Text that ships to every agent — or every user — states things that are false. Each was found
by a documentation sweep and verified against the tree:

- **Boot blocks invite peer cherry-picking.** Both `assets/boot-block-template.md` and
  `assets/boot-block-mcp-template.md` describe `exports` as "public API items for peers to
  cherry-pick". v0.16.0 removed peer cherry-picking (it creates a second copy of a commit
  the supervisor later merges from the peer's branch, leaving the agent's branch
  unmergeable), and the coordination skill now prohibits it. The boot text, which every
  agent reads first, still invites it.
- **The MCP publish tools say the same.** `src/mcp/tools/publish.rs` (the `exports` field
  doc and the tool description) repeats "for peers to cherry-pick" in the schema shown to
  MCP-capable agents.
- **Task prompts name a message type that does not exist.** The Spec Kit consolidated
  footer and the Superpowers execution prompt tell agents to "Publish `agent.done`"; the
  broker defines no `agent.done` message.
- **A task prompt makes a release promise.** The Spec Kit validation-criteria prompt says
  "full enforcement is planned for v1.0.0".
- **An error message is stale.** `git paw add`/`remove` on a bare session says supervisor
  mode is "the default" and that the grid "is not re-tiled incrementally in v0.6.0"; the
  binary is 0.16.x.

## What Changes

- Reword the `exports` description in both boot templates and the MCP publish tools without
  cherry-picking.
- Replace `agent.done` in the task prompts with the mechanism that exists.
- Drop the version promise from the Spec Kit prompt.
- Correct the bare-session `add`/`remove` error text.

## Capabilities

### New Capabilities

<!-- none -->

### Added Capabilities

- `boot-block`: boot text does not invite peer cherry-picking.
- `mcp-agent-publish`: publish tool descriptions do not invite peer cherry-picking.
- `spec-providers`: task prompts name only real broker messages and promise no releases.

## Impact

- `assets/boot-block-template.md`, `assets/boot-block-mcp-template.md`,
  `src/mcp/tools/publish.rs`, `assets/task-prompts/speckit/*`,
  `assets/task-prompts/superpowers/execution.md`, `src/commands/helpers.rs`.
- Content tests that pin the old wording, if any, change with it.
- Text only; no behaviour change. Export-agnosticism applies to every asset touched.
- **Delta coordination**: `start-spec-launch-parity` modifies two *other* `boot-block`
  requirements; these are new requirements, so no collision.
