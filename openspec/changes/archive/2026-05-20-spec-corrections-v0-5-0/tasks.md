# Tasks — spec-corrections-v0-5-0

Three independent corrections. Each correction is self-contained — none depend
on the others. The change is "spec only" with one code-side doc-comment fix
called out explicitly under task 3.

## 1. Correction 1 — conflict-detection auto-emitted question convention

- [x] 1.1 Open `openspec/specs/conflict-detection/spec.md` and locate the
      `### Requirement: Auto-emitted message conventions` block (~line 189).
- [x] 1.2 Replace the second bullet of the requirement preamble. Old text:

      ```
      - `agent.question` messages emitted to the supervisor inbox SHALL set
        `agent_id = "supervisor"` (the recipient), `payload.from = "supervisor"`
        (matching v0.4 supervisor-originated convention), and SHALL include
        `[conflict-detector]` as a token in the question text.
      ```

      New text (matches the delta in `specs/conflict-detection/spec.md` of this
      change):

      ```
      - `agent.question` messages emitted to the supervisor inbox SHALL set
        `agent_id = "supervisor"` (the recipient — and, by the auto-emitted-
        detector convention, the sender-identification slot for this variant,
        since `QuestionPayload` has no `from` field), and SHALL include
        `[conflict-detector]` as a token in the question text.
      ```

- [x] 1.3 Preserve the existing two scenarios verbatim ("Auto-emitted feedback
      uses supervisor as the from field" and "Auto-emitted question is
      addressed to the supervisor inbox"). Their assertions are still correct
      and need no change.
- [x] 1.4 Append the new scenario "Auto-emitted question payload has no from
      field" from the delta. This locks the corrected behaviour into a test
      hook.
- [x] 1.5 Verify no other spec or doc references the obsolete
      `payload.from = "supervisor"` claim for `agent.question`. Run
      `grep -rn "payload.from" openspec/specs/ | grep -i question`. Expected:
      no hits.

## 2. Correction 2 — agent-skills render() signature and TEST_COMMAND scenarios

- [x] 2.1 Open `openspec/specs/agent-skills/spec.md` and locate the
      `### Requirement: Skill template rendering` block (~line 147).
- [x] 2.2 Replace the requirement preamble and signature block. Old text:

      ```
      The `render()` function SHALL accept an additional `project: &str`
      parameter and substitute `{{PROJECT_NAME}}` with the project name
      alongside the existing `{{BRANCH_ID}}` substitution.

      The function signature SHALL be:
      `pub fn render(template: &SkillTemplate, branch: &str, broker_url: &str, project: &str) -> String`
      ```

      New text (matches the delta in `specs/agent-skills/spec.md` of this
      change): the preamble now also references `test_command` and the
      `{{TEST_COMMAND}}` placeholder, the signature has a fifth
      `test_command: Option<&str>` parameter, and the `None → "(not configured)"`
      behaviour is documented inline.

- [x] 2.3 Update the two existing scenarios ("PROJECT_NAME placeholder is
      substituted" and "Both BRANCH_ID and PROJECT_NAME substituted") so the
      `render(...)` call carries the new fifth positional argument (use `None`
      for these scenarios — they do not exercise `{{TEST_COMMAND}}` and `None`
      preserves their semantics).
- [x] 2.4 Append the two new scenarios from the delta:
      - "TEST_COMMAND placeholder is substituted when test_command is Some"
      - "TEST_COMMAND placeholder substitutes a literal when test_command is None"
- [x] 2.5 The "Both BRANCH_ID and PROJECT_NAME substituted" scenario's
      final assertion currently reads
      `no {{...}} placeholders remain (except {{TEST_COMMAND}} which is handled externally)`.
      The "handled externally" caveat is no longer accurate — `{{TEST_COMMAND}}`
      is now handled by `render` directly. Drop the parenthetical so the
      assertion becomes `no {{...}} placeholders remain` (the test template
      under this scenario does not contain `{{TEST_COMMAND}}`, so the
      assertion is satisfiable).
- [x] 2.6 Verify the `Unknown placeholder warning` requirement (~line 167)
      still reads correctly given that `{{TEST_COMMAND}}` is now always
      consumed when `render` runs. No change should be needed — the warning
      requirement is generic across all unknown placeholders and is unaffected
      by which placeholders the renderer happens to handle natively.
- [x] 2.7 No amendment needed to `configuration/spec.md` or
      `supervisor-config/spec.md`. `test_command` is already specified at
      `supervisor-config/spec.md:12` (`test_command: Option<String> — defaults
      to None when absent`) and the generated-config scenario at
      `supervisor-config/spec.md:103` already references it. Confirm by
      `grep test_command openspec/specs/supervisor-config/spec.md` and stop
      if the expected lines are present.

## 3. Correction 3 — broker-messages envelope enumerates seven variants

- [x] 3.1 Open `openspec/specs/broker-messages/spec.md` and locate the
      `### Requirement: Broker message envelope` block (line 6).
- [x] 3.2 Replace the requirement preamble. Old text:

      ```
      The type SHALL be a Rust enum with three variants — `Status`, `Artifact`,
      and `Blocked` — each carrying an `agent_id: String` and a strongly-typed
      payload struct.

      The wire format SHALL be JSON with an internally tagged discriminator
      field named `type`, taking the values `agent.status`, `agent.artifact`,
      or `agent.blocked`. ...
      ```

      New text (matches the delta in `specs/broker-messages/spec.md` of this
      change): seven variants enumerated, all seven `type` discriminator
      values listed.

- [x] 3.3 Preserve the existing four scenarios verbatim. Their assertions
      remain correct under the new wording.
- [x] 3.4 Append the new scenario "Envelope enumerates all seven wire-format
      type values" from the delta.
- [x] 3.5 **Code-side sub-task** — fix the parallel doc-comment drift in
      `src/broker/messages.rs:145-146`. Current text:

      ```rust
      /// The wire format uses JSON with an internally tagged `"type"` discriminator
      /// whose values are `"agent.status"`, `"agent.artifact"`, and `"agent.blocked"`.
      ```

      New text — enumerate all seven:

      ```rust
      /// The wire format uses JSON with an internally tagged `"type"` discriminator
      /// whose values are `"agent.status"`, `"agent.artifact"`, `"agent.blocked"`,
      /// `"agent.verified"`, `"agent.feedback"`, `"agent.question"`, and
      /// `"agent.intent"`.
      ```

      Then run `cargo fmt` and `cargo build` to confirm the doc-comment still
      compiles. No behavioural code is touched.

- [x] 3.6 Verify no other spec, doc, or README has the same stale
      "three variants" claim. Run:

      ```
      grep -rn "three variants" openspec/ docs/ README.md src/ 2>/dev/null
      ```

      Investigate each hit; drop the "three" qualifier or amend to the
      current count if any survive.

## 4. Validate

- [x] 4.1 Run `openspec validate spec-corrections-v0-5-0 --strict` and
      address any reported errors.
- [x] 4.2 Run `just check` to confirm the doc-comment edit in task 3.5 does
      not break the build, clippy, or tests. (No behaviour changes, so no test
      additions are needed under this change beyond what already exists.)
- [x] 4.3 Spot-check by reading the three corrected main-spec sections back
      and confirming the wording matches the delta. The delta files in
      `openspec/changes/spec-corrections-v0-5-0/specs/<capability>/spec.md`
      are the canonical "after" wording.
