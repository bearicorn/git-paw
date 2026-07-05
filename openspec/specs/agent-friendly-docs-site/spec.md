# agent-friendly-docs-site Specification

## Purpose
TBD - created by archiving change agent-friendly-docs-site. Update Purpose after archive.
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

