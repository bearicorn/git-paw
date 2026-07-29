---
name: spec-organization
description: Keep the OpenSpec capability set lean, namespaced by concern, traceable, and readable as documentation — the Gate-3 spec audit. Use when authoring a new capability, archiving a change, consolidating or auditing specs, or preparing the contract for a release freeze. Covers capability granularity and the `<namespace>-<sub>` naming convention, the per-feature-distributes / cross-cutting-centralizes-in-core discipline, contract-preserving verbatim reorganization, RFC-2119 and GIVEN/WHEN/THEN authoring, the requirement-to-test traceability doctrine, the docs include-coupling, and the errors a pre-freeze audit must catch.
license: MIT
compatibility: git-paw
---

# Spec Organization

Use when authoring, archiving, consolidating, or auditing specs — or hardening the contract before a
release freeze. The specs under `openspec/specs/` ARE the contract v1.0.0 freezes AND they render as
the Specifications docs, so they must stay lean, coherent, traceable, and error-free. This skill is
**Gate-3 (spec audit)** of the five-gate framework.

## 1. Capability granularity and namespacing — group by concern

- **One change ≠ one permanent capability.** Don't leave single-requirement residue behind each
  archived change. The capability set should read like a domain map, not a changelog.
- **Merge when:** a capability is single-requirement residue, or a tight cluster is split finer than
  any consumer reads it.
- **Keep separate when:** the capability is a load-bearing design principle with its own CI audit
  (e.g. `core-lang-agnostic`), or a genuinely distinct domain a reader would look up on its own.
