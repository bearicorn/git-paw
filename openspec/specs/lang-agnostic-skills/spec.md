# lang-agnostic-skills Specification

## Purpose
TBD - created by archiving change lang-agnostic-assets. Update Purpose after archive.
## Requirements
### Requirement: New supervisor config keys for doc tooling

The system SHALL accept new `[supervisor]` config fields
`doc_tool_command` (string, optional, default empty) and SHALL
accept additional doc-related fields the apply phase determines
are necessary (e.g. `apidoc_command`). The fields SHALL be loaded
into the supervisor session context for use by skill rendering.

#### Scenario: Default empty value loads cleanly

- **GIVEN** a `.git-paw/config.toml` with no `doc_tool_command`
  field
- **WHEN** the supervisor session loads its configuration
- **THEN** `doc_tool_command` SHALL resolve to an empty string
  with no validation error

#### Scenario: Explicit value is preserved verbatim

- **GIVEN** `[supervisor].doc_tool_command = "sphinx-build -W docs
  docs/_build"`
- **WHEN** the supervisor session loads its configuration
- **THEN** the value SHALL be available to skill rendering verbatim,
  including all whitespace and quoting

### Requirement: New placeholder substitutions in skill rendering

The skill rendering pipeline SHALL substitute three new
placeholders in the bundled supervisor skill:
`{{DOC_TOOL_COMMAND}}`, `{{DEV_ALLOWLIST_PRESET}}`, and
`{{SPEC_PATH_DOCTRINE}}`. Substitutions SHALL be applied via the
existing `{{...}}` substitution machinery used for the v0.5.0
five-gate placeholders.

#### Scenario: DOC_TOOL_COMMAND substitutes config value

- **GIVEN** `[supervisor].doc_tool_command = "cargo doc --no-deps"`
- **WHEN** the supervisor skill is rendered for a session
- **THEN** every occurrence of `{{DOC_TOOL_COMMAND}}` SHALL be
  replaced with `cargo doc --no-deps` in the rendered skill body

#### Scenario: DEV_ALLOWLIST_PRESET renders from constant

- **WHEN** the supervisor skill is rendered
- **THEN** `{{DEV_ALLOWLIST_PRESET}}` SHALL be replaced with a
  prose enumeration generated from
  `src/supervisor/dev_allowlist::DEV_ALLOWLIST_PRESET`, and the
  rendered enumeration SHALL include every constant entry such
  that adding a new entry to the constant changes the rendered
  output without a skill-template edit

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

### Requirement: Backwards-compatible empty-substitution rendering

The supervisor skill template SHALL be authored such that an empty
substitution value still reads naturally. The system SHALL NOT
require `[supervisor].doc_tool_command` (or any new placeholder
source) to be set for the skill to render usefully.

#### Scenario: Empty DOC_TOOL_COMMAND produces readable prose

- **GIVEN** no `doc_tool_command` configured
- **WHEN** the supervisor skill is rendered
- **THEN** the rendered output containing the substitution SHALL
  read as a complete, grammatical sentence (no dangling backticks,
  no broken phrasing)

#### Scenario: Empty SPEC_PATH_DOCTRINE produces a sentinel sentence

- **GIVEN** a session that has not resolved any spec backend
- **WHEN** the supervisor skill is rendered
- **THEN** `{{SPEC_PATH_DOCTRINE}}` SHALL render a sentinel
  sentence explaining that no spec backend has been resolved,
  rather than producing empty whitespace

### Requirement: No language-leak audit

The CI test suite SHALL include a no-leak audit that renders the
supervisor skill against fixture configurations for each spec
backend and SHALL assert no token from a forbidden list appears
in the rendered output outside explicitly-allowed spans. The
forbidden list SHALL include at minimum `cargo` (outside allowlist
prose), `rustdoc`, `.rs:` (as a hardcoded source-path marker),
`Cargo.toml`, and `rustc`.

#### Scenario: Audit passes on the v0.6.0 supervisor template

- **WHEN** the no-leak audit runs against the
  v0.6.0-as-shipped supervisor skill rendered for each backend
- **THEN** the audit SHALL pass

#### Scenario: Audit catches a Rust-leak regression

- **GIVEN** a supervisor skill template edited to add a literal
  `cargo test` outside the allowlist-prose sentinel span
- **WHEN** the no-leak audit runs
- **THEN** the audit SHALL fail and SHALL identify the offending
  token plus its location

#### Scenario: Sentinel-comment scope excludes allowlist prose

- **GIVEN** the rendered `{{DEV_ALLOWLIST_PRESET}}` enumeration
  legitimately contains `cargo test` as one entry
- **WHEN** the no-leak audit runs
- **THEN** that occurrence SHALL be excluded from the audit via
  the sentinel-comment scoping and SHALL NOT cause a failure

### Requirement: Tone and example discipline in bundled skills

Bundled skill content SHALL avoid meta-commentary that names an
implementation language. The bundled supervisor and coordination
skills SHALL NOT use Rust as the default illustrative stack across
consecutive examples. Example bodies SHALL rotate through a
stack-agnostic set covering at minimum a test-runner failure, a
lint/format failure, and a type-check or compile failure.

#### Scenario: No meta-commentary names a language

- **WHEN** the bundled supervisor and coordination skills are
  inspected for meta-commentary (e.g. "we removed the X" or
  "the legacy Y" patterns)
- **THEN** no such commentary SHALL name "Rust", "cargo", or any
  other stack-specific term

#### Scenario: Examples cover at least three failure shapes

- **WHEN** the `agent.feedback` examples in supervisor.md and
  coordination.md are enumerated
- **THEN** the examples SHALL collectively cover at least three
  distinct shapes: a test-runner failure, a lint/format failure,
  and a type-check or compile failure

