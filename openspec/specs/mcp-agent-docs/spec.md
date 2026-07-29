# mcp-agent-docs Specification

## Purpose
Makes the published documentation site machine-consumable by generating, deterministically from the mdBook sources at build time, an `llms.txt` index, a `sitemap.xml`, a `robots.txt`, and per-page structured metadata so an agent can identify pages and target sections without fetching siblings and the machine-readable surface never drifts from the published content, and bundles a path-allowlisted `docs-fetch` helper and agent skill that let coding agents discover (via `llms.txt`) and retrieve documentation pages or sections on demand from a configurable docs base URL — injected only when `docs_base_url` is explicitly set, shipping no doc content in the binary or boot prompt, and degrading gracefully so a fetch failure never blocks the agent.

## Requirements
### Requirement: llms.txt index

The published docs site SHALL expose an `llms.txt` file at the site root that indexes the documentation for LLM consumers, following the llmstxt.org convention: an H1 title, a one-line summary of git-paw, and grouped sections of page entries in the form `- [Page Title](absolute-url): one-line summary`, ordered by the mdBook table of contents.

#### Scenario: llms.txt is published and lists documented pages
- **WHEN** the docs are built and deployed
- **THEN** `llms.txt` exists at the site root, begins with an H1 title and a summary line, and contains a link entry (title + absolute URL + summary) for each page in `docs/src/SUMMARY.md`

#### Scenario: a page summary can be overridden at the source
- **WHEN** a source page begins with an `<!-- summary: ... -->` HTML comment
- **THEN** that text is used as the page's summary in `llms.txt` instead of the auto-derived first sentence

### Requirement: sitemap.xml

The published docs site SHALL expose a valid `sitemap.xml` at the site root enumerating the canonical URL of every documentation page.

#### Scenario: sitemap enumerates every page
- **WHEN** the docs are built
- **THEN** `sitemap.xml` is well-formed XML containing one `<url><loc>` entry with the canonical absolute URL for each page in the mdBook table of contents

### Requirement: robots.txt

The published docs site SHALL expose a `robots.txt` at the site root that permits documentation crawling and advertises the sitemap.

#### Scenario: robots.txt allows crawling and points at the sitemap
- **WHEN** the docs are built
- **THEN** `robots.txt` exists at the site root, does not disallow the documentation paths, and includes a `Sitemap:` line referencing the absolute `sitemap.xml` URL

### Requirement: per-page structured metadata

Each rendered documentation page SHALL carry machine-readable metadata sufficient for an agent to identify the page and target a section without fetching sibling pages: at minimum a description, the canonical URL, and the list of the page's section anchors.

#### Scenario: a built page exposes description, canonical URL, and section anchors
- **WHEN** any documentation page is built
- **THEN** its HTML `<head>` includes a `<meta name="description">` and a machine-readable metadata block giving the page title, canonical URL, and the anchor ids of its headings

### Requirement: deterministic build-time generation

The `llms.txt`, `sitemap.xml`, `robots.txt`, and per-page metadata SHALL be generated from the mdBook sources as part of the documentation build — never hand-maintained — so the machine-readable surface cannot drift from the published content.

#### Scenario: the surface regenerates with the docs and cannot drift
- **WHEN** the documentation build runs (locally via the docs recipe or in the CI `docs` job)
- **THEN** the four artifacts are produced from the current `docs/src` sources and deployed alongside the site, with no manual step required

#### Scenario: generation is reproducible
- **WHEN** the docs build runs twice against unchanged sources
- **THEN** the generated `llms.txt`, `sitemap.xml`, and `robots.txt` are byte-identical between runs (any date field is supplied as a build input, not read from the wall clock)

### Requirement: Bundled docs-fetch skill and helper

git-paw SHALL bundle an agent skill and a `docs-fetch` helper script, and `git paw init` SHALL install the helper into the project's `.git-paw/scripts/` directory and grant the agent allowlist that exact script path — never a broad `curl` grant — mirroring the least-privilege model of the existing broker helper.

