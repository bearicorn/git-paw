## MODIFIED Requirements

### Requirement: Bundled docs-fetch skill and helper

git-paw SHALL bundle an agent skill and a `docs-fetch` helper script, and `git paw init` SHALL install the helper into the project's `.git-paw/scripts/` directory at a single stable path when `docs_base_url` is configured (the Skills layer, per `core-layer-isolation`), so that a permission rule scoped to that exact script path can allow it. Consulting docs through the helper SHALL NOT require a broad `curl` grant, mirroring the least-privilege model of the existing broker helper. When `docs_base_url` is not configured, `git paw init` SHALL NOT install the helper and SHALL remove a copy a previous run installed, reporting the removal.

#### Scenario: init installs the helper at an exact path

- **GIVEN** a project whose config sets `docs_base_url`
- **WHEN** `git paw init` runs in the project
- **THEN** the `docs-fetch` helper is present at `.git-paw/scripts/docs-fetch.sh`
- **AND** the bundled docs-fetch skill invokes it by that exact path, so no wildcard `curl` permission is needed to use it

#### Scenario: init without docs_base_url installs no helper

- **GIVEN** a project whose config does not set `docs_base_url`
- **WHEN** `git paw init` runs
- **THEN** no `docs-fetch` helper is present under `.git-paw/scripts/`, and a copy written by an earlier init has been removed

#### Scenario: the skill instructs invoking the helper, not raw curl
- **WHEN** the bundled docs-fetch skill is rendered into an agent's instructions
- **THEN** it directs the agent to invoke the `docs-fetch` helper to consult docs, and does not instruct the agent to construct a raw `curl` to the docs site
