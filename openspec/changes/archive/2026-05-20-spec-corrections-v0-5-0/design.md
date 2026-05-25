# Design — spec-corrections-v0-5-0

## Context

Three small spec-vs-code drifts found in v0.5.0 audit. The code is correct;
the specs are stale or imprecise. Each correction is independent and amends a
single requirement block in a single capability spec. No new wire formats, no
new routing rules, no new validation paths.

This document captures the *mechanism* for each correction — what the old
wording said, what the new wording says, and the rationale.

## D1 — Correction 1 mechanism: drop `payload.from` for auto-emitted questions

### Old wording (current spec, conflict-detection §"Auto-emitted message conventions")

> `agent.question` messages emitted to the supervisor inbox SHALL set
> `agent_id = "supervisor"` (the recipient), `payload.from = "supervisor"`
> (matching v0.4 supervisor-originated convention), and SHALL include
> `[conflict-detector]` as a token in the question text.

### Problem

`QuestionPayload` has no `from` field. The "v0.4 supervisor-originated
convention" the wording invokes is from `agent.feedback`, where `from` is a
real field on `FeedbackPayload`. The drafter conflated the two payloads.

### New wording

> `agent.question` messages emitted to the supervisor inbox SHALL set
> `agent_id = "supervisor"` (the recipient and, by convention, the
> sender-identification slot for auto-emitted detector messages — there is
> no `payload.from` field on `QuestionPayload`), and SHALL include
> `[conflict-detector]` as a token in the question text.

The auto-emitted-feedback bullet immediately above still references
`payload.from = "supervisor"` because `FeedbackPayload` does have a `from`
field — that bullet is correct and is preserved verbatim in the MODIFIED
requirement block.

The supporting scenario "Auto-emitted question is addressed to the supervisor
inbox" already only asserts `agent_id = "supervisor"` and the `[conflict-detector]`
token — no scenario assertion needs to change. The change is exclusively in
the prose preamble of the requirement.

### Rejected alternative: add a `from` field to `QuestionPayload`

This was considered and rejected. Adding a `from` field would:

- Be a wire-format change for every `agent.question` (every coding agent's
  `agent.question` curl would need the new field).
- Require a validation amendment in `broker-messages` (Validation for Question
  variant).
- Force a skill-prose update across every coding-agent skill that shows an
  `agent.question` curl example.
- Add a backward-compat shim to silently accept the new field as `None`.

All of that effort for a field whose information is already conveyed by the
envelope `agent_id`. Not worth it.

## D2 — Correction 2 mechanism: 5-parameter `render`, including `test_command`

### Old wording (current spec, agent-skills §"Skill template rendering")

```rust
pub fn render(template: &SkillTemplate, branch: &str, broker_url: &str, project: &str) -> String
```

Two scenarios exist: "PROJECT_NAME placeholder is substituted" and "Both
BRANCH_ID and PROJECT_NAME substituted". Both pass `render(template, "feat/x",
"http://127.0.0.1:9119", "my-app")` — four positional arguments.

### Problem

Shipped signature has five parameters; the fifth, `test_command: Option<&str>`,
drives `{{TEST_COMMAND}}` substitution for the supervisor skill (the supervisor
skill renders the user's configured `test_command` value at boot time so the
"merge orchestration" and "spec audit" prose contain the literal command
string, not a placeholder).

When `test_command` is `None`, `render` substitutes the literal `"(not
configured)"` string (`src/skills.rs:428`).

### New wording

```rust
pub fn render(
    template: &SkillTemplate,
    branch: &str,
    broker_url: &str,
    project: &str,
    test_command: Option<&str>,
) -> String
```

