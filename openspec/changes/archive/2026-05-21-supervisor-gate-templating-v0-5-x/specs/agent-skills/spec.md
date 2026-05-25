## ADDED Requirements

### Requirement: Skill template SHALL substitute six gate-command placeholders

`src/skills.rs::render` SHALL substitute the following placeholder strings in the supervisor skill template based on `[supervisor]` config values:

| Placeholder                  | Source field                              | When `None`         |
| ---------------------------- | ----------------------------------------- | ------------------- |
| `{{TEST_COMMAND}}`           | `[supervisor].test_command`               | `(not configured)`  |
| `{{LINT_COMMAND}}`           | `[supervisor].lint_command`               | `(not configured)`  |
| `{{BUILD_COMMAND}}`          | `[supervisor].build_command`              | `(not configured)`  |
| `{{DOC_BUILD_COMMAND}}`      | `[supervisor].doc_build_command`          | `(not configured)`  |
| `{{SPEC_VALIDATE_COMMAND}}`  | `[supervisor].spec_validate_command`      | `(not configured)`  |
| `{{FMT_CHECK_COMMAND}}`      | `[supervisor].fmt_check_command`          | `(not configured)`  |
| `{{SECURITY_AUDIT_COMMAND}}` | `[supervisor].security_audit_command`     | `(not configured)`  |

The substitution SHALL be plain string replacement (every occurrence of the placeholder in the template is replaced with the rendered value).

`{{CHANGE_ID}}` is NOT substituted by `render` — it passes through into the rendered skill verbatim. The supervisor agent SHALL substitute it at verification time using the change name it is currently auditing.

#### Scenario: Test command substitution with Some(value)

- **GIVEN** a skill template containing the literal string `Run {{TEST_COMMAND}} on the worktree.`
- **AND** a `SupervisorConfig` with `test_command = Some("just check")`
- **WHEN** `render` is called
- **THEN** the rendered output SHALL contain `Run just check on the worktree.`
- **AND** SHALL NOT contain the literal `{{TEST_COMMAND}}`

#### Scenario: Test command substitution with None

- **GIVEN** the same template
- **AND** a `SupervisorConfig` with `test_command = None`
- **WHEN** `render` is called
- **THEN** the rendered output SHALL contain `Run (not configured) on the worktree.`

#### Scenario: All six new placeholders are substituted

- **GIVEN** a template containing each of the six new placeholders (`{{LINT_COMMAND}}`, `{{BUILD_COMMAND}}`, `{{DOC_BUILD_COMMAND}}`, `{{SPEC_VALIDATE_COMMAND}}`, `{{FMT_CHECK_COMMAND}}`, `{{SECURITY_AUDIT_COMMAND}}`) at least once
- **AND** a `SupervisorConfig` with corresponding fields set to `Some("CMD-N")` for each
- **WHEN** `render` is called
- **THEN** every placeholder SHALL be replaced with its corresponding `CMD-N` value
- **AND** the rendered output SHALL NOT contain any of the original `{{...}}` placeholder strings

#### Scenario: CHANGE_ID placeholder passes through unrendered

- **GIVEN** a `spec_validate_command = Some("openspec validate {{CHANGE_ID}} --strict")` in `SupervisorConfig`
- **AND** a template containing `Run {{SPEC_VALIDATE_COMMAND}} for the change being audited.`
- **WHEN** `render` is called
- **THEN** the rendered output SHALL contain `Run openspec validate {{CHANGE_ID}} --strict for the change being audited.`
- **AND** the `{{CHANGE_ID}}` substring SHALL still appear verbatim in the rendered output (it is NOT a render-time placeholder)

### Requirement: Supervisor skill prose SHALL use placeholders, not hardcoded commands

The embedded `assets/agent-skills/supervisor.md` SHALL reference gate commands exclusively via the placeholder names listed above, not via the hardcoded `just`, `cargo`, `mdbook`, or `openspec` invocations. The gate prose SHALL state explicitly that when a placeholder renders as `(not configured)`, the agent SHALL skip the tooling invocation but the gate's manual review (e.g. spec scenario coverage check, OWASP-category diff scan) still applies.

#### Scenario: Rendered skill uses placeholders for all five gates

- **GIVEN** the embedded supervisor skill rendered with all six new `[supervisor]` fields set to `Some("CMD-N")`
- **WHEN** the rendered content is inspected
- **THEN** the content SHALL contain a `Gate 1`/`Testing` section that uses `CMD-N` (the substituted `{{TEST_COMMAND}}`)
- **AND** the content SHALL contain a `Doc audit` section that uses the substituted `{{DOC_BUILD_COMMAND}}`
- **AND** the content SHALL contain a `Security audit` section that uses the substituted `{{SECURITY_AUDIT_COMMAND}}`
- **AND** the content SHALL NOT contain a hardcoded `just check`, `mdbook build`, `cargo audit`, `cargo clippy`, `openspec validate`, or `cargo fmt --check` string outside of placeholder context

#### Scenario: Not-configured gates render as graceful skip prose

- **GIVEN** the embedded supervisor skill rendered with all six new fields set to `None`
- **WHEN** the rendered content is inspected
- **THEN** the content SHALL contain the literal phrase `(not configured)` in each of the five gate sections
- **AND** the content SHALL contain prose stating that gates whose commands render as `(not configured)` SHALL skip the tooling invocation but still apply manual review

#### Scenario: Hardcoded git-paw command audit

- **WHEN** the embedded supervisor skill template (pre-render) is read from `assets/agent-skills/supervisor.md`
- **THEN** no occurrence of `just check`, `cargo test`, `cargo clippy`, `cargo audit`, `cargo fmt --check`, `mdbook build` SHALL appear in the gate prose (those literal strings only appear inside the `{{...}}` placeholder definitions or in fenced code blocks demonstrating EXAMPLE values like `# [supervisor]\n# test_command = "just check"`)
