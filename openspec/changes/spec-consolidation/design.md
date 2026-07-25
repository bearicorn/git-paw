# Design — spec-consolidation

## Context

97 capabilities, ~a third single-requirement residue of archived changes; several clusters
split finer than anyone reads them. The set is truthful (0/97 `Purpose: TBD`) but fragmented,
with a few frozen-contract errors. And the docs Specifications page is wired to the spec
directory layout, so the merges force a docs rewrite. Consolidation and docs-restructure are
therefore one change. Full inventory + 31-row before→after merge table:
`.git-paw/v0.13.0-spec-consolidation-audit.md`.

## Decisions

### D1 — Contract-preserving by construction; the traceability map is the gate

Every merge moves each SHALL/MUST requirement and WHEN/THEN scenario **verbatim** into a
section of the target capability. The only content changes are the two reconciliations (D7).
Before and after every merge wave, run the `spec-traceability-audit` requirement→test map:
if a scenario loses its covering test, a move dropped coverage — restore it. "Reduce
redundancy, never reduce coverage" (mirrors test-consolidation discipline).

### D2 — Domain is the single organizing axis — for specs AND docs

The audit's ~46 targets are grouped into ~16 domains (Broker, Approval/allowlist,
Skills & boot-block, Spec backends, CLI, Session, Supervisor, Conflict, Learnings,
Worktree/Git, Dashboard, MCP, Docs, Test/CI, Config, Infra). **This domain map is also the
docs information architecture.** We restructure the spec directories and the docs
Specifications page against the *same* domain grouping, so they improve in lockstep and a
reader moving between them sees one structure, not two.

### D3 — Restructure the docs Specifications page (the forced-and-opportune half)

`docs/src/specifications/README.md` today:
- `{{#include ../../../openspec/specs/<cap>/spec.md}}` embeds **8 "foundational" specs** in
  full (an arbitrary v0.1–v0.2 cut);
- the other **89 are a flat A–Z list of GitHub blob links**, hand-maintained (hence the old
  "8 of 93 surfaced" drift; it still lists `test-coverage-v0-5-0`, which we delete).

Every merge/rename breaks the `{{#include}}` paths and the 89 blob links → `mdbook build`
fails. So the page is rebuilt, and rebuilt *well*:

1. **Domain-grouped, not A–Z.** ~16 domain sections, each listing its 1–5 consolidated
   capabilities — navigable instead of a wall of 97 links.
2. **Drop the "foundational 8 embedded / rest linked" split.** Choose one rendering and
   apply it consistently (embed by domain, or link all with a short Purpose blurb per
   capability). With ~46 caps, consistent embedding-by-domain is feasible and readable.
3. **Each capability entry leads with its merged `Purpose`** (D4) as the human-readable
   blurb, so the index reads as documentation, not a link dump.
4. **Cross-link to the matching `user-guide/` chapter** (D6).
5. **Remove the manual `test-coverage-v0-5-0` / "internal process specs" callouts** the
   delete makes stale.

Sketch:

```
# Specifications
## Broker
  - broker-runtime — <Purpose>            (user-guide: —)
  - broker-protocol — <Purpose>           (user-guide: Coordination)
  - broker-watcher-and-state — <Purpose>
## Supervisor
  - supervisor-launch — <Purpose>         (user-guide: Supervisor)
  - supervisor-config — <Purpose>
  - supervisor-directives — <Purpose>
  - supervisor-skill-discipline — <Purpose>
## CLI …  ## Session …  ## Spec backends … (one section per domain)
```

### D4 — Merged `Purpose` becomes the docs section intro

Today: 97 fragmentary micro-purposes. After merge: ~46 coherent Purpose statements. Write
each merged Purpose as a **doc-quality paragraph that stands alone** (e.g. one `broker-runtime`
Purpose narrates server + endpoints + lifecycle as a whole). The Specifications page then
surfaces the Purpose directly — the biggest "reads better" win, and free, since we author the
merged Purpose regardless.

### D5 — Prefer a generated index over a hand-maintained one

The A–Z list drifted because it was hand-maintained. Prefer generating the domain-grouped
index from the spec set (each capability contributes its name + Purpose + domain tag), so it
cannot drift again. This pairs with the `agent-friendly-docs-site` / `agent-docs` capability
(the generated, complete, domain-grouped index also feeds `llms.txt`/sitemap/per-page
metadata). If build-time generation is too much for the cycle, a hand-authored domain-grouped
page is the acceptable floor — but add a convention test asserting every `openspec/specs/`
directory appears on the page (kills the drift class).

### D6 — Specs (contract) and user-guide (how-to) stay distinct, aligned, cross-linked

They serve different readers: `user-guide/` is task-oriented prose ("how do I run supervisor
mode?"); specs are the normative contract ("what SHALL supervisor launch do?"). Do **not**
merge them. Mirror the top-level domain grouping between the two and cross-link each spec
section to its user-guide chapter (and vice-versa), so a reader hops from how-to to contract
in one click. The mdBook TOC already groups the user-guide by feature; the spec sections adopt
the same order.

### D7 — The two content reconciliations (the only non-verbatim changes)

- `add-branch` "Paused-session interplay": `git paw resume` → `git paw start` (no `resume`
  subcommand exists; `start` resumes a paused session).
- `learnings-mode` "No `agent.learning` broker variant in v0.5.0": removed (contradicted by
  `agent-learning-variant`); applied *before* the `learnings` merge so the merged doc carries
  no contradiction.

Both are captured as this change's formal spec deltas. `test-coverage-v0-5-0` is deleted
(no product SHALL).

## Execution order (lowest-risk first — from the audit)

1. **Cleanup, no merge:** delete `test-coverage-v0-5-0`; fix phantom `resume` in `add-branch`.
2. **Trivial 2→1 merges** (zero overlap): skill-standardization+validation, detection+selection,
   session-logging+replay, dashboard+broker-log, mcp-server+read-tools, add+remove-branch, …
3. **Micro-spec absorptions** into their parents (cli-parsing, supervisor-launch/skill-discipline,
   session-state, supervisor-config).
4. **Facet merges** (broker-runtime/protocol/watcher-state, spec-backends, command-allowlist-seeding).
5. **Careful reconciliation merges, last:** `auto-approval` (dual send-gate — reconcile the two
   near-duplicate statements), `learnings` (apply D7 removal), `broker-protocol` (size — split if
   it exceeds ~700 lines).
6. **Docs restructure** applied incrementally as capabilities land (keep `mdbook build` green at
   every step), finalised once the ~46 set is stable.

Each wave is a task group; run the traceability map at each gate.

## Risks

- **Docs build breakage** is the mechanical risk — mitigated by updating the Specifications
  page `{{#include}}`/links in the *same* wave as each merge and gating on `mdbook build`.
- **A silent coverage drop** on a verbatim move — mitigated by the before/after traceability map
  (D1). The two deliberate content changes (D7) are documented, not silent.
- **`broker-protocol` unwieldy size** — fall back to a 2→1 (`broker-messages`+`message-delivery`)
  and fold introspection/advanced-main into `broker-watcher-and-state`.
