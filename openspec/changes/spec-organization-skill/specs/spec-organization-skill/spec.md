## ADDED Requirements

### Requirement: Spec-organization skill provided

The repository SHALL provide a `spec-organization` agent skill at
`.agents/skills/spec-organization/SKILL.md` (agentskills.io format; `name` matching the folder)
codifying the Gate-3 spec-audit doctrine: capability granularity (one change ≠ one permanent
capability; when to merge vs keep separate), contract-preserving verbatim reorganization (the
requirement-count invariant and the requirement→test map as the safety net), RFC-2119 and
GIVEN/WHEN/THEN authoring rules (including the SHALL-on-first-line validator trap and the `## Purpose`
placeholder guard), specs-as-documentation (the docs `{{#include}}` coupling and a domain-grouped
Purpose-led index), the errors a pre-freeze audit must catch, and the frozen-contract and
`--skip-specs` archive discipline.

#### Scenario: Skill present and conformant

- **GIVEN** the repository checkout
- **WHEN** `.agents/skills/spec-organization/SKILL.md` is loaded via git-paw's own skill resolver
- **THEN** it SHALL parse as a valid agentskills.io skill (`name = "spec-organization"`, non-empty description)
- **AND** it SHALL cover capability granularity, contract-preserving verbatim reorganization, and the requirement→test traceability doctrine

#### Scenario: Skill is repo-local dev tooling, not an export

- **WHEN** the exported asset set (`assets/agent-skills/`, the `git paw init` default config) is inspected
- **THEN** the `spec-organization` skill SHALL NOT appear there — it is repo-local dev tooling only
