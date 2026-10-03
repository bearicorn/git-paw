## MODIFIED Requirements

### Requirement: Bundled docs-fetch skill and helper

git-paw SHALL bundle an agent skill and a `docs-fetch` helper script, and `git paw init` SHALL install the helper into the project's `.git-paw/scripts/` directory at a single stable path, so that a permission rule scoped to that exact script path can allow it. Consulting docs through the helper SHALL NOT require a broad `curl` grant, mirroring the least-privilege model of the existing broker helper.

#### Scenario: init installs the helper at an exact path
- **WHEN** `git paw init` runs in a project
- **THEN** the `docs-fetch` helper is present at `.git-paw/scripts/docs-fetch.sh`
- **AND** the bundled docs-fetch skill invokes it by that exact path, so no wildcard `curl` permission is needed to use it

#### Scenario: the skill instructs invoking the helper, not raw curl
- **WHEN** the bundled docs-fetch skill is rendered into an agent's instructions
- **THEN** it directs the agent to invoke the `docs-fetch` helper to consult docs, and does not instruct the agent to construct a raw `curl` to the docs site
