## Why

`git paw mcp` refuses to start in any repository configured for the Superpowers spec format:

```
$ git paw mcp      # in a repo with [specs].type = "superpowers"
error: MCP: invalid [specs].type = "superpowers" in .git-paw/config.toml.
       Valid values: openspec, markdown, speckit.
(exit 1)
```

Reproduced against the current build in a throwaway repository. The MCP server is therefore
entirely unavailable for one of the four spec formats git-paw supports — not merely missing a
listing, as first reported.

Three sites in `src/mcp/` dispatch on the spec-type **string** with a hardcoded set that
predates the Superpowers backend:

- `src/mcp/server.rs` `validate_startup_config` — `const VALID: [&str; 3]`, which rejects
  `superpowers` and makes the server exit non-zero.
- `src/mcp/tools/project.rs` `get_skill` — the match that picks the backend for the
  spec-path doctrine has no `superpowers` arm, so it renders the no-backend sentinel.
- `src/mcp/query/specs.rs` — a doc comment still lists three backends.

The canonical dispatch, `backend_for_type` in `src/specs/mod.rs`, has all four. The MCP
server spec drifted too: *Project knowledge tools* still lists backends as `openspec |
markdown | speckit`.

This is the `SpecBackendKind` ripple AGENTS.md §7 warns about — but these three sites are
**string** dispatches, not exhaustive `match`es on the enum, so the compiler never flagged
them, and §7's list of sites names only the compile-forcing ones. That is exactly how they
were missed when Superpowers was added.

## What Changes

- The MCP server's set of valid `[specs].type` values is derived from the same source spec
  discovery uses, so a supported backend can never be rejected and the two cannot drift.
- `get_skill` resolves the Superpowers backend for the spec-path doctrine.
- The stale three-backend doc comment is corrected.
- AGENTS.md §7's `SpecBackendKind` list gains the non-compile-forcing string-dispatch sites,
  so the next backend addition scopes them up front.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `mcp-server`: *Hard errors only for malformed configuration* (valid types derive from spec
  discovery; every supported type is accepted) and *Project knowledge tools* (four backends,
  Superpowers served).

## Impact

- `src/mcp/server.rs`, `src/mcp/tools/project.rs`, `src/mcp/query/specs.rs`.
- Possibly `src/specs/mod.rs` — expose the supported type names (or a `SpecBackendKind`
  parse) so `src/mcp/` stops keeping its own list.
- `AGENTS.md` §7.
- **Delta coordination**: `mcp-server` is not touched by any other in-flight change.
