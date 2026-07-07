# docs-fetch-skill Specification

## Purpose
TBD - created by archiving change docs-fetch-skill. Update Purpose after archive.
## Requirements
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

