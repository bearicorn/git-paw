## Why

git-paw publishes an mdBook documentation site (`https://bearicorn.github.io/git-paw/`) authored purely for human readers: there is no machine-readable index, no crawl/discovery surface, and no per-page metadata. An agent (git-paw's own coding/supervisor agents, or any LLM consulting the docs) cannot cheaply discover which page answers a question or fetch just the relevant section — it must guess URLs or ingest whole chapters. This blocks the companion `docs-fetch-skill` change and makes git-paw's conventions hard to consult on demand.

## What Changes

- Publish an **`llms.txt`** index at the site root (the emerging convention): a curated Markdown list of the key pages — title, one-line summary, absolute URL — so an agent can pick the right page in a single fetch.
- Publish a **`sitemap.xml`** enumerating every page with its last-modified time, for crawler/agent discovery.
- Publish a **`robots.txt`** that explicitly permits documentation crawling and points at the sitemap.
- Give each rendered page **structured metadata** — a stable, machine-readable header (title, summary, canonical URL, and the page's section anchors) — so an agent can target exactly the section it needs.
- **Generate all of the above deterministically from the mdBook sources at build time** (the same `mdbook build docs/` path the CI `docs` job runs), so the machine-readable surface can never drift from the authored content, and deploy them alongside the site.

Non-goals: no change to human-facing content or navigation; the on-demand fetch *skill* that consumes this surface is a separate change (`docs-fetch-skill`).

## Capabilities

### New Capabilities
- `agent-friendly-docs-site`: the published documentation site SHALL expose a machine-readable discovery + retrieval surface — an `llms.txt` index, a `sitemap.xml`, a `robots.txt`, and per-page structured metadata — generated deterministically from the mdBook sources during the docs build.

### Modified Capabilities
<!-- none: this adds a new surface; `user-documentation` requirements are unchanged -->

## Impact

- **Docs build**: a generation step (mdBook preprocessor or post-build script) that emits `llms.txt`, `sitemap.xml`, `robots.txt`, and per-page metadata into `docs/book/`. Exact mechanism chosen in `design.md` (prefer a build script over a new crate dependency).
- **CI**: the `docs` job in `.github/workflows/ci.yml` runs the generation as part of the docs build and publishes the extra files with the GitHub Pages artifact.
- **`docs/src/`**: may add per-page front-matter/metadata source, depending on the chosen mechanism.
- **No runtime/CLI code change** and **no new approved-dependency** unless `design.md` justifies one.
- **Consumed by** the separate `docs-fetch-skill` change.
