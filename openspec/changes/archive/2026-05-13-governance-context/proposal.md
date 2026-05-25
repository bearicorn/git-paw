## Why

`governance-config` shipped a path-pointer mechanism — `[governance]` paths to ADRs, DoD, test strategy, security checklist, and constitution. Nothing reads it. This change is the thin runtime consumer that injects those paths into the supervisor agent's boot prompt and updates the supervisor skill so the agent reads them and uses them as criteria during the existing spec-audit step.

That's it. No `[governance.gates]` table, no `[governance-gate:<doc>]` tag convention, no per-doc check rubric, no separate flow step. The supervisor agent is an LLM with file-read access; given the doc paths it can read each doc and apply judgment as part of normal spec audit. Findings flow through the existing `agent.feedback` path. Heavier enforcement is dogfood-driven follow-up.

This change deliberately stays small. Earlier MILESTONE drafts proposed gating semantics, per-doc rubrics, and a parallel flow step (governance verification as step 7.5 between spec audit and verify); all of that was dropped after a scope review (MILESTONE drift item #18). The user's stance: ADR conventions, DoD format, security checklists, and constitutions are owned by teams' existing processes — git-paw shouldn't dictate any of that.

## What Changes

- **Boot-prompt injection.** When the supervisor agent's boot prompt is constructed AND any `[governance]` path is set, the boot prompt SHALL include a "Governance documents" section listing the configured paths. One bullet per path: doc name + path. Paths whose value is `None` SHALL NOT appear. When ALL paths are `None`, the section is omitted entirely (no header, no placeholder).
- **Path-only injection.** Doc *content* SHALL NOT be embedded in the boot prompt. The supervisor reads the files via its existing tool access at audit time. This avoids prompt bloat for projects with large constitutions.
- **No gates summary in the boot prompt.** Earlier drafts had a parallel "Governance gates" line listing gated docs. That's gone — there's no `[governance.gates]` table to consume per `governance-config`'s reduction.
- **Supervisor skill update.** The embedded `supervisor.md` skill SHALL gain a "Governance verification" section instructing the supervisor agent to:
  1. If the boot prompt's "Governance documents" section is present, read each listed doc during the existing spec-audit step.
  2. Use each doc as criteria — for example, walk DoD items against the branch state, scan the diff for ADR drift, walk the security checklist against the diff, check code against constitutional principles.
  3. Surface findings as `agent.feedback` errors (one error per finding, the same way other audit findings are surfaced).
  4. Apply judgment — the skill provides examples of *what* to look for per doc type, but does NOT define structured rubrics. The supervisor agent reads the doc and decides what counts as a finding given the project's conventions.
- **Findings flow through `agent.feedback`.** No new tag prefix, no `[governance-gate:<doc>]`, no new wire format. Governance findings are spec-audit findings; the supervisor reports them under existing channels.
- **No separate flow step.** Governance verification is a sub-step of the existing spec-audit step (currently step 7 in the supervisor flow). The skill makes this explicit; the rest of the workflow ordering is unchanged.
- **Missing-doc handling.** If a configured path doesn't resolve to a readable file, the supervisor MAY note this as a finding under `agent.feedback` ("configured doc <path> not found in worktree") but does NOT block — same as any other audit finding.

Not in scope:
- Programmatic Rust-side per-doc checks. Per-doc reasoning is LLM-level.
- Embedding doc content in the boot prompt.
- A `[governance-gate:<doc>]` tag convention or any other governance-specific tag prefix on `agent.feedback`. Findings are plain audit feedback.
- A separate flow step. Governance fits inside spec audit.
- Project-wide ADR drift detection beyond the diff. v0.5.0 limits scope to "what's the supervisor seeing in this branch."
- Per-CLI prompt variations for governance content.
- Init-time scaffolding (out per `init-governance-scaffolding` being dropped during MILESTONE rewrite).

## Capabilities

### New Capabilities
*(none — this change extends two existing capabilities only)*

### Modified Capabilities
- `supervisor-injection`: boot prompt SHALL include a "Governance documents" section when `[governance]` paths are configured; section omitted when all paths are `None`.
- `agent-skills`: supervisor skill SHALL include a "Governance verification" section instructing the per-doc consultation flow during the existing spec-audit step. Findings flow through `agent.feedback`.

## Impact

**Code**:
- `src/supervisor/boot.rs` (or wherever the supervisor's boot prompt is constructed) — add a renderer that takes `&GovernanceConfig` and produces a "Governance documents" section. Skip the section entirely when no paths are configured.
- `assets/agent-skills/supervisor.md` — modest section addition for governance verification (mostly skill-level prose; no curl examples needed since findings reuse the existing `agent.feedback` path).
- `docs/src/user-guide/supervisor.md` — document the new sub-step inside spec audit and link to the user-guide examples of governance docs.

**Tests**:
- Boot prompt: `[governance]` empty → section omitted; one path set → section present with that path; all paths set → all five listed in canonical order.
- Skill content: skill mentions "Governance verification" section; lists all five doc names; states the supervisor consults configured docs during spec audit; states findings flow through `agent.feedback`; does NOT mention `[governance.gates]` or `[governance-gate:<doc>]` (those were dropped).
- Integration: a fixture with `[governance.dod]` set and a deliberately-incomplete DoD checklist → supervisor's `agent.feedback` includes an error mentioning the unchecked DoD item. Same fixture without the DoD path → no DoD-related audit feedback.
- Round-trip: governance config + supervisor flow + dashboard observation reflects governance findings as standard `agent.feedback` errors.

**Backward compatibility**: fully additive. Configurations without `[governance]` paths produce identical v0.4.0 behaviour — the boot prompt section is omitted, the supervisor skill's governance section runs but finds no docs to consult, and no governance-related `agent.feedback` is emitted.

**Mismatches resolved**:
- MILESTONE drift item #18 (governance scope was over-reaching) — this change is the slimmed runtime capability that replaces the original `governance-verification` change. Per-doc rubrics, gate flags, tag conventions, and the parallel flow step are all out.
