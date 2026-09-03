## Why

git-paw's agent→broker write path is shell: the boot block instructs each agent to invoke
`.git-paw/scripts/broker.sh` for its four runtime events (register, done, blocked,
question). That works, but it means every agent must be able to execute a shell helper
without a permission prompt — which is the entire reason permission seeding, curl
allowlists and classifier special-cases exist around the bundled helper paths.

The MCP server (`git paw mcp`) is already the one genuinely vendor-neutral surface
git-paw exposes: stdio JSON-RPC, spoken by Claude Code, Codex, Gemini CLI and Cursor
alike. But it is **read-only** — specs, tasks, coordination state, git context,
documentation. So the read path is universal while the write path is shell-and-
permissions, and the awkward half is the one every agent must traverse at t=0.

Adding publish tools for exactly the four boot events makes the write path vendor-neutral
for any MCP-capable CLI, and shrinks that CLI's boot block to "connect to the git-paw MCP
server and call `publish_status`". aider does not speak MCP, so the shell path remains —
both are needed, and this change is explicit that neither replaces the other.

## What Changes

- Add MCP tools covering **exactly the four agent boot events**: register/status, done/
  artifact, blocked, and question.
- Each tool publishes as the **caller's own resolved agent id**. A tool SHALL NOT accept
  a caller-supplied `agent_id`, so an agent cannot publish as a peer.
- **The supervisor verbs are not exposed.** `agent.verified` and `agent.feedback` are
  authority messages — `agent.verified` is what authorises a merge. Exposing them over a
  tool surface an agent can call would let a coding agent self-verify and bypass the
  five-gate framework entirely. They stay off the MCP write surface.
- The MCP server stops being *exclusively* read-only, and the boundary is stated
  explicitly rather than left implied: a bounded publish set, agent-scoped, with the
  authority verbs excluded by requirement.
- The boot block may express the four events as MCP tool calls for a CLI that speaks MCP,
  falling back to the `broker.sh` helper otherwise. The four-event contract itself is
  unchanged — only how they are invoked.

**Not breaking:** the `broker.sh` helper path is retained unchanged and remains the
default for CLIs without MCP. Existing boot blocks continue to work.

## Capabilities

### New Capabilities

- `mcp-agent-publish`: the bounded MCP publish tool set for the four agent boot events —
  its tools, the agent-identity rule, and the explicit exclusion of the supervisor
  authority verbs.

### Modified Capabilities

- `mcp-server`: the server is no longer exclusively read-only. It advertises a bounded,
  agent-scoped publish category alongside the read-only tools; every other tool category
  remains read-only and deterministically sourced.
- `boot-block`: the four runtime events MAY be expressed as MCP tool invocations when the
  target CLI speaks MCP, with the `broker.sh` helper form as the fallback. The
  requirement that there are exactly four events, and which four, is unchanged.

## Impact

- **Code:** `src/mcp/` — a publish tool category alongside the existing read-only ones;
  agent-identity resolution (reuse the existing branch→`agent_id` `slugify_branch`
  conversion rather than trusting an argument). `src/skills.rs` `build_boot_block` — the
  MCP variant of the four events. `src/broker/` — no wire-format change; the tools
  publish the same `BrokerMessage` shapes the helper does.
- **Enum-variant ripple: none.** All four events are existing `BrokerMessage` variants
  (`Status`, `Artifact`, `Blocked`, `Question`). No variant is added or removed.
- **Security surface — this is the substantive review point.** The MCP server currently
  cannot mutate anything; after this it can publish. The mitigations are: a fixed tool
  set (no generic publish), agent-scoped identity (no caller-supplied `agent_id`), and
  exclusion of `agent.verified`/`agent.feedback`. This change SHALL NOT introduce a
  generic "publish arbitrary BrokerMessage" tool, now or as a convenience later.
- **Interaction with `remove-inert-permission-seeding`:** that change removes inert
  allowlist seeding; this one reduces how much an MCP-capable agent needs shell access at
  all. Complementary, independently landable.
- **Docs:** the MCP tool reference gains the publish category and must state the
  read-only/write boundary; the boot-block chapter documents the MCP expression and its
  fallback; `--help` unchanged.
