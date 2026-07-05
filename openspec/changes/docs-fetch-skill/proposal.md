## Why

Even with the agent-friendly docs surface (`agent-friendly-docs-site`), an agent working with git-paw has no low-friction, bundled way to *consult* those docs on demand. Today the choice is either dumping doc content into the boot prompt (token bloat, and content shipped inside the binary) or not consulting the docs at all. A bundled fetch skill lets an agent pull exactly the git-paw convention or example it needs, when it needs it — discovering the right page via `llms.txt` and retrieving just that page.

## What Changes

- Add a **bundled agent skill** (`assets/agent-skills/`) that teaches an agent to consult git-paw's documentation on demand: read `llms.txt` to find the right page, then fetch that page and read the section it needs (using the per-page metadata/anchors from `agent-friendly-docs-site`).
- Add a **bundled helper script** (`assets/scripts/docs-fetch.sh`) that performs the fetch — discovery (`llms.txt`) and page/section retrieval — mirroring the least-privilege `broker.sh`/`sweep.sh` pattern: installed by `git paw init` and **allowlisted by exact script path, not raw `curl`**.
- The **docs base URL is configurable** (defaults to git-paw's published docs site) so a fork can point the skill at its own docs.
- Degrade gracefully: if the docs site is unreachable, the skill instructs the agent to proceed without blocking.

Non-goals: no local doc mirror shipped in the binary; no change to the docs content itself (that is `agent-friendly-docs-site`).

## Capabilities

### New Capabilities
- `docs-fetch-skill`: git-paw SHALL bundle an agent skill + a path-allowlisted helper that let an agent discover (via `llms.txt`) and retrieve git-paw documentation on demand from the configurable docs site, without shipping doc content in the binary or the boot prompt.

### Modified Capabilities
<!-- none: the helper's path grant is specified as a requirement of this capability, mirroring the broker.sh least-privilege model, rather than modifying curl-allowlist -->

## Impact

- **Bundled assets**: new `assets/agent-skills/<docs-fetch>.md` skill + `assets/scripts/docs-fetch.sh` helper (parallel to `broker.sh`/`sweep.sh`).
- **`git paw init`**: installs the helper into `.git-paw/scripts/` and seeds the agent allowlist with its exact path (same by-path least-privilege as the broker helper).
- **Config**: a `docs_base_url` (default = git-paw's published site) so forks can retarget.
- **Depends on** `agent-friendly-docs-site` (`llms.txt` discovery + per-page metadata/anchors).
- **Agnosticism**: the *default* URL points at git-paw's own docs (a tool consulting its own documentation), and it is configurable — no consumer-stack assumption is baked in.
- **No new approved-dependency**; the helper uses the same fetch primitive as the existing broker helper.
