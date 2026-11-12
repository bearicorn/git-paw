## MODIFIED Requirements

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
paths — at minimum `Claude Code` and `.claude/` — outside an explicitly-allowed span or
that vendor's own CLI fragment. A bundled asset may name a specific CLI only inside such a
span (for example, an enumeration of known CLIs), or inside the per-CLI fragment of that
CLI's own family — a package file under `assets/cli/<family>/`, rendered only for agents of
that family (see *Vendor naming is confined to the matching CLI fragment*) — so that naming
a vendor is a deliberate, reviewable act rather than an accident.

#### Scenario: Audit passes on the v0.6.0 supervisor template

- **WHEN** the no-leak audit runs against the
  v0.6.0-as-shipped supervisor skill rendered for each backend
- **THEN** the audit SHALL pass

Test: `tests/lang_agnostic_skill_audit.rs::audit_passes_for_openspec_backend`, `tests/lang_agnostic_skill_audit.rs::audit_passes_for_speckit_backend`, `tests/lang_agnostic_skill_audit.rs::audit_passes_for_markdown_backend`, `tests/lang_agnostic_skill_audit.rs::audit_passes_for_multi_backend_session`, `tests/lang_agnostic_skill_audit.rs::audit_passes_for_no_backend_session`

#### Scenario: Audit catches a Rust-leak regression

- **GIVEN** a supervisor skill template edited to add a literal
  `cargo test` outside the allowed-span sentinels
- **WHEN** the no-leak audit runs
- **THEN** the audit SHALL fail and SHALL identify the offending
  token plus its location

Test: `tests/lang_agnostic_skill_audit.rs::audit_catches_a_rust_leak_regression`

#### Scenario: Sentinel-comment scope excludes an allowed span

- **GIVEN** a bundled asset whose `<!-- allowlist-prose -->` …
  `<!-- /allowlist-prose -->` span legitimately contains a
  forbidden token
- **WHEN** the no-leak audit runs
- **THEN** that occurrence SHALL be excluded from the audit via
  the sentinel-comment scoping and SHALL NOT cause a failure

Test: `tests/lang_agnostic_skill_audit.rs::audit_excludes_allowlist_prose_via_sentinel`

#### Scenario: Audit covers relocated prose assets

- **GIVEN** prose relocated from compiled code into a bundled asset
- **WHEN** the no-leak audit runs
- **THEN** that asset SHALL be included in the audited set

Test: `tests/lang_agnostic_skill_audit.rs::audit_covers_every_relocated_prose_asset`

#### Scenario: Audit catches a vendor product-name leak

- **GIVEN** a bundled asset edited to add a vendor product name outside an allowed span
- **WHEN** the no-leak audit runs
- **THEN** the audit SHALL fail and SHALL identify the offending name plus its location

Test: `tests/lang_agnostic_skill_audit.rs::audit_catches_a_vendor_product_name_leak`

#### Scenario: Allowed span permits a deliberate CLI enumeration

- **GIVEN** a bundled asset that names specific CLIs inside an explicitly-allowed span
- **WHEN** the no-leak audit runs
- **THEN** those occurrences SHALL NOT cause a failure

Test: `tests/lang_agnostic_skill_audit.rs::allowed_span_permits_the_paste_buffer_cli_enumeration`

#### Scenario: A CLI fragment may name its own vendor

- **GIVEN** the `claude` boot fragment at `assets/cli/claude/boot.md`, which names its vendor
- **WHEN** the no-leak audit reads it as a package file
- **THEN** it SHALL pass as that family's fragment
- **AND** the same text read from a path outside `assets/cli/<family>/` SHALL fail

Test: `tests/lang_agnostic_skill_audit.rs::a_package_file_under_assets_cli_is_read_as_its_familys_fragment`

## ADDED Requirements

### Requirement: Vendor naming is confined to the matching CLI fragment

The no-language-leak audit SHALL render every template that uses a CLI placeholder once per registered CLI family and once for `generic`. It SHALL allow a vendor product name or vendor-specific path only in text from that family's own fragment, including when it reads a package's `assets/cli/<family>/` files one by one. Base templates and every `generic` fragment SHALL contain no vendor product name or path outside an explicitly-allowed span.

#### Scenario: Generic rendering is vendor-free

- **WHEN** the coordination skill and both boot-block forms are rendered with the `generic` fragment
- **THEN** the audit SHALL find no vendor product name or vendor path outside an allowed span

Test: `tests/lang_agnostic_skill_audit.rs::generic_rendering_is_vendor_free`, `tests/lang_agnostic_skill_audit.rs::cli_placeholder_templates_pass_the_audit_for_every_family`

#### Scenario: A fragment naming another vendor fails

- **GIVEN** a copy of the `claude` fragment edited to name a different vendor's CLI
- **WHEN** the audit runs
- **THEN** it SHALL fail naming the fragment and the offending token

Test: `tests/lang_agnostic_skill_audit.rs::a_fragment_naming_another_vendor_fails`

#### Scenario: A vendor name in a base template fails

- **GIVEN** a copy of the coordination `SKILL.md` with a vendor product name added outside any fragment and any allowed span
- **WHEN** the audit runs
- **THEN** it SHALL fail naming the offending token

Test: `tests/lang_agnostic_skill_audit.rs::a_vendor_name_in_a_base_template_fails`

