# Tasks — spec-consolidation

Full before→after map + per-requirement disposition: `.git-paw/v0.13.0-spec-consolidation-audit.md`.
Run the `spec-traceability-audit` requirement→test map at every wave gate — no scenario may lose
its covering test.

## 1. Cleanup + error fixes (no merge)
- [ ] Fix phantom `git paw resume` → `git paw start` in `add-branch` "Paused-session interplay" (delta authored)
- [ ] Remove the obsolete negative `learnings-mode` "No agent.learning broker variant in v0.5.0" (delta authored)
- [ ] Delete `test-coverage-v0-5-0` capability (meta/bookkeeping, no product SHALL); drop its callout + link from the docs Specifications page
- [ ] Editorial pass: reframe dated "preserve v0.5.0 behaviour" wording where it appears (behaviour unchanged)

## 2. Trivial 2→1 merges (zero overlap)
- [ ] skill-standardization + skill-validation → `standardized-skills`
- [ ] safe-command-classification + permission-detection → `command-classification`
- [ ] supervisor-agent-inventory + supervisor-tell → `supervisor-directives`
- [ ] session-logging + replay-command → `session-logging`
- [ ] cli-detection + cli-selection → `cli-resolution`
- [ ] cli-submit-profile + robust-cli-launch → `cli-launch`
- [ ] conflict-detection ← conflict-detector-fn-granularity
- [ ] git-hook-injection ← worktree-branch-guard
- [ ] agents-md-injection ← worktree-agents-md
- [ ] add-branch + remove-branch → `add-remove-branch` (carries the resume fix from §1)
- [ ] dashboard ← dashboard-broker-log
- [ ] mcp-server + mcp-read-tools → `mcp`
- [ ] agent-friendly-docs-site + docs-fetch-skill → `agent-docs`
- [ ] test-isolation + cold-start-ci-parity → `test-and-ci-hygiene`

## 3. Micro-spec absorptions
- [ ] cli-parsing ← start-force-flag + cli-specs-supervisor-filter
- [ ] supervisor-skill-discipline ← supervisor-stream-timeout-recovery + per-commit-verification + no-fail-fast-verification (+ context-budget/advanced-main skill text)
- [ ] supervisor-launch ← supervisor-cli + supervisor-first-agent-cwd + supervisor-pane-affordances
- [ ] session-state ← session-json-location + session-receipt-hygiene + session-recovery-integrity
- [ ] approval-configuration → fold into supervisor-config
- [ ] agent-skills ← coordination-context-budget (keep `lang-agnostic-skills` standalone — load-bearing design principle + CI audit)
- [ ] agent-broker-helper ← stuck-prompt-detection (keep the canonical `approve <pane>` gate in auto-approval; cross-ref)

## 4. Facet merges
- [ ] broker-server + broker-endpoints + broker-lifecycle → `broker-runtime`
- [ ] broker-roster-hygiene + filesystem-watcher + status-republish-on-write + terminal-status-protection → `broker-watcher-and-state`
- [ ] spec-scanning + openspec/markdown/spec-kit/superpowers-integration → `spec-backends`
- [ ] curl-allowlist + custom-cli-curl-seeding + dev-command-allowlist → `command-allowlist-seeding` (harmonise the idempotent/non-fatal wording, don't weaken)
- [ ] boot-block-format + template-substitution + shared-helper → `boot-block`
- [ ] supervisor-injection + manual-injection + from-specs-launch → `boot-block-injection`

## 5. Careful reconciliation merges (do last, review closely)
- [ ] automatic-approval + broker-mediated-approvals + auto-approve-file-edits → `auto-approval` — reconcile the dual send-gate into one non-contradictory set; keep every scenario
- [ ] learnings-mode + qualitative-learnings + agent-learning-variant → `learnings` — apply the §1 negative removal first; reframe the qualitative "no confidence field" as a positive schema statement
- [ ] broker-messages + message-delivery (+ introspection + advanced-main) → `broker-protocol` — split if it exceeds ~700 lines (fall back to 2→1 + fold introspection/advanced-main into broker-watcher-and-state)

## 6. Docs restructure (docs/src/) — first-class deliverable
- [ ] Rewrite `docs/src/specifications/README.md`: domain-grouped sections aligned 1:1 with the ~46 consolidated capabilities (drop the flat A–Z list + the "foundational 8" split)
- [ ] Lead each capability entry with its merged `Purpose` blurb (author merged Purposes as stand-alone doc paragraphs)
- [ ] Fix/replace all `{{#include ../../../openspec/specs/<cap>/spec.md}}` paths to the new capability dirs; keep `mdbook build docs/` green at every merge wave
- [ ] Cross-link each spec domain section to its matching `user-guide/` chapter (and back); mirror the SUMMARY.md domain order
- [ ] Prefer a generated domain-grouped index (pairs with `agent-docs` — feeds llms.txt/sitemap). If hand-authored, add a convention test asserting every `openspec/specs/` dir appears on the page (anti-drift guard)
- [ ] Update `docs/src/SUMMARY.md` if spec section anchors change

## 7. Verification (five gates, at every wave)
- [ ] Gate 3 (spec audit): traceability map run before AND after each wave — every WHEN/THEN still maps to ≥1 test; `openspec validate --strict` passes for the touched specs
- [ ] Gate 4 (doc audit): `mdbook build docs/` succeeds; Specifications page renders every capability; no dead `{{#include}}`/links; `git paw resume` appears nowhere as a real command
- [ ] Gate 1/2: full suite green vs merge-base (no code changed, but prose-pin/`*_skill_content` tests that assert spec content may need updating in lockstep)
- [ ] Gate 5 (security): no exported-asset or agnosticism regression (docs-only + spec-doc reorg)
- [ ] `openspec validate spec-consolidation --strict` passes
- [ ] Confirm no capability directory is orphaned or duplicated after the reorg (target ~46)
