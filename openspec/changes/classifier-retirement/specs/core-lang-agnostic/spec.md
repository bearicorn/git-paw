## MODIFIED Requirements

### Requirement: New placeholder substitutions in skill rendering

The skill rendering pipeline SHALL substitute two placeholders in the
bundled supervisor skill: `{{DOC_TOOL_COMMAND}}` and
`{{SPEC_PATH_DOCTRINE}}`. Substitutions SHALL be applied via the
existing `{{...}}` substitution machinery used for the v0.5.0
five-gate placeholders. The bundled skills SHALL NOT carry a
`{{DEV_ALLOWLIST_PRESET}}` placeholder or any other enumeration of
commands presented as approvable.

#### Scenario: DOC_TOOL_COMMAND substitutes config value

- **GIVEN** `[supervisor].doc_tool_command = "cargo doc --no-deps"`
- **WHEN** the supervisor skill is rendered for a session
- **THEN** every occurrence of `{{DOC_TOOL_COMMAND}}` SHALL be
  replaced with `cargo doc --no-deps` in the rendered skill body

#### Scenario: No dev-allowlist placeholder remains

- **WHEN** the bundled supervisor and coordination skill templates are inspected
- **THEN** neither SHALL contain `{{DEV_ALLOWLIST_PRESET}}`
- **AND** the rendered supervisor skill SHALL NOT contain a dev-command allowlist enumeration

#### Scenario: SPEC_PATH_DOCTRINE renders per resolved backend

- **GIVEN** a supervisor session resolved to the OpenSpec backend
- **WHEN** the supervisor skill is rendered
- **THEN** `{{SPEC_PATH_DOCTRINE}}` SHALL be replaced with a
  paragraph that references `openspec/changes/<name>/...` paths
  and the `openspec validate` workflow

#### Scenario: SPEC_PATH_DOCTRINE for Spec Kit backend

- **GIVEN** a supervisor session resolved to the Spec Kit backend
- **WHEN** the supervisor skill is rendered
- **THEN** `{{SPEC_PATH_DOCTRINE}}` SHALL be replaced with a
  paragraph that references `.specify/specs/<feature>/...` paths
  and Spec Kit's checklist convention

#### Scenario: SPEC_PATH_DOCTRINE for Markdown backend

- **GIVEN** a supervisor session resolved to the Markdown backend
- **WHEN** the supervisor skill is rendered
- **THEN** `{{SPEC_PATH_DOCTRINE}}` SHALL be replaced with a
  paragraph that references `paw_status: pending` Markdown files
  and explains the absence of a per-artifact workflow

#### Scenario: Multi-backend session renders a multi-backend doctrine

- **GIVEN** a session whose discovered specs span more than one
  backend (cross-format selection from v0.5.0)
- **WHEN** the supervisor skill is rendered
- **THEN** `{{SPEC_PATH_DOCTRINE}}` SHALL list the present
  backends with their respective path conventions in a single
  paragraph

### Requirement: No language-leak audit

The CI test suite SHALL include a no-leak audit that renders the
supervisor skill against fixture configurations for each spec
backend and SHALL assert no token from a forbidden list appears
in the rendered output outside explicitly-allowed spans. The
forbidden list SHALL include at minimum `cargo` (outside an
explicitly-allowed span), `rustdoc`, `.rs:` (as a hardcoded
source-path marker), `Cargo.toml`, and `rustc`.

The audit SHALL additionally cover every bundled asset carrying agent-facing prose, not
only the supervisor skill, so prose relocated out of compiled code is subject to the same
enforcement as the skills.

The forbidden list SHALL additionally include vendor product names and vendor-specific
paths — at minimum `Claude Code` and `.claude/` — outside an explicitly-allowed span. A
bundled asset may name a specific CLI only inside such a span (for example, an enumeration
of known CLIs), so that naming a vendor is a deliberate, reviewable act rather than an
accident.

#### Scenario: Audit passes on the v0.6.0 supervisor template

- **WHEN** the no-leak audit runs against the
  v0.6.0-as-shipped supervisor skill rendered for each backend
- **THEN** the audit SHALL pass

#### Scenario: Audit catches a Rust-leak regression

- **GIVEN** a supervisor skill template edited to add a literal
  `cargo test` outside the allowed-span sentinels
- **WHEN** the no-leak audit runs
- **THEN** the audit SHALL fail and SHALL identify the offending
  token plus its location

#### Scenario: Sentinel-comment scope excludes an allowed span

- **GIVEN** a bundled asset whose `<!-- allowlist-prose -->` …
  `<!-- /allowlist-prose -->` span legitimately contains a
  forbidden token
- **WHEN** the no-leak audit runs
- **THEN** that occurrence SHALL be excluded from the audit via
  the sentinel-comment scoping and SHALL NOT cause a failure

#### Scenario: Audit covers relocated prose assets

- **GIVEN** prose relocated from compiled code into a bundled asset
- **WHEN** the no-leak audit runs
- **THEN** that asset SHALL be included in the audited set

#### Scenario: Audit catches a vendor product-name leak

- **GIVEN** a bundled asset edited to add a vendor product name outside an allowed span
- **WHEN** the no-leak audit runs
- **THEN** the audit SHALL fail and SHALL identify the offending name plus its location

#### Scenario: Allowed span permits a deliberate CLI enumeration

- **GIVEN** a bundled asset that names specific CLIs inside an explicitly-allowed span
- **WHEN** the no-leak audit runs
- **THEN** those occurrences SHALL NOT cause a failure

### Requirement: Agent-facing prose is authored in bundled assets

Agent-facing prose SHALL be authored in bundled asset files rather than assembled from string literals in compiled code. Agent-facing prose means instructions, rules, directives, framing text and task-prompt scaffolding injected into an agent's sidecar, boot prompt, or skill content. Assets SHALL be embedded at compile time so the binary remains self-contained and renders correctly with no accompanying filesystem state.

Compiled code SHALL retain the assembly around that prose: placeholder substitution,
conditional inclusion, per-backend selection, deduplication, and region gating. The
requirement governs where sentences are *authored*, not how they are composed.

#### Scenario: Agent-facing prose lives in an asset

- **WHEN** the sources of agent-facing prose are inspected
- **THEN** the inter-agent rules, drive-loop directive, spec-path doctrine sentences, governance section header, boot-prompt framing, supervisor framing prompt, and task-prompt section scaffolding SHALL be sourced from bundled assets
- **AND** SHALL NOT be assembled from string literals in compiled code

#### Scenario: Assembly logic remains compiled

- **WHEN** prose is rendered from an asset
- **THEN** placeholder substitution, conditional inclusion, per-backend selection, deduplication and region gating SHALL be performed by compiled code

#### Scenario: Relocation does not change rendered output

- **GIVEN** prose relocated from compiled code into a bundled asset
- **WHEN** it is rendered under identical inputs
- **THEN** the rendered text SHALL be byte-identical to the text rendered before relocation

#### Scenario: Single-binary rendering preserved

- **WHEN** the binary runs with no accompanying asset files on disk
- **THEN** all relocated prose SHALL still render from the compile-time embedded copies
