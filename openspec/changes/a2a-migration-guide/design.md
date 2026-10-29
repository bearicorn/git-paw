## Context

The facts the docs restate come from the cycle's sources, listed in `a2a-wire-model/design.md`
under "Sources". The mapping table is `a2a-wire-model` design D2.

The docs follow the four-layer rule in `doc-completeness`:

- `--help` is unaffected, because A2A adds no CLI surface.
- README gets a feature line.
- mdBook gets the chapters below.
- The configuration reference already gained `[broker.a2a]` in `a2a-task-lifecycle`.

## Goals / Non-Goals

**Goals:**

- A reader can enable A2A and drive a session with a generic A2A client from the chapter alone.
- An existing orchestrator author can map every legacy concept they use to A2A, or learn that it
  stays legacy.
- The extension URI resolves to its specification.

**Non-Goals:**

- Re-documenting the A2A specification. The docs link to it.
- Any change to the bundled skills. Workers' instructions are unchanged.

## Decisions

### D1: Separate chapters for the user guide and the migration guide

The chapter is task-oriented: "how do I use it". The migration guide is concept-oriented: "where
did X go". Keeping them apart stops the chapter from turning into a changelog.

### D2: Host the extension page at its URI path

The page is `docs/src/a2a/coordination/v1/index.md`. mdBook renders it as
`a2a/coordination/v1/index.html`, so the URI `…/a2a/coordination/v1/` resolves on the published
site. Whether GitHub Pages also redirects the version without the trailing slash, as the card uses
it, is **to verify** at publish time.

This page is the extension's normative description. A later breaking change gets a new `v2` page at
a new URI (A2A §4.6.3), and the `v1` page stays.

### D3: Drop the MCP HTTP transport (MILESTONE item 6) from v1.2.0

The research gives five reasons.

1. **The framing is stale.** MCP's HTTP+SSE transport (2024-11-05) has been deprecated since
   revision 2025-03-26. The current transport is Streamable HTTP, and revision 2026-07-28 reshaped
   it: no protocol sessions, no GET stream, server-to-client requests carried through MRTR,
   required `Mcp-Method` and `Mcp-Name` headers, and mandatory `Origin` validation. Shipping now
   means following a transport that is still changing.
2. **The crate fit is uncertain.** The pinned `rmcp` 1.7.0 (2026-05-13) has a
   `transport-streamable-http-server` feature, but it predates revision 2026-07-28. The latest
   `rmcp` is 3.5.1. Which `rmcp` release implements 2026-07-28 is **to verify**, and adopting it is
   a version upgrade across two majors.
3. **There is no demand signal.** MILESTONE itself gates the item on dogfood evidence that
   long-lived MCP is wanted, and none has been recorded.
4. **The stated benefit does not follow.** The "ChatGPT Web gap" needs a publicly reachable
   endpoint. A web-hosted client cannot reach loopback. Public exposure needs TLS and remote trust,
   and the v0.18.0 MCP write tools (`start`, `stop`, `purge`) behind an HTTP endpoint carry a large
   blast radius. That is v1.4.0's remote-access problem, not a transport toggle.
5. **It is a different protocol.** A2A Appendix B describes MCP and A2A as complementary. The
   HTTP integration path for agent-to-session delegation is now the A2A face.

*Recommendation:* revisit as a standalone later 1.x item only if there is demand. If it comes back,
build it on the then-current `rmcp` Streamable HTTP server, loopback-only, with Origin validation,
and with read-only tools by default.

## Risks / Trade-offs

- **Docs drift from the behavior.**
  → `tests/a2a_docs_content.rs` asserts the anchor terms each requirement names: the methods and
  error codes, the state names, the extension URI and the credential command.
- **The extension page URL redirect is unverified.**
  → Checked at publish. The URI is an identifier either way.

## Migration Plan

This change is documentation only. To roll back, revert.

## Open Questions

- Whether the README feature line belongs in the v1.2.0 landing page that v0.21.0 slimmed down.
  *Recommendation:* one line with a link, consistent with the slim README.
