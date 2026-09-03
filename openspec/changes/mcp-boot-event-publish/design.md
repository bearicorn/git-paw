## Context

git-paw has two channels to an agent. The **push** channel is file-based — the injected
sidecar and the boot block — and is the only way to put coordination discipline in front
of an agent at turn zero; every supported CLI reads some markdown instruction file. The
**pull** channel is the MCP server, which is genuinely vendor-neutral and lets an agent
fetch current state without pasting prose into context.

The asymmetry this change addresses: the *read* path is universal, but the *write* path
(agent → broker) is a shell helper, so every agent needs prompt-free shell execution at
t=0. That single requirement is what motivates permission seeding, curl allowlists, and
classifier special-cases for the bundled helper paths.

MCP is spoken by Claude Code, Codex, Gemini CLI and Cursor. aider is not an MCP client.
So MCP cannot replace the shell path — it can only make it unnecessary for the CLIs that
do speak MCP.

## Goals / Non-Goals

**Goals:**

- Give MCP-capable agents a write path that needs no shell permission at all.
- Keep the four-event contract identical across both invocation forms, so the
  coordination model does not fork by CLI.
- State the read/write boundary of the MCP server explicitly, since "read-only" has been
  a load-bearing property of that surface until now.

**Non-Goals:**

- Replacing `broker.sh`. It remains the fallback and the only path for non-MCP CLIs.
- A generic publish tool. Not now, and not later as a convenience — see D2.
- Exposing supervisor verbs (`agent.verified`, `agent.feedback`) over MCP.
- Any other MCP mutation: no file writes, no git mutations, no config writes.
- Changing the broker wire format. The tools publish existing variants.

## Decisions

**D1 — Exclude the authority verbs, by requirement rather than by omission.**
`agent.verified` is what authorises a merge. A coding agent that could publish it would
self-verify and bypass the five-gate framework — the single most valuable thing an agent
could do to escape supervision, and therefore the thing most worth making structurally
impossible. `agent.feedback` is excluded on the same logic: it is the supervisor's
corrective channel, and an agent able to forge it could suppress or fabricate correction.
Both exclusions are specified with adversarial scenarios so they are tested, not merely
intended.

**D2 — Fixed tool set, never a generic publish.** *Considered and rejected:* one
`publish(type, payload)` tool, which is less code and mirrors `/publish` exactly. Rejected
because it makes D1 a runtime string check rather than a structural property — the moment
the type is a parameter, "which types are allowed" becomes validation logic that can be
wrong, bypassed, or relaxed later. Four narrow tools cannot express `agent.verified` at
all.

**D3 — Derive `agent_id`, never accept it.** The tools resolve the publishing identity
from the session's worktree branch via the existing `slugify_branch` conversion. An
`agent_id` parameter would let any agent publish as any peer — forging a peer's
`agent.artifact` could induce another agent to act on work that does not exist, and
forging `agent.blocked` could stall a wave. Absent from the schema, it cannot be sent.

**D4 — Boot block chooses form by CLI capability, defaulting to the helper.** Unknown
capability resolves to the shell form, because a boot block that names tools a CLI cannot
call strands the agent at t=0 — the exact failure GP-01/GP-02 produced. The safe default
is the path that always works.

**D5 — Wire-identical messages across both forms.** Both paths publish the same
`BrokerMessage` shapes, so the broker, conflict detector, dashboard and learnings
aggregator cannot tell which path an event arrived by, and no consumer needs a per-path
branch.

## Risks / Trade-offs

- **The MCP server gains a mutation surface it never had.** This is the real cost, and it
  is why D1–D3 are specified adversarially rather than left to implementation care. →
  Bounded by construction: four tools, agent-scoped identity, authority verbs absent.
- **Two boot-block forms means two things to keep in sync.** → D5 plus the spec scenario
  requiring both forms to carry exactly the same four events; a divergence is a test
  failure, not a latent inconsistency.
- **Pressure to add "just one more" tool later.** Each addition erodes the bounded-surface
  argument. → The boundary is a spec requirement with scenarios, so widening it requires
  an explicit spec change and its own review rather than a quiet commit.
- **An agent may have both paths available and use them inconsistently.** → Harmless by
  D5: the published messages are indistinguishable downstream.
