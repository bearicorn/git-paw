## Why

v0.5.0 Batch 3 surfaced a recurring friction: when a user
edits a bundled skill mid-session (a fix to coordination.md's
forward-coordination wording, a clarification in
supervisor.md's gate prose), the running agents don't see the
change. Their CLI sessions cached the skill at boot. The
workaround during v0.5.0 was per-section `agent.feedback`
nudges from the supervisor — fragile, manual, and easy to
forget. As bundled skills grow over v0.6.0 (with
[[lang-agnostic-assets]], [[coordination-context-budget]],
[[supervisor-introspection]], etc.), in-session skill edits
become a meaningful UX dimension.

This change adds a `skill-version` discovery so agents can
detect that a skill has changed mid-session and re-read it.
The approach is universal (works across CLIs without per-CLI
hooks; those are v1.0.0 territory) and opt-in (agents poll a
broker endpoint at sweep intervals).

## What Changes

- **New broker endpoint `GET /skills/version/<skill_name>`**
  returning the current content hash of the rendered skill:
  ```
  GET /skills/version/coordination
  → 200 OK
  { "skill": "coordination",
    "version": "sha256:9af3...e8c1",
    "rendered_at": "2026-05-29T12:34:56Z" }
  ```
- **Rendered-content hash** — the version is a hex SHA-256
  prefix (16 chars) of the *rendered* skill content (post
  `{{...}}` substitution from
  [[lang-agnostic-assets]]'s render pipeline). Two agents on
  the same session with the same render context get the same
  hash; the hash changes whenever the underlying skill file
  changes OR when a substitution input (config value, backend
  resolution) changes.
- **Watcher invalidation** — when the existing filesystem
  watcher sees a write to `assets/agent-skills/*.md` (or the
  override location), it invalidates the cached render and
  recomputes the version on the next request.
- **Skill prose update** in `assets/agent-skills/
  coordination.md` and `assets/agent-skills/supervisor.md` —
  both gain a "Detecting skill drift" subsection teaching:
  - Cache the skill's version on first read (boot time)
  - On every broker poll loop, additionally fetch
    `/skills/version/<your skill>` and compare
  - When the version changes, re-read the skill (either via
    a re-fetch endpoint OR by reading the file directly from
    the bundled path)
- **New endpoint `GET /skills/content/<skill_name>`** returning
  the rendered skill body so agents that don't have direct
  filesystem access can re-read via broker:
  ```
  GET /skills/content/coordination
  → 200 OK
  Content-Type: text/markdown
  <rendered skill body>
  ```
  Same render path as `version/`, ensuring `version` →
  `content` consistency.
- **MCP `get_skill(name)` tool** is added to
  [[mcp-server]]'s read tool set (coordinated at apply) so
  chat clients can also fetch rendered skills.
- **Opt-out via config** — `[broker.skill_endpoints] enabled
  = false` disables the new endpoints for projects that
  don't want them exposed.

## Non-goals

- **No push notification.** Agents poll for drift; the broker
  doesn't push reload signals. Push patterns vary by CLI
  (Claude has hooks; others don't) and per-CLI work is
  v1.0.0.
- **No automatic re-render at file-write time** beyond
  invalidating the cache. Agents drive their own re-read
  cadence.
- **No skill diff endpoint** in v0.6.0. Agents fetching a
  new version see the full body; structural diff is a
  v0.7.0 candidate if dogfood demands it.
- **No per-agent skill personalisation.** All agents fetch
  the same rendered output.
- **No interception of in-CLI skill caches.** We can't force
  Claude (or any CLI) to drop its boot-time context; the
  re-read pattern relies on the agent LLM reading the new
  content and continuing.
- **No agent CLI invoked as LLM backend** (inherited).

## Capabilities

### New Capabilities
- `skill-hot-reload`: the rendered-content hash, the
  `/skills/version/<name>` and `/skills/content/<name>`
  broker endpoints, the watcher-driven cache invalidation,
  the agent-skill prose teaching the drift-detection
  pattern, and the opt-out config field.

### Modified Capabilities
- `broker-endpoints` (existing): adds the two new
  endpoints. Delta at archive time.
- `agent-skills` (existing): coordination.md and
  supervisor.md gain the drift-detection subsections.
  Delta at archive time.
- `template-substitution` (existing): the render pipeline
  gains a hash-of-rendered-output accessor. Delta at
  archive time.
- `mcp-read-tools` (from [[mcp-server]]): adds the
  `get_skill` tool. Delta once mcp-server lands.

## Impact

- **New code**:
  - `src/skills.rs` (or wherever render lives) — `render_with_version()` returning `(content, version_hash)`
  - `src/broker/server.rs` — two new GET routes
  - `src/broker/skill_cache.rs` — version cache invalidated by
    the existing filesystem watcher
- **New config field**:
  `[broker.skill_endpoints].enabled: Option<bool>` (default
  `true`)
- **Existing modules touched**:
  - `src/filesystem_watcher.rs` (or v0.5.0 equivalent) —
    skill-file write events bubble to the skill cache
- **Tests**:
  - Hash stability: same content + same substitution inputs
    → same hash
  - Cache invalidation: writing to a watched skill file →
    next `version/` request returns a new hash
  - `content/` and `version/` responses are consistent (the
    rendered body hashes to the advertised version)
  - Opt-out: 404 (or 410 Gone) when `enabled = false`
- **Documentation**:
  - User-guide section on hot-reloading skills mid-session
  - Configuration reference for the new endpoint toggle
  - Release notes call out the drift-detection pattern
- **Backwards compatibility**:
  - Existing agents (without the new skill prose) keep
    working — they just don't poll for drift; v0.5.0
    behaviour preserved
  - Existing broker consumers ignore the new endpoints
- **Cross-references**:
  - [[lang-agnostic-assets]] — render pipeline is the
    versioning source
  - [[coordination-context-budget]],
    [[supervisor-stream-timeout-recovery]],
    [[supervisor-introspection]],
    [[qualitative-learnings]],
    [[advanced-main-event]],
    [[conflict-detector-fn-granularity]] —
    all add skill prose this change helps reach running
    agents
  - [[mcp-server]] — `get_skill` is the MCP analogue
  - [[dashboard-broker-log]] — endpoint accesses are not
    broker messages, so no log entry; deliberate
