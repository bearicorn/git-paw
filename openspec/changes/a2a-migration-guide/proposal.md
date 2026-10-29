## Why

MILESTONE v1.2.0 item 5 asks for a migration guide from the "proto" protocol to formal A2A. The
cycle's design changes what that guide has to say. The legacy wire is not replaced: it remains the
agent-side transport, and A2A is an opt-in external face. A reader who comes with the MILESTONE
framing ("swap custom JSON", `/.well-known/agent.json`, per-agent cards) needs a guide that maps
each existing concept to its A2A counterpart, and says honestly what stays legacy.

Three more documents are needed:

- The coordination extension's URI has to resolve to its specification.
- Users need a chapter on how to enable and use the face.
- MILESTONE item 6, the MCP HTTP+SSE transport, needs a recorded decision. This change records
  that it is dropped, and why.

## What Changes

- **New A2A chapter**, `docs/src/user-guide/a2a.md`. It covers:
  - enabling the face, and the loopback-only rule;
  - getting the credential (`git paw __authority`), and the required `A2A-Version: 1.0`;
  - discovery at `<broker_url>/.well-known/agent-card.json`;
  - the task model, with one task per worker and the state table;
  - the supported and unsupported methods, and their errors;
  - blocking `SendMessage` and `returnImmediately`;
  - verdicts through the coordination extension;
  - streaming, when it is served;
  - worked JSON-RPC examples.
- **New migration guide**, `docs/src/user-guide/a2a-migration.md`. It covers:
  - a mapping table from every legacy endpoint and message type to its A2A counterpart, or to
    "stays legacy";
  - how an orchestrator moves from `sweep.sh` verdict verbs and `/status`/`/log` polling to A2A;
  - what workers keep doing: `broker.sh`, MCP publish, nothing changed;
  - the legacy-support policy, which is stable for the rest of the 1.x line and not deprecated;
  - the corrected facts: the card path, one card per broker, and the full eight-state lifecycle.
- **Extension specification page**, `docs/src/a2a/coordination/v1/index.md`, published at the
  extension URI `https://bearicorn.github.io/git-paw/a2a/coordination/v1/`. It gives the metadata
  schema, the event values, the verdict semantics and the versioning rule.
- **Cross-links.**
  - `coordination.md` states that the legacy wire is the agent-side transport, and links to the A2A
    chapter.
  - `mcp.md` states that `git paw mcp` is served over stdio only, and points HTTP-based agent
    integrations at the A2A face.
  - README gets a one-line feature entry.
- **The MCP HTTP transport item is dropped from v1.2.0.** The reasons are in design D3.

## Capabilities

### New Capabilities

<!-- None. `broker-a2a` is introduced by `a2a-wire-model`. -->

### Modified Capabilities

- `broker-a2a`: adds documentation requirements for the A2A chapter, the migration guide, the
  extension specification page, and the cross-links from the coordination and MCP chapters.

## Impact

- **Docs:**
  - New: `docs/src/user-guide/a2a.md`, `docs/src/user-guide/a2a-migration.md` and
    `docs/src/a2a/coordination/v1/index.md`.
  - Updated: `docs/src/SUMMARY.md`, `docs/src/user-guide/coordination.md`,
    `docs/src/user-guide/mcp.md`, `README.md`, and the specifications index entry for `broker-a2a`.
- **Tests:** a new `tests/a2a_docs_content.rs`. It asserts stable anchors (headings and required
  terms) and never exact prose, per `test-strategy`.
- **Code:** none.
- **Enum-variant ripple:** none.
- **Dependencies:** none.
- **Cross-version dependencies:**
  - Documents `a2a-wire-model`, `a2a-task-lifecycle`, `a2a-agent-card`, `a2a-legacy-coexistence`
    and, if it is approved, `a2a-task-streaming`.
  - References the v0.18.0 `git paw __authority` and the v1.0.0 stability chapter.
  - Must land last in the cycle.
- **Export agnosticism:** none of these are bundled assets, and the extension page is git-paw's own
  protocol documentation.
