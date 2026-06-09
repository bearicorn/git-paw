## ADDED Requirements

### Requirement: Skill version endpoint

The broker SHALL expose `GET /skills/version/<skill_name>`
returning a JSON object containing `skill` (echo of the
name), `version` (string of the form `"sha256:<16-hex-chars>"`),
and `rendered_at` (ISO 8601 UTC timestamp of the most recent
render). The system SHALL return 404 when the skill name is
unknown.

#### Scenario: Known skill returns a version

- **WHEN** an agent calls `GET /skills/version/coordination`
- **THEN** the response SHALL be 200 with a JSON body
  containing `skill: "coordination"`, a `version` string
  matching the `sha256:` prefix pattern, and a `rendered_at`
  ISO 8601 timestamp

#### Scenario: Unknown skill name returns 404

- **WHEN** an agent calls
  `GET /skills/version/no-such-skill`
- **THEN** the response SHALL be 404 with an error body
  identifying the unknown name

### Requirement: Skill content endpoint

The broker SHALL expose `GET /skills/content/<skill_name>`
returning the rendered skill body. The response SHALL use
`Content-Type: text/markdown`. The body returned SHALL be
the content that produced the hash advertised by
`/skills/version/<skill_name>` at the same point in time.

#### Scenario: Content endpoint returns the rendered body

- **WHEN** an agent calls `GET /skills/content/coordination`
- **THEN** the response SHALL be 200 with `Content-Type:
  text/markdown` and the body SHALL be the fully-rendered
  (post-substitution) skill content

#### Scenario: Content matches the advertised version

- **WHEN** an agent fetches both
  `/skills/version/coordination` and
  `/skills/content/coordination` in close succession
- **THEN** the SHA-256-16-hex-char hash of the content
  response SHALL equal the `version` value's
  `sha256:`-prefixed portion

### Requirement: Stable hashing

The system SHALL produce identical version hashes for
identical rendered content. Rendering the same skill twice
with the same substitution inputs (config values, resolved
backend, etc.) SHALL yield the same hash. Changing the skill
file contents OR any substitution input SHALL change the
hash.

#### Scenario: Identical inputs yield identical hashes

- **GIVEN** a stable configuration and an unchanged skill
  file
- **WHEN** the version endpoint is queried twice
- **THEN** both responses SHALL carry identical `version`
  values

#### Scenario: Skill file edit changes the hash

- **GIVEN** an initial version hash
- **WHEN** the skill file is edited and the watcher
  invalidates the cache
- **THEN** the next version response SHALL carry a
  different hash

#### Scenario: Substitution input change changes the hash

- **GIVEN** an initial version hash
- **WHEN** a config value backing a `{{...}}` placeholder
  changes (e.g. `[supervisor].doc_tool_command`)
- **THEN** the next version response SHALL carry a
  different hash

### Requirement: Watcher-driven cache invalidation

The system SHALL invalidate the cached render of a skill
when the filesystem watcher observes a write to that
skill's source file (bundled or override). The next
`/skills/version/<name>` request after the invalidation
SHALL trigger a fresh render.

#### Scenario: Override file write invalidates cache

- **GIVEN** a cached render of the coordination skill
- **WHEN** the user writes a change to the override copy at
  `~/.config/git-paw/agent-skills/coordination.md`
- **THEN** the next version request SHALL produce a fresh
  hash reflecting the new content

#### Scenario: Bundled-asset edit invalidates cache (dev mode)

- **GIVEN** a development run where `assets/agent-skills/`
  is the watched source
- **WHEN** the developer edits a bundled skill file
- **THEN** the next version request SHALL produce a fresh
  hash

### Requirement: Drift-detection skill prose

The bundled coordination and supervisor skills SHALL include
a "Detecting skill drift" subsection teaching agents to:
- Cache the skill's version on first read.
- On every broker poll cycle, fetch the version endpoint
  and compare to the cached value.
- When the version differs, re-read the content endpoint
  and update the cached version.

#### Scenario: Coordination skill teaches the pattern

- **WHEN** the bundled `coordination.md` is inspected
- **THEN** it SHALL contain a "Detecting skill drift"
  subsection covering the three-step boot-cache + poll-
  compare + re-read pattern

#### Scenario: Supervisor skill teaches the same pattern

- **WHEN** the bundled `supervisor.md` is inspected
- **THEN** it SHALL contain an equivalent
  drift-detection subsection

### Requirement: Opt-out via config

The system SHALL accept `[broker.skill_endpoints].enabled`
as a boolean config field (default `true`). When the field
is `false`, the broker SHALL return 404 for both
`/skills/version/<name>` and `/skills/content/<name>`,
falling back agents to v0.5.0 boot-time-only skill access.

#### Scenario: Opt-out returns 404 for both endpoints

- **GIVEN** `[broker.skill_endpoints].enabled = false`
- **WHEN** an agent calls either skill endpoint
- **THEN** the response SHALL be 404, regardless of skill
  name validity

#### Scenario: Default config enables the endpoints

- **GIVEN** a `.git-paw/config.toml` with no
  `[broker.skill_endpoints]` section
- **WHEN** an agent calls
  `/skills/version/coordination`
- **THEN** the response SHALL be 200 (the default behaviour
  is enabled)
