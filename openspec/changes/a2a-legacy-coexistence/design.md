## Context

v1.0.0 freezes the following as part of its stable surface:

- the broker's endpoints and message types;
- the helper scripts' install paths and documented subcommands, such as `broker.sh status`,
  `broker.sh poll`, `sweep.sh verified` and the rest.

v1.0.0 also defines a deprecation registry. An item is removable only at a major, after at least
two warned minor releases. The v1.0.0 layer model (`core-layer-isolation`) has these layers:
Manual, Skills, Broker, Supervised and Unattended. It does not include A2A, although the original
seven-layer list in MILESTONE named A2A as layer 7.

## Goals / Non-Goals

**Goals:**

- A definite answer on how long the legacy path works, and how it could ever be deprecated.
- The A2A entries recorded in the major-1 contract from 1.2.0.
- A2A fitted into the layer model without any agent-side leak.

**Non-Goals:**

- Deprecating or removing anything in v1.2.0.
- Removals from the deprecation registry. None is scheduled for 2.0.0: the supervisor vocabulary
  (including the `supervisor` skill alias) is removed at 1.0.0 by `remove-supervisor-vocabulary`,
  and every pre-1.0 deprecation is resolved at the freeze. See Open Questions.

## Decisions

### D1: Do not deprecate the legacy wire or the helpers

Deprecation means a removal is planned. Removing this path would require an agent-side replacement,
and A2A offers none:

- A2A has no agent-originated publish operation and no broadcast.
- Coding CLIs drive tools by running shell commands or calling MCP tools. They do not act as A2A
  clients.

Marking the path deprecated would warn on every helper call and would promise a removal that cannot
happen. So the path stays stable for the rest of the 1.x line.

If a future major ever replaces the agent-side transport, the route is:

1. a registry entry in some later minor release;
2. warnings from the v1.0.0 warn-helper for at least two minor releases;
3. removal at v2.0.0 or later.

### D2: A2A requires Broker but does not imply it

A2A has no host without the broker, so the A2A layer depends on Broker. Two options were
considered:

- **(a) Imply Broker**, as Supervised does. Rejected. Turning on A2A would then install the whole
  Broker surface (hooks, `broker.sh`, skills, boot blocks) into every worktree. A flag that names
  an external protocol face should not change what agents see.
- **(b) Require Broker and ignore the setting with a notice when Broker is off.** Chosen. This is
  least surprise, and it matches layer isolation's "installs exactly its declared surface" rule.

### D3: The A2A surface is host-only

The A2A plan contains two things: the A2A routes and the Agent Card route on the broker host. It
contains no file, hook, environment variable, skill text or prompt text. The CI matrix entry for
Manual + Broker + A2A therefore asserts that the worktree artifacts are byte-identical to the
Manual + Broker entry, and that the broker host answers the card route.

### D4: Manifest entries

The new stable entries are:

- the configuration key `[broker.a2a] enabled`;
- the endpoints `POST /a2a` and `GET /.well-known/agent-card.json`;
- the declared A2A protocol version `1.0`;
- the extension URI `https://bearicorn.github.io/git-paw/a2a/coordination/v1`.

A2A's own data shapes are A2A's contract, not git-paw's. git-paw's promise is that it serves A2A
`1.0`, and the extension's metadata schema is versioned by its URI.

## Risks / Trade-offs

- **The MILESTONE reader expects "swap", and finds "two faces, legacy kept".**
  → The stability and migration chapters explain the reason in a few lines, citing the protocol
  facts.
- **The pending v1.0.0 changes rename the capabilities.**
  → These deltas are ADDED requirements only, so a rebase means editing a folder name.

## Migration Plan

Nothing migrates. Existing sessions behave identically, and enabling A2A is opt-in.

## Open Questions

- **Release number (decided).** On 2026-10-19 the user decided to ship A2A as v1.2.0, a minor
  release, because the cycle is additive.
- **A future agent-side A2A path.** If agent CLIs ever become A2A clients, revisit D1.
