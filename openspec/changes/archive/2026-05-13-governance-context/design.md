## Context

The supervisor agent in v0.5.0 already runs a structured workflow (per `assets/agent-skills/supervisor.md`): baseline → watch → test → regression check → spec audit → verify-or-feedback. Conflict detection runs automatically in the broker process; learnings are captured at end-of-session. This change extends the supervisor's *spec-audit* step (step 7) to also consult any user-configured governance documents — without introducing a separate flow step.

The Rust contribution is intentionally small: a boot-prompt renderer that lists the configured `[governance]` paths so the supervisor agent knows what to read. The supervisor skill picks it up from there: read the docs, apply judgment, surface findings through the existing `agent.feedback` path.

The model is the *thinnest* version of the original `governance-verification` design. Earlier drafts had: a `[governance.gates]` table, per-doc rubrics in spec form, a `[governance-gate:<doc>]` tag convention, and governance-as-its-own-flow-step. All of that was dropped after the user pointed out that ADR conventions, DoD format, security checklists, and constitutions are owned by teams' existing processes — git-paw shouldn't dictate any of that.

## Goals / Non-Goals

**Goals:**
- Inject the configured governance paths into the supervisor agent's boot prompt, with the section omitted entirely when no paths are configured.
- Update the supervisor skill with a brief "Governance verification" section instructing the agent to read configured docs during the existing spec-audit step.
- Reuse the existing `agent.feedback` wire format with no new tag prefix. Governance findings are spec-audit findings.
- Keep the supervisor flow ordering unchanged. Governance is a sub-step inside step 7, not a new step 7.5.
- Boot prompt carries paths only, not doc content.
- Trust the supervisor LLM. Don't define rubrics; provide examples in the skill but let the agent decide what counts as a finding.

**Non-Goals:**
- Programmatic Rust-side per-doc checks. Per-doc reasoning is LLM-level.
- Embedding doc content in the boot prompt.
- A `[governance.gates]` table or any per-doc gate flags (gone per `governance-config`).
- A `[governance-gate:<doc>]` tag prefix on `agent.feedback`. Findings are plain audit feedback.
- A separate flow step. Governance is a sub-step of spec audit.
- Per-CLI prompt variations for governance content.
- Init-time scaffolding (out per the broader v0.5.0 scope reduction).
- Project-wide ADR drift detection beyond the branch's diff.

## Decisions

### D1. Boot-prompt section: paths only, single block

The supervisor's boot prompt gains a section after the existing supervisor skill content:

```
## Governance documents

The following project documents are configured for the supervisor to consult
during spec audit:

- ADRs: docs/adr/
- Test strategy: docs/test-strategy.md
- DoD: docs/definition-of-done.md
```

Bullets only appear for configured paths. When `[governance]` is empty (no paths), the entire section is omitted.

Rationale for path-only injection:
- **Prompt bloat:** a 4 KB constitution embedded in every supervisor session would 5x the boot-prompt size. Reading at audit time is cheap (the supervisor has worktree filesystem access).
- **Stale snapshots:** doc content can change mid-session; reading at audit time captures the current state.
- **Per-CLI tooling:** agents that have file-read tools (most modern CLIs) can `cat` paths trivially.

Earlier drafts had a parallel "Governance gates" line listing gated docs — gone with the gates table.

### D2. No tag prefix on findings

Earlier drafts proposed `[governance-gate:<doc>]` as the first error string of `agent.feedback` for governance findings, mirroring `[conflict-detector]` from `conflict-detection`. That's gone:

- Without gates, there's nothing to "gate." The findings are just audit feedback.
- A tag prefix implies a category that consumers (dashboards, MCP) might branch on. Without a behavioural distinction (gating vs. not), the category buys nothing.
- Plain audit findings flow through the existing `agent.feedback` `errors` array unchanged.

If users later need to distinguish governance feedback from spec feedback in dashboards, a tag prefix can be added without breaking the wire format.

### D3. Supervisor flow ordering: governance is a sub-step of spec audit, not a separate step

Earlier drafts inserted "Governance verification" as step 7.5 between spec audit (step 7) and verify-or-feedback (step 8). That created an artificial split — spec audit was checking the spec, and step 7.5 was checking project-wide rules. Now they merge: spec audit considers both the change's spec AND any configured governance docs.

The skill update spells this out:

> ## Spec Audit Procedure
>
> 1. Locate specs at `openspec/changes/<change-name>/specs/`.
> 2. For each scenario, search the codebase for matching tests …
> 3. For each requirement, verify struct fields, signatures, types …
> 4. **If the boot prompt's "Governance documents" section is present**, read each listed doc and check the diff/branch against it. Examples:
>    - DoD: walk each `- [ ]` item against branch state.
>    - ADRs: scan the diff for new architectural patterns and verify a matching ADR exists.
>    - Security checklist: walk each item against the diff.
>    - Test strategy: check test composition matches.
>    - Constitution: check code against principles.
> 5. Compile gaps. If any → `agent.feedback`. If none → `agent.verified`.