#### Scenario: init installs and path-allowlists the helper
- **WHEN** `git paw init` runs in a project
- **THEN** the `docs-fetch` helper is present under `.git-paw/scripts/` and the agent allowlist grants that exact helper path (not a wildcard `curl` command)

#### Scenario: the skill instructs invoking the helper, not raw curl
- **WHEN** the bundled docs-fetch skill is rendered into an agent's instructions
- **THEN** it directs the agent to invoke the `docs-fetch` helper to consult docs, and does not instruct the agent to construct a raw `curl` to the docs site

### Requirement: Gated injection into agent sessions

The docs-fetch skill SHALL be injected into each coding agent's managed `AGENTS.md` block if and only if `docs_base_url` is explicitly configured. Because the effective docs base URL defaults to git-paw's own published site when unset, injecting the skill unconditionally would point every consumer's agents at git-paw's documentation — so the skill is withheld until the operator has pointed git-paw at their own docs, keeping the exported skill project-agnostic. When injected alongside the coordination skill, the two occupy one managed block, each retaining its own heading structure.

#### Scenario: injected when docs_base_url is configured
- **WHEN** a session starts (or an agent is added) in a project whose config sets `docs_base_url`
- **THEN** each agent's managed `AGENTS.md` block includes the docs-fetch skill content directing it to the `docs-fetch` helper

#### Scenario: withheld when docs_base_url is unset
- **WHEN** a session starts in a project that has not configured `docs_base_url`
- **THEN** no docs-fetch skill content is injected into any agent's `AGENTS.md`, so agents are not pointed at git-paw's own docs by default

#### Scenario: coexists with the coordination skill in one block
- **WHEN** both the coordination skill (broker enabled) and the docs-fetch skill (`docs_base_url` set) apply
- **THEN** the agent's managed block carries both, coordination first, each keeping its own headings

### Requirement: On-demand discovery via llms.txt

The helper SHALL provide a discovery operation that reads the docs site's `llms.txt` and returns the pages best matching a query (title, absolute URL, and summary), so an agent finds the right page before retrieving it.

#### Scenario: discovery returns matching pages
- **WHEN** an agent runs the helper's discovery operation with a query term
- **THEN** the helper fetches `llms.txt` from the configured docs base URL and returns the matching page entries (title, URL, summary)

### Requirement: Targeted page and section retrieval

The helper SHALL provide a retrieval operation that fetches a documentation page and, when given a section anchor, returns just that section — using the per-page metadata/anchors published by `agent-friendly-docs-site`.

#### Scenario: retrieve a whole page
- **WHEN** an agent runs the retrieval operation for a page URL or path
- **THEN** the helper returns that page's documentation content

#### Scenario: retrieve a single section by anchor
- **WHEN** the retrieval operation is given a page plus a section anchor
- **THEN** the helper returns only that section's content rather than the whole page

### Requirement: Configurable docs base URL

The docs base URL the helper targets SHALL default to git-paw's published documentation site and SHALL be overridable via configuration, so a fork or mirror can retarget it.

#### Scenario: default base URL
- **WHEN** no docs base URL is configured
- **THEN** the helper targets git-paw's published documentation site

#### Scenario: overridden base URL
- **WHEN** a docs base URL is configured
- **THEN** the helper targets the configured URL for both discovery and retrieval

### Requirement: Graceful degradation and no shipped doc content

Documentation lookup SHALL be best-effort: on an unreachable site or missing page the helper exits non-zero with a short diagnostic and the skill instructs the agent to proceed without blocking. Documentation content SHALL NOT be shipped inside the binary or injected wholesale into the boot prompt — it is fetched on demand.

#### Scenario: fetch failure does not block the agent
- **WHEN** the docs site is unreachable or the requested page does not exist
- **THEN** the helper exits non-zero with a diagnostic and the skill directs the agent to continue its task without the docs

#### Scenario: no doc content is bundled or boot-injected
- **WHEN** an agent session starts with the docs-fetch skill enabled
- **THEN** no documentation page content is embedded in the binary or the boot prompt; the agent retrieves docs only on demand via the helper
