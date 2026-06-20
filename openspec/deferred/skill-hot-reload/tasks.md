## 1. Config field

- [ ] 1.1 Add `[broker.skill_endpoints].enabled:
      Option<bool>` to `src/config.rs` (`#[serde(default)]`;
      resolves to true when absent)
- [ ] 1.2 Document in `docs/src/configuration/`
- [ ] 1.3 Unit tests: default true, explicit false, v0.5.0
      configs still parse

## 2. Render pipeline versioning

- [ ] 2.1 Add `render_with_version(skill_name) -> Result<(String, String)>`
      to `src/skills.rs` (or wherever rendering lives)
      returning (rendered_body, version_hash)
- [ ] 2.2 Hash function: SHA-256 over the rendered body,
      hex-encoded, first 16 chars, prefixed with `sha256:`
- [ ] 2.3 Implement an in-process cache keyed by
      `(skill_name, session_context_hash)`; serve cached
      value when valid, recompute on miss
- [ ] 2.4 `session_context_hash` covers every substitution
      input (gate commands, governance paths,
      doc_tool_command, dev allowlist preset, backend
      resolution)
- [ ] 2.5 Unit tests: same inputs → same hash; changed file
      → new hash; changed config → new hash

## 3. Watcher integration

- [ ] 3.1 Identify the filesystem-watcher path used in v0.5.0;
      verify it covers both bundled assets in dev mode AND
      user-override directories
- [ ] 3.2 On a skill-file write event, invalidate the cache
      entry for that skill name (all session-context
      variants)
- [ ] 3.3 If the override-file path isn't currently watched,
      extend the watcher to cover it
- [ ] 3.4 Unit test: simulated file-write event invalidates
      the cache; next render produces a fresh hash

## 4. Broker endpoints

- [ ] 4.1 Add `GET /skills/version/<name>` route to the
      broker
- [ ] 4.2 Implementation calls `render_with_version()`,
      returns JSON with `skill`, `version`, `rendered_at`
- [ ] 4.3 Add `GET /skills/content/<name>` route returning
      `text/markdown` body
- [ ] 4.4 Both routes honour
      `[broker.skill_endpoints].enabled = false` by
      returning 404
- [ ] 4.5 Unknown skill name → 404 with a clear error body
- [ ] 4.6 Unit + integration tests covering the four spec
      scenarios (200 on known, 404 on unknown, 404 on
      opt-out, version-content consistency)

## 5. Skill drift-detection prose

- [ ] 5.1 Append a "Detecting skill drift" subsection to
      `assets/agent-skills/coordination.md` covering the
      three-step pattern (boot cache, poll compare, re-read)
- [ ] 5.2 Append the equivalent subsection to
      `assets/agent-skills/supervisor.md`
- [ ] 5.3 Use `{{GIT_PAW_BROKER_URL}}` template variable in
      the curl examples so the substituted output points at
      the active broker
- [ ] 5.4 No-language-leak audit from
      [[lang-agnostic-assets]] passes
- [ ] 5.5 Skill-content tests assert both files contain the
      drift-detection subsection

## 6. MCP get_skill tool

- [ ] 6.1 Coordinate with [[mcp-server]] apply: add
      `get_skill(name)` to the read-tool set
- [ ] 6.2 Tool returns `{ name, version, content }` using
      `render_with_version()`
- [ ] 6.3 If mcp-server's tool set is locked, ship as a
      follow-up MCP change (not a v0.6.0 blocker)
- [ ] 6.4 E2E MCP test: `get_skill("coordination")` returns
      the same content as `/skills/content/coordination`
      with the same version

## 7. Documentation

- [ ] 7.1 Add "Hot-reloading skills mid-session" section to
      the user-guide skills chapter (or coordination chapter)
- [ ] 7.2 Update broker-endpoints spec at archive time with
      the two new routes
- [ ] 7.3 Update configuration reference for
      `[broker.skill_endpoints].enabled`
- [ ] 7.4 Release notes: skills now hot-reload mid-session
      via a polling pattern
- [ ] 7.5 `mdbook build docs/` succeeds

## 8. Integration tests

- [ ] 8.1 E2E: start a session, fetch version, edit the
      override skill file, fetch version again — version
      differs
- [ ] 8.2 E2E: `/skills/content/<name>` returns the
      post-edit rendered body
- [ ] 8.3 E2E: opt-out config produces 404 on both endpoints
- [ ] 8.4 E2E: changing a config value backing a
      placeholder produces a new version on the next request
- [ ] 8.5 Every requirement in `skill-hot-reload/spec.md`
      has at least one asserting test

## 9. Quality gates

- [ ] 9.1 `just check` + `just deny` + `cargo audit` pass
- [ ] 9.2 Coverage ≥ 80% on the new render-with-version
      path + broker endpoints
- [ ] 9.3 Manual dogfood pass: edit `coordination.md`
      mid-session, observe at least one agent picking up the
      change within one sweep cycle (verified via the
      drift-detection prose actually firing)