This keeps the workflow simple: still one spec-audit step, with a governance sub-step inside.

### D4. Per-doc skill examples, not rubrics

The skill provides *examples* of what to look for per doc type. It does NOT define structured rubrics ("for ADRs, scan for these specific keywords; for security, run these specific checks"). Those would force git-paw to encode opinions about each doc type's structure, which is exactly what the user pushed back on.

Examples in the skill are illustrative:
- "DoD example: an item like `- [ ] CHANGELOG updated` is a finding when the diff doesn't include changes to `CHANGELOG.md`."
- "ADR example: a new dependency like `tokio` warrants a matching ADR if the project has an established ADR convention."
- "Security example: a checklist item like 'validate user input' is a finding when a new HTTP handler has no input validation."

These are starting points for the supervisor agent's reasoning, not exhaustive checks. The agent uses judgment.

### D5. Missing-doc handling

If a configured path doesn't exist when the supervisor tries to read it, the supervisor MAY add an `agent.feedback` error noting the missing file ("configured DoD doc `docs/dod.md` not found in worktree"). It does NOT block — same as any audit finding.

This is much simpler than the earlier drafts which had distinct gated/non-gated handling. Without gates, missing-doc is just one more audit observation.

### D6. Boot-prompt rendering location

The supervisor's boot prompt is constructed in the supervisor-injection capability (`src/supervisor/boot.rs` or equivalent). This change adds a `governance_section(&GovernanceConfig) -> String` helper that returns the formatted section (or empty string when no paths configured). The boot-prompt builder calls this helper after the existing skill-rendered content.

The helper is pure — given a `GovernanceConfig`, returns a string. Easy to unit-test independently.

### D7. Permission-prompt safety for governance file reads

Reading governance docs goes through the agent's normal file-read tool. If the project's auto-approve policy doesn't include the governance paths, the agent may hit a permission prompt mid-audit. v0.5.0 doesn't auto-add paths to the allow-list — manual config is acceptable for an opt-in feature. If dogfood shows this is friction, a follow-up release can add `[supervisor.auto_approve] include_governance_paths = true`.

## Risks / Trade-offs

- **[Risk] Without gates, supervisors might emit governance findings inconsistently across sessions.** A DoD item the supervisor flags as a finding in one session might be ignored in another. → **Mitigation:** the supervisor skill provides examples to reduce inconsistency. Long-term, dogfood reveals whether structured gates are needed; v0.5.0 ships the simple version first.
- **[Risk] Findings get lost in the noise of regular audit feedback.** A supervisor publishes a single `agent.feedback` with both spec-audit gaps and governance findings; the agent might prioritise the spec gaps. → **Mitigation:** acceptable for v0.5.0. If dogfood shows this is a problem, a `[governance:<doc>]` prefix can be re-introduced as a non-gating categorisation hint.
- **[Risk] LLM produces false positives.** "Missing ADR for new tokio dep" when in fact the team's ADR convention covers it differently. → **Mitigation:** the agent receiving feedback can push back; the supervisor's existing escalation path (human via `agent.question`) handles disputes.
- **[Trade-off] Skill-level vs. code-level checks.** Putting examples in the skill means iteration requires skill updates. The user-override mechanism lets users iterate on per-doc guidance without rebuilding git-paw. Net positive.
- **[Trade-off] No governance-specific tag prefix.** Distinguishing governance feedback from spec feedback in dashboards becomes a string-matching exercise. → Acceptable: dashboards aren't built for that distinction in v0.5.0.

## Migration Plan

Additive. Steps:

1. Land `governance-config` first (provides the storage slot).
2. Land this change. v0.4 / early-v0.5 sessions see the boot prompt extended only when `[governance]` paths are configured; users without `[governance]` see no change.
3. Users opt in by configuring `[governance]` paths. The supervisor automatically picks up the boot-prompt injection and the skill's new section.
4. Rollback: revert. Boot-prompt section disappears; skill section disappears; the agent stops mentioning governance docs in audit. No data migration.

Release-notes call-outs:
- Supervisor's spec audit now consults configured governance docs (when `[governance]` paths are set).
- Findings appear as standard `agent.feedback` errors — no new wire format.
- Boot prompt grows when `[governance]` is configured (paths only, not doc content).

## Open Questions

- **Should the boot prompt list non-existent paths?** Decision: yes — the skill instructs the agent to detect missing files at read time and surface them as findings. Hiding non-existent paths from the boot prompt would mask user typos.
- **Should governance verification run on the supervisor's own merge commit?** Decision: no for v0.5.0. Governance is per-agent-branch. Re-running governance on the merge result is a candidate for a v1.0.0 "post-merge audit" feature.
- **Should the skill include curl examples for publishing governance findings?** Decision: no. Findings flow through the existing `agent.feedback` path; the existing `agent.feedback` curl example in the supervisor skill (corrected by `v040-hardening`) is the example. Governance findings are not a new message type.
