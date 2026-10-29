## Context

The A2A facts here come from `a2a-wire-model/design.md`, section "Sources":

- §8.1–§8.3 and §14.3 cover card discovery and the protocol declaration;
- §4.4 and the proto `AgentCard` and `AgentSkill` cover the fields;
- §8.6 covers caching.

In the proto, the required `AgentCard` fields are `name`, `description`, `supportedInterfaces`,
`version`, `capabilities`, `defaultInputModes`, `defaultOutputModes` and `skills`. `AgentSkill`
requires `id`, `name`, `description` and `tags`, and §5.7 says a required array must have at least
one element.

## Goals / Non-Goals

**Goals:**

- A standard client can discover the session at the well-known URI and configure itself from the
  card alone.
- The card declares exactly what `a2a-task-lifecycle` serves.

**Non-Goals:**

- Per-agent cards.
- Extended cards.
- Card signing. JWS would need a crypto dependency, and the card is served on loopback.
- A card catalog across repositories (v1.4.0).

## Decisions

### D1: One card per session broker, with capability skills

The alternatives considered:

- **(a) One card per worker** at non-standard paths, plus a catalog. Rejected:
  - only one card can sit at the well-known URI of an origin;
  - workers do not execute A2A requests;
  - every card would describe the same capability set;
  - workers are created and removed per wave, so the cards would churn and defeat §8.6 caching.
- **(b) One card with one skill per worker.** Rejected. Skills describe capabilities, not
  instances, and A2A 1.0's `SendMessageRequest` has no skill selector, so a per-agent skill could
  not address anything.
- **(c) A2A `tenant` per worker.** Rejected. It encodes identity into routing, and a client would
  need a different interface entry for each agent.
- **(d) One card, capability skills, workers as tasks.** Chosen. A worker assignment is a task, and
  task-centric addressing is native to A2A. `ListTasks` is the discovery mechanism for workers.

### D2: The skills

| id | Name | Description (abridged) | Tags |
|---|---|---|---|
| `observe-workers` | Observe workers | List and read the session's worker tasks: lifecycle state, status messages and committed artifacts | `git-paw`, `coordination`, `status` |
| `steer-worker` | Steer a worker | Send guidance to a worker's task, or cancel the task. The worker receives guidance as an authoritative answer | `git-paw`, `coordination`, `guidance` |
| `record-verdict` | Record a verdict | Record `verified` (completes the task) or `feedback` (errors; the worker resumes). Requires the coordination extension | `git-paw`, `verification` |

Each skill names its input and output modes. No skill carries a skill-level security requirement,
because the card-level requirement already applies to every operation.

### D3: The security declaration mirrors the lifecycle's authentication

The card declares:

```text
securitySchemes: { "pawSession": { "httpAuthSecurityScheme": { "scheme": "Bearer",
                     "description": "git-paw session credential; obtain with `git paw __authority`" } } }
securityRequirements: [ { "schemes": { "pawSession": { "list": [] } } } ]
```

The field is named `securityRequirements`, following the normative proto. The §8.5 sample's
`security` key is not used.

### D4: Cache headers

The card changes only when git-paw is upgraded, the configuration changes, or streaming becomes
available, so `max-age=60` is enough. The `ETag` is a strong hash of the serialized card, and
`If-None-Match` gets a `304` with an empty body.

## Risks / Trade-offs

- **A generic client tries a binding git-paw does not serve.**
  → `supportedInterfaces` lists only JSONRPC, and §8.3.2 requires clients to pick from the list.
- **The session name reveals the repository's directory name on loopback.**
  → It is already visible through `/status` and the tmux session list. That is acceptable, and the
  posture chapter states it.

## Migration Plan

This change is additive and gated by `[broker.a2a] enabled`. To roll back, revert or disable it.

## Open Questions

- A signed card when v1.4.0 adds remote access. It would need a JWS crate, which is a dependency
  decision for that cycle.
