## Context

`SpecBackendKind` has four variants and `backend_for_type` maps all four type strings. The MCP
server never consults either: it keeps its own three-string list for startup validation and
its own three-arm string match for `get_skill`. Both were correct before the Superpowers
backend existed, and nothing forced them to change when it was added.

## Goals / Non-Goals

**Goals:**

- `git paw mcp` starts and serves in a Superpowers repository.
- One source of truth for which spec types exist, consulted by the MCP server.
- Make the next backend addition impossible to miss in `src/mcp/`.

**Non-Goals:**

- Making `SpecBackend` a full strategy object across the codebase (v0.19.0).
- Changing how Superpowers specs are discovered or rendered.

## Decisions

**Derive, don't list.** Replace `VALID` with a check against the same mapping spec discovery
uses — either by attempting `backend_for_type`, or by parsing into `SpecBackendKind`. Either
way the server accepts exactly what discovery supports.

**Prefer an exhaustive match in `get_skill`.** Convert the string to `SpecBackendKind` once
and `match` on the enum. A future variant then fails to compile at this site instead of
silently rendering the no-backend sentinel — moving it into §7's compile-forcing list rather
than merely documenting it.

**Document the remaining string sites in AGENTS.md §7.** Anything that must stay a string
dispatch is listed, so the ripple is scoped up front.

## Risks / Trade-offs

- **Error wording changes.** The invalid-type error should list the valid values from the
  same source, so its message also stays current. Tests that pinned the old three-value
  message need updating — intentionally.
- **Low blast radius.** The change only widens what the server accepts to what the rest of
  git-paw already supports.