Existing scenarios are amended to pass the new fifth argument (`None` is the
no-op-equivalent value that preserves the previous behaviour for tests that
don't exercise the new placeholder).

A new scenario is added:

> #### Scenario: TEST_COMMAND placeholder is substituted
> - **GIVEN** a `SkillTemplate` whose content contains `run {{TEST_COMMAND}} after merge`
> - **WHEN** `render(template, "feat/x", "url", "proj", Some("just check"))` is called
> - **THEN** the resulting string contains `run just check after merge`
> - **AND** the resulting string contains no `{{TEST_COMMAND}}`

And a second new scenario for the `None` case:

> #### Scenario: TEST_COMMAND placeholder substitutes a literal when test_command is None
> - **GIVEN** a `SkillTemplate` whose content contains `run {{TEST_COMMAND}} after merge`
> - **WHEN** `render(template, "feat/x", "url", "proj", None)` is called
> - **THEN** the resulting string contains `run (not configured) after merge`
> - **AND** the resulting string contains no `{{TEST_COMMAND}}`
> - **AND** no `{{TEST_COMMAND}}` warning is written to standard error

The third assertion ("no warning") is important: the unknown-placeholder
warning in §"Unknown placeholder warning" only fires for substrings that
*remain* in the rendered output. Substituting `(not configured)` consumes the
placeholder, so the warning path is silent — that's the desired behaviour and
the scenario pins it.

### Configuration spec impact

`test_command` is already in `supervisor-config/spec.md:12` and referenced
across `configuration/spec.md:351`. No second-capability amendment is needed
under this change — the field exists, the type is correct, the default is
correct, and the spec text accurately describes how it threads into the
generated default config. Only `agent-skills` needs amending.

## D3 — Correction 3 mechanism: enumerate all seven variants

### Old wording (current spec, broker-messages §"Broker message envelope")

> The type SHALL be a Rust enum with three variants — `Status`, `Artifact`,
> and `Blocked` — each carrying an `agent_id: String` and a strongly-typed
> payload struct.
>
> The wire format SHALL be JSON with an internally tagged discriminator field
> named `type`, taking the values `agent.status`, `agent.artifact`, or
> `agent.blocked`. Every message SHALL include `agent_id` and `payload`
> fields at the top level alongside `type`.

### Problem

The enum has seven variants: `Status`, `Artifact`, `Blocked`, `Verified`,
`Feedback`, `Question`, `Intent`. Each is governed by its own requirement
block in the same spec file. A reader who stops at the envelope requirement
would conclude there are only three.

### Decision — enumerate all seven (not "core + extension")

Two wording options were considered:

1. **Enumerate all seven** — list every variant and every wire `type` value
   in the envelope requirement.
2. **Core + extension** — say "the core variants are `Status`/`Artifact`/
   `Blocked`; additional variants — `Verified`, `Feedback`, `Question`,
   `Intent` — are defined in dedicated requirements below."

Option 1 is chosen. Rationale:

- The "core vs extension" framing implies a hierarchy or capability gate
  that doesn't exist in code. All seven variants are first-class in the
  enum, share the same serde-tag mechanism, share the same `from_json`
  validation entry point, share the same `Display` impl, and share the same
  routing layer.
- Future-proofing the requirement against "what if we add an 8th variant?"
  is not worth the imprecision now. A future change that adds a variant
  will add a requirement block for it AND amend this envelope requirement
  to enumerate it (just like every existing extension variant did when it
  shipped — `Verified`, `Feedback`, `Question`, `Intent` all extended the
  envelope wording in their respective change proposals).
- Symmetry — the rest of the spec has one requirement per variant. The
  envelope requirement enumerating all seven is the natural index.

### New wording

> The type SHALL be a Rust enum with seven variants — `Status`, `Artifact`,
> `Blocked`, `Verified`, `Feedback`, `Question`, and `Intent` — each
> carrying an `agent_id: String` and a strongly-typed payload struct.
>
> The wire format SHALL be JSON with an internally tagged discriminator field
> named `type`, taking the values `agent.status`, `agent.artifact`,
> `agent.blocked`, `agent.verified`, `agent.feedback`, `agent.question`, or
> `agent.intent`. Every message SHALL include `agent_id` and `payload`
> fields at the top level alongside `type`.

The "Unknown message type is rejected" scenario stays correct — it asserts
that `"agent.unknown"` fails, which is still true (and now even more clearly
so, since the new wording lists every accepted value).

### Code-side doc-comment

`src/broker/messages.rs:145-146` carries the same drift in a Rust doc-comment:

```rust
/// The wire format uses JSON with an internally tagged `"type"` discriminator
/// whose values are `"agent.status"`, `"agent.artifact"`, and `"agent.blocked"`.
```

This is corrected as a sub-task under task 3 in `tasks.md`. It is the only
code-side change in this entire spec correction. It is purely a comment;
behaviour is unchanged.

## Open questions

None. All three corrections have a clear shipping behaviour to align the spec
to, and none introduce a new design decision beyond the wording trade-off
captured in D3 (resolved: enumerate all seven).
