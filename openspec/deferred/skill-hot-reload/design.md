## Context

Skill files (`assets/agent-skills/*.md`) ship with the binary,
but git-paw also reads user-override copies from
`~/.config/git-paw/agent-skills/` (and the in-repo override
location). The render pipeline from
[[lang-agnostic-assets]] substitutes `{{...}}` placeholders
at request time. Once an agent reads its skill at boot, the
CLI caches the result for the agent's session — there's no
generic way to invalidate that cache without restarting the
agent.

The pragmatic v0.6.0 fix is a polling pattern: agents poll
the broker for a version stamp every sweep iteration, compare
it to their boot-time cached version, and re-read when it
changes. The broker side is simple: render the skill,
hash the output, return both the hash and (on a separate
endpoint) the full body. The watcher invalidates the
cached render so subsequent fetches reflect file edits.

## Goals / Non-Goals

**Goals:**
- Universal across CLIs — no per-CLI hook required for v0.6.0.
- Stable hash for unchanged inputs (same skill body + same
  substitution context → same hash).
- Cheap polling — version is a 16-hex-char string; clients
  fetch it on existing sweep ticks, not extra timers.
- Skill prose teaches the pattern so agent LLMs adopt it
  naturally.

**Non-Goals:**
- Push-based reload notifications.
- Per-agent customisation.
- Structural diff between versions.
- In-CLI cache eviction.

## Decisions

### D1. Version hash shape: 16 hex chars of SHA-256

**Decision:** `version = format!("sha256:{}",
hex(sha256(rendered_body))[..16])`. Matches the id-hash
pattern from [[agent-learning-variant]] and
[[advanced-main-event]]. Sixteen hex chars is plenty for
collision-resistance in this domain (a few hundred unique
versions per session, max).

### D2. Render cache invalidated by the existing watcher

**Decision:** The render cache is keyed by
`(skill_name, session_context_hash)`. On any write event to
a watched skill file, the cache entry for that skill is
dropped. The next `/skills/version/<name>` request triggers
a fresh render → fresh hash. No timer-based invalidation;
events drive it.

`session_context_hash` covers the substitution inputs
([[lang-agnostic-assets]]'s `DOC_TOOL_COMMAND`,
`DEV_ALLOWLIST_PRESET`, `SPEC_PATH_DOCTRINE`, plus the v0.5.0
gate placeholders). Config edits change this hash too, so a
mid-session config tweak that affects a skill bumps its
version naturally.

### D3. Two endpoints: version + content, consistent rendering

**Decision:**
- `GET /skills/version/<name>` → JSON with hash + timestamp
- `GET /skills/content/<name>` → text/markdown rendered body

Both routes go through the same `render_with_version()` so
the response of `/version/` accurately predicts what
`/content/` will return. Agents fetch version every poll;
content only when version drifted.

`<name>` is the skill stem (`coordination`, `supervisor`),
no extension. Unknown names return 404.

### D4. Skill prose teaches the drift-detection pattern

**Decision:** coordination.md and supervisor.md gain a
"Detecting skill drift" subsection:

```
On boot, cache the skill version: curl
{{GIT_PAW_BROKER_URL}}/skills/version/coordination

On every broker poll cycle, fetch the version again. If it
differs from your cached value, re-read the skill content
from {{GIT_PAW_BROKER_URL}}/skills/content/coordination and
update your cached version.
```

The prose is explicit so the agent LLM picks up the pattern
consistently. Cadence matches the existing inbox-poll cycle
— no new schedule.

### D5. Opt-out config

**Decision:**
`[broker.skill_endpoints] enabled: Option<bool>` (default
`true`). When `false`, the broker returns 404 for both
endpoints and agents fall back to v0.5.0 boot-time-only
behaviour. The default is on so v0.6.0 dogfood exercises
the feature; teams that don't want it exposed flip it off.

### D6. MCP `get_skill` tool

**Decision:** Add `get_skill(name)` to
[[mcp-server]]'s read-tool set, returning
`{ name, version, content }`. Backed by the same render
pipeline as the broker endpoints. Coordinated with the
mcp-server apply phase; if its tool set is locked, this
ships as a follow-up MCP change.

### D7. No log of skill fetches

**Decision:** Skill-endpoint accesses are not broker
messages, so they don't appear in
[[dashboard-broker-log]]'s feed. Deliberate — the
fetches happen N times per agent per session, would flood
the log, and offer no actionable signal. The dashboard
gains a small "skill versions" status line as a future
candidate if dogfood demands.

## Risks / Trade-offs

- **Polling overhead** — N agents × poll rate × tiny GET.
  Mitigation: hash response is ~80 bytes; the existing
  broker can serve thousands per second; impact is
  negligible.
- **Agent LLM ignores the pattern** — Mitigation: prose
  is explicit; v0.6.0 dogfood observes adoption.
- **Skill render is expensive** — Mitigation: D2's cache
  serves repeated requests at constant cost; only the
  invalidation path re-renders.
- **Override file not in watcher scope** — Mitigation:
  the v0.5.0 filesystem watcher already covers
  `~/.config/git-paw/agent-skills/` (or the project
  override location); verify in apply.
- **Hash drift on whitespace-only edits** — by design
  (the rendered content really did change, even if
  semantically equivalent). Mitigation: agents re-read; no
  behaviour change.

## Migration Plan

1. Add `render_with_version()` to the render pipeline.
2. Wire the watcher-driven skill cache.
3. Add the two broker endpoints + config opt-out.
4. Update coordination.md + supervisor.md with the
   drift-detection prose.
5. Coordinate the MCP tool with [[mcp-server]] apply.
6. Documentation + release notes.
7. No rollback concern — additive endpoints + opt-out
   config; agents without the prose are unaffected.

## Open Questions

- **Should the version endpoint also report
  `last_modified`** (file mtime) for diagnostics? Lean: yes
  in the JSON response (alongside `rendered_at`); cheap to
  include.
- **Cache invalidation on config change** — confirm at
  apply that the substitution-context hash captures all
  config-derived inputs (governance paths, gate commands,
  etc.).
- **Override-file watch coverage** — verify the existing
  watcher covers user-override directories; extend if
  not.