- **Name every capability `<namespace>-<sub>`** by concern (git-paw's namespaces: `core- cli- git-
  tmux- session- boot- broker- supervisor- approval- spec- mcp- skill-`). The set then reads as
  "N namespaces × a few caps," not a flat list. Two rules:
  - **No cap name may equal its namespace** — a bare `mcp` in the `mcp-` namespace becomes
    `mcp-server`; every cap carries a distinguishing sub-word.
  - **Put a capability in the namespace of the subsystem it belongs to**, not the one that happens to
    launch it (e.g. `broker-conflict-detection` + `broker-dashboard` run on / view broker state, even
    though the supervisor drives them).

### The distribution vs centralization discipline (what belongs in `core-`)

The hardest calls are *cross-cutting* concerns. The rule:

- **Per-feature CONTENT distributes** into the feature's own domain. Documentation is the archetype —
  there is no `docs-` domain; each capability's doc requirements live *with that capability*. If a
  "capability" is really a grab-bag of per-feature assertions (git-paw's old `user-documentation`),
  dissolve it and distribute the requirements to the domains they describe.
- **Cross-cutting MECHANISMS, INVARIANTS, and POLICY centralize** in `core-` — the shared plumbing
  every domain depends on: configuration, error types, the export-agnosticism principle, test/CI
  hygiene, memory isolation, repo conventions, governance/role-gating. Stated once, not scattered.
- **A runtime FEATURE is never `core-`.** If git-paw *does* it — produces output, exposes a
  tool/command a user invokes — it's a feature domain even when it feels foundational (e.g. `learnings`
  is observability the supervisor runs → `supervisor-learnings`, not `core-`).
- **Litmus:** *would a reader look this up as a feature, or is it plumbing/convention/policy that's
  true everywhere?* Feature → its own domain. Plumbing/convention/policy → `core-`.

## 2. Contract-preserving reorganization

- Move every `### Requirement:` section — its prose, ALL `#### Scenario:` blocks, and any `Test:`
  pointers — **verbatim**. Never drop, weaken, or reword a requirement or scenario, except a
  reconciliation you have explicitly flagged (see §5).
- The only new prose you author in a merge is the composed `## Purpose` paragraph.
- **The count invariant:** sum the `### Requirement:` count across all specs before the reorg; after
  it, the global count is unchanged (merges move requirements, they don't remove them). A drop or a
  duplicate shows up as a count mismatch — the cheapest possible safety net.
- Run the requirement→test map **before AND after** each merge wave: no scenario may lose its
  covering test.

## 3. Authoring rules

- RFC 2119 keywords: **SHALL/MUST** (mandatory), **SHOULD** (recommended), **MAY** (optional).
- Every requirement carries ≥1 GIVEN/WHEN/THEN scenario; every scenario maps to ≥1 test — add a
  `Test:` pointer to the covering test.
- **Validator trap:** the FIRST non-blank line after `### Requirement:` must contain SHALL/MUST.
  Reword so the keyword lands on line 1, or `openspec validate --strict` rejects it.
- Every spec has a real `## Purpose` — never the `Update Purpose after archive` placeholder (guarded
  by `tests/spec_purpose_backfilled.rs`).
- Drive authoring through the `opsx:*` skills, not hand-rolled file writes, except when amending an
  already-validated change. `openspec validate <change> --strict` must pass.

## 4. Specs as documentation

- The docs Specifications page `{{#include}}`s spec files **by path** — any rename or merge breaks
  `mdbook build`. Fix the includes in the same wave as the merge; keep the build green at every commit.
- Prefer a **namespace-grouped, Purpose-led index** — one section per namespace, aligned 1:1 with the
  capabilities — over a flat A–Z link list or a version-era "foundational N" split. Prefer a generated
  index over a hand-maintained one (hand-maintained drifts; if you hand-author it, add an anti-drift
  convention test asserting every `openspec/specs/` dir appears on the page, per
  `tests/specifications_page_lists_every_capability.rs`).
- Keep specs (the contract) distinct from `user-guide/` (how-to); cross-link the two.

## 5. Errors a pre-freeze audit must catch (Gate-3)

- **Spec lags tested code (drift):** reconcile by amending the spec to the shipped, tested behavior —
  the code is the tested truth. Amend the spec first; never leave a SHALL that the code contradicts.
- **Phantom commands:** a subcommand/flag the spec names that doesn't exist (e.g. a `resume` command).
- **Self-contradictory negatives:** a "there is no X" requirement after X shipped.
- **Dated / version-stamped framing** baked into a permanent requirement ("in v0.5.0 …").
- **Meta/bookkeeping specs** with no product SHALL — delete them (traceability owns scenario→test).
- **Enumeration drift:** a spec list (a CLI roster, an accepted-value set) that no longer matches the
  code. Lock these with a standing sync guard (§7).

## 6. Frozen-contract & archive discipline

- Before a freeze, leanness and coherence are the deliverable — that is the whole point of the audit.
- When you reorganize `openspec/specs/` **directly** (a structural reorg, not a normal delta), any
  sibling change's deltas become the audit trail for content already reflected in the canonical specs;
  archive those with `openspec archive <change> --skip-specs`.
- **Sequence matters:** fold delta-based corrections into the canonical specs BEFORE a wholesale
  reorg, so the corrections carry through the merges and no delta target vanishes under a rename.

## 7. Standing guards

Model on `tests/spec_purpose_backfilled.rs` (filesystem scan over `openspec/specs/`) and
`tests/source_audit.rs` (`include_str!` + structural assertions). Cheap, CI-runnable guards that lock
a closed gap so it can't silently reopen: purpose-placeholder absence, roster↔spec sync, accepted-value
list completeness, no-phantom-command prose, and requirement-count preservation across a reorg.

## Supervisor consumption

At the review gate, this skill IS the Gate-3 audit: every SHALL/MUST maps to code, every WHEN/THEN to
a test, no requirement contradicts shipped behavior, and the set is lean and reads as documentation.
Consult it at author time and at the gate (see the `standards-skill-integration` change for how the
worker and supervisor consult the project's standards skills). Related skills: `test-strategy`
(scenario→test), `doc-completeness` (the Specifications docs layer), `definition-of-done` (spec is one
done-dimension), `code-standards`.
