# spec-corrections-v0-5-0

## Why

A post-merge audit of the v0.5.0 main spec set under `openspec/specs/` against
the shipped code surfaced three small spec-vs-code drifts. None of them block
the release — the code is correct, the wire format is stable, and downstream
consumers (agents, supervisor, dashboards) behave as documented in the user-
facing skill prose. But each drift is a future-trap: the next contributor who
reads the spec before the code will write tests against, or implement against,
the wrong shape.

This change reconciles the three drifts by amending the three specs to match
the shipped behaviour. No code changes ship as part of this change; the only
"implementation" line item is a doc-comment correction in
`src/broker/messages.rs` whose enum doc-comment lists only three variants while
the enum body has seven.

The corrections were found while reviewing v0.5.0 against the merged broker
conflict-detector, skill renderer, and `agent.intent`/`agent.question` plumbing.

## What Changes

### Correction 1 — `conflict-detection`: drop `payload.from = "supervisor"` for `agent.question`

`conflict-detection/spec.md` (Auto-emitted message conventions requirement,
~line 194) asserts that auto-emitted `agent.question` messages SHALL set
`payload.from = "supervisor"`. The current `QuestionPayload` (defined in
`broker-messages` and implemented in `src/broker/messages.rs:111-115`) has a
single field — `question: String` — and no `from` field at all. The shipped
`emit_question` in `src/broker/conflict.rs:440-448` builds
`QuestionPayload { question: ... }` only.

The actual sender-identification convention for auto-emitted questions is at
the envelope level: `agent_id = "supervisor"` (per the `CONFLICT_DETECTOR_SENDER`
constant at `src/broker/conflict.rs:41`), since `Question` messages route into
the supervisor inbox and use `agent_id` as the recipient label by convention.

**Fix**: amend `conflict-detection/spec.md` to drop the `payload.from = "supervisor"`
claim and replace it with `agent_id = "supervisor"`. Do NOT add a `from` field
to `QuestionPayload` — that would be a wider change with no justification (no
caller needs it; the envelope `agent_id` carries the same information).

### Correction 2 — `agent-skills`: `render()` actually has five parameters

`agent-skills/spec.md` (Skill template rendering requirement, ~line 151) pins
the signature of `render()` as four parameters:

```rust
pub fn render(template: &SkillTemplate, branch: &str, broker_url: &str, project: &str) -> String
```

The shipped signature in `src/skills.rs:420-426` is five parameters — the
extra `test_command: Option<&str>` drives the `{{TEST_COMMAND}}` placeholder
substitution. When `test_command` is `None`, the placeholder substitutes to
the literal string `"(not configured)"` (see `src/skills.rs:428`). Call sites
at `src/main.rs:870` and `src/main.rs:933` pass `supervisor_cfg.test_command.as_deref()`.

**Fix**: amend `agent-skills/spec.md` to document the actual 5-parameter
signature and add a scenario covering `{{TEST_COMMAND}}` substitution including
the `None → "(not configured)"` behaviour. The `test_command` field itself is
already specified in `supervisor-config/spec.md:12`, so no second-capability
amendment is required.

### Correction 3 — `broker-messages`: envelope says "three variants" but enum has seven

`broker-messages/spec.md:6-10` (Broker message envelope requirement) describes
`BrokerMessage` as having "three variants — `Status`, `Artifact`, and
`Blocked`" and enumerates only `agent.status`/`agent.artifact`/`agent.blocked`
as wire `type` values. The shipped enum has seven variants — the same three
plus `Verified`, `Feedback`, `Question`, and `Intent` (each governed by its
own requirement block later in the same spec).

The parallel drift exists in `src/broker/messages.rs:145-146` doc-comment:

```rust
/// The wire format uses JSON with an internally tagged `"type"` discriminator
/// whose values are `"agent.status"`, `"agent.artifact"`, and `"agent.blocked"`.
```

**Fix**: amend the envelope requirement to enumerate all seven variants. The
trade-off "list all seven" vs "say core + extension" is resolved in design.md
D3 — we pick the explicit enumeration so a reader of the envelope requirement
gets the complete wire-format picture without chasing forward references. The
doc-comment in `src/broker/messages.rs:145-146` is also corrected as a sub-task
under correction 3 in `tasks.md`.

## Impact

- **Affected capabilities**: `conflict-detection`, `agent-skills`, `broker-messages`
- **Affected code** (sole code-side touchup): `src/broker/messages.rs:145-146` doc-comment.
  No behavioural code changes.
- **No new requirements** — every change is a MODIFIED requirement that
  re-states the existing requirement block with corrected wording. No
  scenario is dropped; some scenarios are corrected to match the new wording.
- **No wire-format change** — the JSON wire shape, validation rules, routing
  rules, and field names are all unchanged. This change is purely a spec text
  correction so the spec matches the code that already shipped in v0.5.0.
