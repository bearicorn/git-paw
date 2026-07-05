## Context

Consistency-sweep finding #8: the `git paw init` template (`generate_default_config`) omits commented examples for `[mcp]`, `[layout]`, `[broker.watcher]`, `[supervisor.auto_approve]`, `[supervisor.learnings_config]`, and `[governance]`, which the configuration reference documents. A user reading the generated template as the schema misses these knobs.

## Goals / Non-Goals

**Goals:** the init template documents every supported config section (commented), matching the configuration reference.
**Non-Goals:** no default or behavior change (all additions stay commented); no new config field is introduced.

## Decisions

- **D1 — Add the missing sections as COMMENTED example stanzas** (illustrative values, not active), so an untouched generated config behaves identically to today. Rationale: discoverability without changing defaults or breaking existing configs.
- **D2 — Treat `docs/src/configuration` as the canonical section list**, and add a parity test so the template and the reference cannot drift apart again.

## Risks / Trade-offs

- The template grows longer → acceptable: it is commented and improves discoverability. The parity test prevents future divergence.
