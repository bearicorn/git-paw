## 1. docs-fetch helper (D1/D2/D4)

- [ ] 1.1 Add `assets/scripts/docs-fetch.sh` with a `find <query>` op (fetch `llms.txt`, return best-matching page entries: title + absolute URL + summary) and a `get <page-or-url> [anchor]` op (retrieve the page; when an anchor is given, narrow to that section via the page's metadata/anchors); reuse the broker helper's fetch primitive — no new dependency
- [ ] 1.2 Resolve the docs base URL from git-paw config, falling back to the built-in default (git-paw's published site)
- [ ] 1.3 On an unreachable site or missing page, exit non-zero with a short diagnostic (graceful degradation, never hang)

## 2. Bundled skill

- [ ] 2.1 Add the docs-fetch skill markdown under `assets/agent-skills/` — teach when/why to consult git-paw docs, direct the agent to invoke `docs-fetch.sh` (`find` → `get`) rather than raw `curl`, and instruct it to continue its task without the docs if a lookup fails

## 3. Install + allowlist + config wiring

- [ ] 3.1 `git paw init` installs `docs-fetch.sh` into `.git-paw/scripts/` (parallel to `broker.sh`/`sweep.sh`)
- [ ] 3.2 Seed the agent allowlist with the exact `.git-paw/scripts/docs-fetch.sh` path (by-path least-privilege — no `curl` wildcard)
- [ ] 3.3 Add a `docs_base_url` config field (default = git-paw's published site) and wire it into the helper resolution

## 4. Tests

- [ ] 4.1 `git paw init` installs the helper and grants the exact helper path (assert no `curl` wildcard grant)
- [ ] 4.2 The rendered skill instructs helper invocation (not raw `curl`) and includes the degrade-gracefully instruction
- [ ] 4.3 Discovery + retrieval against fixture `llms.txt` + page: default vs overridden base URL, section-by-anchor narrowing, and non-zero exit on a missing page

## 5. Docs

- [ ] 5.1 Document the docs-fetch skill and the `docs_base_url` config field (user-guide chapter + configuration reference)
