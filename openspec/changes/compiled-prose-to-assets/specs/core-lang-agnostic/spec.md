## ADDED Requirements

### Requirement: Agent-facing prose is authored in bundled assets

Agent-facing prose SHALL be authored in bundled asset files rather than assembled from string literals in compiled code. Agent-facing prose means instructions, rules, directives, framing text and task-prompt scaffolding injected into an agent's sidecar, boot prompt, or skill content. Assets SHALL be embedded at compile time so the binary remains self-contained and renders correctly with no accompanying filesystem state.

Compiled code SHALL retain the assembly around that prose: placeholder substitution,
conditional inclusion, per-backend selection, deduplication, and region gating. The
requirement governs where sentences are *authored*, not how they are composed.

Prose that is **generated from a compiled constant specifically so it cannot drift from
that constant** SHALL remain compiled. The rendered dev-allowlist enumeration is the
governing example: deriving it from the allowlist constant is what guarantees the
documented allowlist matches what is actually auto-approved, and a hand-editable asset
would let the two diverge silently and widen the approval surface.

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

#### Scenario: Constant-derived prose stays compiled

- **WHEN** the dev-allowlist enumeration is rendered
- **THEN** it SHALL be generated from the compiled allowlist constant
- **AND** SHALL NOT be sourced from a separately editable asset

## MODIFIED Requirements

### Requirement: No language-leak audit

The CI test suite SHALL include a no-leak audit that renders the
supervisor skill against fixture configurations for each spec
backend and SHALL assert no token from a forbidden list appears
in the rendered output outside explicitly-allowed spans. The
forbidden list SHALL include at minimum `cargo` (outside allowlist
prose), `rustdoc`, `.rs:` (as a hardcoded source-path marker),
`Cargo.toml`, and `rustc`.

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
