## Context

git-paw already bundles agent helpers as path-allowlisted scripts — `broker.sh` (agent↔broker) and `sweep.sh` (supervisor) — installed by `git paw init` into `.git-paw/scripts/` and referenced by the bundled skills. Agents invoke the helper instead of hand-rolling `curl`, which keeps the permission grant a single exact path (least-privilege) and the behavior deterministic across LLM regenerations. This change applies the same pattern to documentation retrieval, consuming the machine-readable surface added by `agent-friendly-docs-site` (`llms.txt`, per-page metadata/anchors).

## Goals / Non-Goals

**Goals:**
- An agent can, on demand, find the right git-paw doc page and read the relevant section, without doc content in its boot prompt or the binary.
- Retrieval goes through one path-allowlisted helper (`docs-fetch.sh`), not raw `curl`.
- The docs source is configurable (default: git-paw's published site).

**Non-Goals:**
- No offline/bundled doc mirror; no caching layer in this change (may follow later).
- No change to doc content or the docs site (that is `agent-friendly-docs-site`).
- Not a general web-fetch tool — scoped to the configured docs base URL.

## Decisions

**D1 — Ship as a bundled skill markdown + a `docs-fetch.sh` helper, mirroring `broker.sh`/`sweep.sh`.**
The skill teaches *when/why* to consult the docs; the helper does the fetch. Rationale: consistent with the established least-privilege pattern (one allowlisted path, no raw `curl`), fewer tokens per lookup, deterministic across regenerations. *Alternative:* a markdown-only skill instructing the agent to `curl` the site directly — rejected: needs a broad `curl` grant and re-derives the fetch logic each time.

**D2 — Two helper operations: `find <query>` and `get <page-or-url>`.**
`find` fetches `llms.txt` and returns the best-matching page entries (title + URL + summary); `get` retrieves a page and, given a section anchor, narrows to that section using the per-page metadata/anchors from `agent-friendly-docs-site`. Rationale: matches the discovery→retrieve flow the docs surface is designed for; keeps payloads small.

**D3 — Configurable `docs_base_url`, defaulting to git-paw's published site.**
The helper reads the base URL from git-paw config (falling back to the built-in default). Rationale: a tool consulting *its own* docs may hard-code its own site as the default without violating agnosticism, but a fork/mirror must be able to retarget. *Alternative:* hard-code with no override — rejected (forks can't retarget).

**D4 — Graceful degradation on fetch failure.**
If the docs site is unreachable or a page is missing, the helper exits non-zero with a short diagnostic and the skill instructs the agent to proceed without the docs rather than block. Rationale: doc lookup is an aid, never a hard dependency of the agent's task.

## Risks / Trade-offs

- **Docs site unavailable / network-restricted environment** → helper degrades gracefully (D4); the agent continues. Documented so operators know lookups are best-effort.
- **Published docs lag the installed binary version** (site tracks `main`; a user may run an older release) → note in the skill that docs reflect latest; acceptable for conventions/examples which change rarely.
- **Fetch-primitive availability** (the helper needs the same fetch tool the broker helper uses) → reuse that primitive; no new dependency, and `git paw init` verifies helper installation like the other scripts.

## Migration Plan

Additive: new bundled skill + helper, installed by `git paw init`; no removals. Rollback = drop the skill/helper and its allowlist grant. Existing sessions are unaffected when the skill is absent.

## Open Questions

- Wiring: is the skill always injected into agent AGENTS.md, or only when a docs-consuming feature is enabled? Lean: inject alongside the other coordination skills, gated the same way.
- Should a later change add a local cache of fetched pages to cut repeat network calls? Deferred — out of scope here.
