## 1. Boot-prompt section renderer

- [x] 1.1 In the supervisor boot-prompt module (`src/supervisor/boot.rs` or wherever the existing boot-prompt builder lives), add a pure helper `pub fn governance_section(governance: &GovernanceConfig) -> String`.
- [x] 1.2 The helper SHALL return `String::new()` when all five path fields are `None`.
- [x] 1.3 When at least one path is set, the helper SHALL produce a string containing:
  - The heading `## Governance documents`.
  - A short preamble line indicating the supervisor consults these docs during spec audit.
  - One bullet per configured path, in canonical order: `adr`, `test_strategy`, `security`, `dod`, `constitution`. Bullet format: `- <doc>: <path>`.
  - Excluding any doc whose path is `None` from the bullet list.
  - NO "gates" sub-line, NO gate-flag summary, NO per-doc enforcement metadata.

## 2. Boot-prompt wiring

- [x] 2.1 In the existing boot-prompt builder (the function that assembles the supervisor's full boot prompt), call `governance_section(&config.governance)` and append the result after the supervisor skill content. Insert a blank line separator before the section header.
- [x] 2.2 When `governance_section` returns an empty string, the boot prompt is unchanged from v0.4 — no extra blank line, no placeholder header.

## 3. Boot-prompt tests

- [x] 3.1 `governance_section` returns empty string when `GovernanceConfig::default()` is passed.
- [x] 3.2 With `dod = Some("docs/dod.md")` and other paths `None`: returns a section containing `## Governance documents`, a bullet for `dod`, and does NOT mention `adr`, `test_strategy`, `security`, or `constitution`.
- [x] 3.3 With all five paths populated: bullets list all five in canonical order.
- [x] 3.4 Output does NOT contain a "Gated docs" line, "Governance gates" sub-section, or any text referencing per-doc gate flags.
- [x] 3.5 Boot-prompt builder integration test: full boot prompt for a session with no `[governance]` config matches v0.4 baseline (no governance content). Same builder with a populated `[governance]` config inserts the section between the skill content and any subsequent task content.

## 4. Embedded supervisor skill update

- [x] 4.1 In `assets/agent-skills/supervisor.md`, locate the existing `### Spec Audit Procedure` section (or wherever spec audit is described). Add a sub-section (or extend the existing list) with governance-verification guidance per the spec delta requirements:
  - Activation condition: only run if the boot prompt's "Governance documents" section is present.
  - Ordering: a sub-step inside the existing Spec Audit Procedure, NOT a separate flow step.
  - Per-doc examples for DoD, ADR, security, test strategy, constitution. Examples are illustrative, not exhaustive rubrics.
  - Findings flow through `agent.feedback` as standard audit errors. NO governance-specific tag prefix.
  - Missing-doc handling: missing files become findings in the `agent.feedback` errors list.
  - The skill SHALL NOT use "gating" / "blocking on governance failures" language. SHALL NOT reference `[governance.gates]` or `[governance-gate:<doc>]`.
- [x] 4.2 Mirror the section into `docs/src/user-guide/supervisor.md` (or whichever user-guide chapter covers the supervisor).

## 5. Skill-content tests

- [x] 5.1 Supervisor skill contains the substring `Governance verification`.
- [x] 5.2 Skill describes governance reading as a sub-step of Spec Audit Procedure (no "step 7.5" framing).
- [x] 5.3 Skill provides illustrative examples for all five doc types (DoD, ADR, security, test strategy, constitution).
- [x] 5.4 Skill states findings are reported as `agent.feedback` errors.
- [x] 5.5 Skill does NOT contain the substring `[governance-gate:`.
- [x] 5.6 Skill does NOT contain the substring `[governance.gates]`.
- [x] 5.7 Skill does NOT contain the substrings "gating" or "blocking on governance failures" (case-insensitive).
- [x] 5.8 Skill instructs that missing files become findings in the audit's `agent.feedback`.

## 6. Integration test

- [x] 6.1 Build a test fixture with `[governance.dod] = "docs/dod.md"` and a deliberately-incomplete DoD checklist (e.g. `- [ ] CHANGELOG.md updated`). Drive the supervisor through a session where an agent claims done; assert the supervisor's `agent.feedback` includes an error mentioning the unchecked DoD item, alongside any other spec-audit findings. The error SHALL NOT contain the `[governance-gate:dod]` tag (it doesn't exist).
- [x] 6.2 Same fixture without the `[governance.dod]` path: assert the audit produces no DoD-related feedback.
- [x] 6.3 Fixture with `[governance.adr] = "docs/adr"` and a diff introducing a new dep with no matching ADR: assert the supervisor's `agent.feedback` MAY include an ADR-drift observation; the supervisor MAY also publish an `agent.learning` with `adr-candidate` at end-of-session per `learnings-mode` (cross-emission still applies).
- [x] 6.4 Fixture with `[governance.dod]` set, file missing: assert the supervisor's audit `agent.feedback` includes an error noting the missing DoD file.

## 7. Documentation

- [x] 7.1 Update `docs/src/user-guide/supervisor.md` (or wherever supervisor docs live) to document the new sub-step inside spec audit. Cross-reference `governance-config` (storage). Cross-reference `learnings-mode` (cross-emission for systemic findings).
- [x] 7.2 Add a "Governance" subsection in the user guide showing user-guide *examples* (illustrative, not vendored templates) of what an ADR-0001, DoD checklist, security checklist, and test strategy doc *might* look like. Frame these as "if your project doesn't already have a convention, here's a starting point." Repository structure of these examples is up to the doc author; suggested location: `docs/src/user-guide/governance-examples/`.
- [x] 7.3 `mdbook build docs/` succeeds.

## 8. Release notes

- [x] 8.1 v0.5.0 release notes: announce that the supervisor now consults configured `[governance]` docs during spec audit when paths are set. Note that findings flow through standard `agent.feedback` (no new wire format). Reference `governance-config` (storage) and the user-guide examples.

## 9. Quality gates

- [x] 9.1 `just check` — fmt, clippy, all tests green.
- [x] 9.2 `just deny` — supply chain clean.
- [x] 9.3 No new `unwrap()` / `expect()` in non-test code added by this change.
- [x] 9.4 `mdbook build docs/` succeeds.
- [x] 9.5 `openspec validate governance-context` passes.
