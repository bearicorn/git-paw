## Why

MILESTONE v1.2.0 item 4 says "curl templates still work for CLIs without A2A". This change answers
three questions about that path: how long it keeps working, how it would ever be deprecated, and
what the A2A layer may install. Two facts settle the answer.

1. **The v1.0.0 contract.** `core-stability-contract` (v1.0.0 `cli-contract-freeze`) makes these
   stable entries:
   - the broker's HTTP endpoints and message types;
   - the bundled helper scripts' install paths and documented subcommands.

   A stable entry can be removed only at a major version, and only after it has been deprecated for
   at least two minor releases. v1.2.0 is a minor release and nothing on that path is deprecated,
   so it cannot remove any of it.
2. **The protocol.** A2A has no operation for an agent to publish its own status or to broadcast.
   (See `a2a-wire-model` D1.) No agent CLI that git-paw supports is known to act as an A2A client.
   This is to be verified per CLI against the v0.23.0 support matrix.

So `broker.sh`, `sweep.sh` and the legacy wire are not a compatibility shim. They are the agent-side
transport, and A2A is the external face. The contract needs to say so, record the new A2A entries,
and add the A2A layer to the v1.0.0 layer model without letting it leak into worktrees.

## What Changes

- **The legacy wire and the helpers stay stable for the rest of the 1.x line, and are not
  deprecated.** From 1.2.0, the major-1 contract manifest keeps:
  - all five legacy broker endpoints;
  - every broker message type;
  - every bundled helper's install path and documented subcommands.

  None of these is entered in the deprecation registry. The stability chapter states their role
  and the only route by which they could be removed: a registry entry, at least two warned minor
  releases, and removal at a later major. That route applies only once an agent-side replacement
  exists.
- **The A2A entries become stable in 1.2.0.** These are `[broker.a2a] enabled`, `POST /a2a`,
  `GET /.well-known/agent-card.json`, the declared protocol version `1.0`, and the coordination
  extension URI v1.
- **A2A becomes a layer in the v1.0.0 layer model.**
  - It is enabled by `[broker.a2a] enabled = true`.
  - It requires Broker, but does not imply it. With Broker off, the setting is ignored, and git-paw
    prints one notice line.
  - Its surface is only the A2A routes and the Agent Card on the broker host. It installs nothing in
    worktrees, managed blocks, skills, boot blocks, prompts, hooks, scripts or the session
    environment.
  - The CI layer matrix gains a Broker + A2A entry, and the modes chapter documents the layer.
- **No BREAKING change.** Sessions with A2A disabled are unchanged, and so are agent-side surfaces
  with A2A enabled.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `core-stability-contract` (introduced by v1.0.0 `cli-contract-freeze`): adds a requirement that
  the legacy broker wire and helpers stay stable for the rest of major 1, and a requirement that
  records the A2A entries.
- `core-layer-isolation` (introduced by v1.0.0 `layer-isolation`): adds the A2A layer, its
  dependency on Broker, and its host-only surface.

## Impact

- **Code:**
  - `src/layers.rs`, from v1.0.0 `layer-isolation`: `LayerSet` gains `A2a`. `resolve_layers` gains
    the `[broker.a2a]` input, the Broker requirement and the ignored-setting notice. `SurfacePlan`
    gains the host-only A2A surface.
  - The `__broker` host reads the resolved layer set rather than `[broker.a2a] enabled` directly.
  - The contract manifest generator gains the A2A entries.
- **Tests:**
  - Unit rows for the resolver and the plan.
  - A new CI layer-matrix entry for Manual + Broker + A2A.
  - Updated manifest guard fixtures.
  - The modes-chapter parity test.
- **Docs:**
  - `docs/src/stability.md`: the roles of the legacy wire and A2A, and the A2A entries.
  - `docs/src/user-guide/modes.md`: the A2A layer.
- **Enum-variant ripple:** none in `BrokerMessage` or `SpecBackendKind`. `LayerSet`'s match sites
  in `src/layers.rs` change, and the compiler enumerates them.
- **Dependencies:** none.
- **Cross-version dependencies:**
  - Builds on v1.0.0 `cli-contract-freeze` (`core-stability-contract`, the manifest, the
    deprecation registry) and v1.0.0 `layer-isolation` (`core-layer-isolation`, `src/layers.rs`,
    the CI matrix, the modes chapter).
  - Requires `a2a-task-lifecycle`, which owns the config field and the serving conditions, and
    `a2a-agent-card`, which owns the card route.
  - Both capabilities are introduced by pending v1.0.0 changes. These deltas are ADDED requirements
    only, so they need a rebase only if those changes rename the capabilities.
