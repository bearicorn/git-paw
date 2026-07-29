# Tasks — spec-consolidation

Full before→after map + per-requirement disposition: `.git-paw/v0.13.0-spec-consolidation-audit.md`.
Run the `spec-traceability-audit` requirement→test map at every wave gate — no scenario may lose
its covering test.

## 1. Cleanup + error fixes (no merge) — Wave 1 DONE (applied directly to openspec/specs/)
- [x] Fix phantom `git paw resume` → `git paw start` in `add-branch` "Paused-session interplay" (applied to main spec + delta is audit trail; archive with `--skip-specs`)
- [x] Remove the obsolete negative `learnings-mode` "No agent.learning broker variant in v0.5.0" (applied to main spec; the forward-design note is re-added as a positive statement when `learnings-mode` merges into `learnings` in §5)
- [x] Delete `test-coverage-v0-5-0` capability (meta/bookkeeping, no product SHALL); dropped its callout + index link from the docs Specifications page (src/tests comment references are historical annotations, left in place)
- [ ] Editorial pass: reframe dated "preserve v0.5.0 behaviour" wording → **DEFERRED (cosmetic, behaviour unchanged)**; fold into the relevant §4/§5 merge passes where those specs are already open, rather than a separate churny sweep

## 2. Trivial 2→1 merges (zero overlap) — Wave 2 DONE (14 merges; 730 reqs verbatim-preserved; mdbook green; 96→82 dirs)
- [x] skill-standardization + skill-validation → `standardized-skills`
- [x] safe-command-classification + permission-detection → `command-classification`
- [x] supervisor-agent-inventory + supervisor-tell → `supervisor-directives`
- [x] session-logging + replay-command → `session-logging` (anomaly: requirement name `List available log sessions` exists in BOTH with different bodies — `list_log_sessions()` lib fn vs `git paw replay --list` stdout; both preserved verbatim as two sections, no content dropped)
- [x] cli-detection + cli-selection → `cli-resolution` (docs `{{#include}}` line 38 re-pointed)
- [x] cli-submit-profile + robust-cli-launch → `cli-launch`
- [x] conflict-detection ← conflict-detector-fn-granularity
- [x] git-hook-injection ← worktree-branch-guard
- [x] agents-md-injection ← worktree-agents-md
- [x] add-branch + remove-branch → `add-remove-branch` (carries the resume fix from §1)
- [x] dashboard ← dashboard-broker-log
- [x] mcp-server + mcp-read-tools → `mcp`
- [x] agent-friendly-docs-site + docs-fetch-skill → `agent-docs`
- [x] test-isolation + cold-start-ci-parity → `test-and-ci-hygiene`

## 3. Micro-spec absorptions — Wave 3 DONE (7 absorptions; 730 reqs preserved; 82→68 dirs; mdbook green; corrections woven in)
- [x] cli-parsing ← start-force-flag + cli-specs-supervisor-filter
- [x] supervisor-skill-discipline ← supervisor-stream-timeout-recovery + per-commit-verification + no-fail-fast-verification (absorbed only these 3; `coordination-context-budget`/`advanced-main-event` left for §5/§6 per the parenthetical ambiguity)
- [x] supervisor-launch ← supervisor-cli + supervisor-first-agent-cwd + supervisor-pane-affordances — **wove in C3** (Initial-prompt-injection → sidecar `.git-paw/AGENTS.local.md` model)
- [x] session-state ← session-json-location + session-receipt-hygiene + session-recovery-integrity
- [x] approval-configuration → folded into supervisor-config — **wove in C6** (safe preset → composed defaults) + **gemini `agy` flag-table** correction on "Permission flag mapping"
- [x] agent-skills ← coordination-context-budget (kept `lang-agnostic-skills` standalone)
- [x] agent-broker-helper ← stuck-prompt-detection

**Normalization follow-up (accumulating; do in §6 or a normalization pass):** duplicate requirement NAMES within a merged spec, kept verbatim to preserve the 730 count — `supervisor-skill-discipline` now has 3 × `### Requirement: Stack-agnostic phrasing`; `session-logging` has 2 × `List available log sessions` (from §2). Disambiguate the headings (scope suffix) without changing scenario content.

## 4. Facet merges — Wave 4 DONE (6 merges; 730 reqs preserved; 68→53 dirs; mdbook green)
- [x] broker-server + broker-endpoints + broker-lifecycle → `broker-runtime` — **wove in C5** (GET /messages hold-cursor-at-`since`)
- [x] broker-roster-hygiene + filesystem-watcher + status-republish-on-write + terminal-status-protection → `broker-watcher-and-state`
- [x] spec-scanning + openspec/markdown/spec-kit/superpowers-integration → `spec-backends` (dup requirement names kept verbatim: "Extract paw_cli from frontmatter" ×2, "Boot-prompt assembly" ×2 — normalization follow-up)
- [x] curl-allowlist + custom-cli-curl-seeding + dev-command-allowlist → `command-allowlist-seeding` (kept verbatim; idempotent/non-fatal harmonization deferred to the normalization pass, not weakened)
- [x] boot-block-format + template-substitution + shared-helper → `boot-block`
- [x] supervisor-injection + manual-injection + from-specs-launch → `boot-block-injection` — reconciled the `--from-specs` alias (canonical `--from-all-specs`; deprecated, removal v1.0.0) as a Purpose note; moved requirements keep `--from-specs` verbatim

## 5. Careful reconciliation merges — Wave 5 DONE (97→46 caps reached; total 730→729 via C4)
- [x] automatic-approval + broker-mediated-approvals + auto-approve-file-edits → `auto-approval` (15 reqs, verbatim). **REAL contradiction surfaced by the merge (see §5b) — the two capabilities specced conflicting live-prompt capture windows; both kept verbatim + flagged, resolve against code next**
- [x] learnings-mode + qualitative-learnings + agent-learning-variant → `learnings` (28 reqs) — reframed "No confidence field in payload" → positive "Qualitative payload schema"; re-added the forward-design note as positive prose
- [x] broker-messages + message-delivery → `broker-protocol` (44 reqs) — **wove in C4** (roster upserts scoped to Status/Artifact/Blocked/Intent; the −1 that makes the total 729). Over-700-line fallback applied: `supervisor-introspection` + `advanced-main-event` folded into `broker-watcher-and-state` (11→25) instead of broker-protocol

## 5b. Post-merge reconciliation + normalization (do before finishing)
- [x] **auto-approval live-prompt window contradiction (freeze blocker) — RESOLVED:** the code (`auto_approve.rs`) uses BOTH `LIVE_PROMPT_TAIL = 4` (prompt must anchor in the last 4 non-blank lines) AND `LIVE_PROMPT_BLOCK = 15` (textual markers detected across a wider multi-option block). The two specs each described one half. Reconciled "Approval keystrokes require a re-confirmed live prompt" (+ its 3 scenarios) to the anchor(4)+block(15) model per the Live-prompt gate, so they no longer contradict; matches tested code. Count unchanged (prose only)
- [x] Disambiguated duplicate requirement NAMES via scope-suffixed headings (prose/scenarios byte-identical): `supervisor-skill-discipline` 3×, `broker-watcher-and-state` 2×, `spec-backends` 4× (paw_cli×2 + boot-prompt×2), `session-logging` 2×. `uniq -d` clean in all four; count still 729
- [x] Remapped 17 `[[...]]` cross-reference tokens to the renamed caps (mcp, learnings, broker-watcher-and-state, supervisor-directives, supervisor-skill-discipline, dashboard); zero stale left-column refs remain

## 6. Docs restructure (docs/src/) — Wave 6 DONE (domain-grouped index over the 46 caps; mdbook green; anti-drift guard added)
- [x] Rewrote `docs/src/specifications/README.md`: replaced the flat A–Z list with a **domain-grouped, Purpose-led index** over all 46 caps (9 domains). Kept the 8 foundational `{{#include}}` blocks (full inline specs) — a lighter touch than removing them, still fixes the staleness + adds domain structure
- [x] Each capability entry leads with a condensed Purpose blurb
- [x] `{{#include}}` paths valid (only cli-detection→cli-resolution needed re-pointing, done in §2); mdbook green
- [~] Cross-link each domain to its `user-guide/` chapter — DEFERRED (nice-to-have; the domain index + existing per-chapter links suffice for now)
- [x] Hand-authored + **anti-drift guard** `tests/specifications_page_lists_every_capability.rs` (asserts every `openspec/specs/` dir appears on the page)
- [x] `SUMMARY.md` unchanged — the Specifications page is a single entry (no per-cap anchors)

## 7. Namespaced taxonomy (post-consolidation refinement — user-directed 2026-07-29)
Reorganize into ~14 concern-namespaces (core/cli/git/tmux/session/boot/broker/supervisor/approval/spec/mcp/governance/skill/quality); `core-` gathers cross-cutting foundations (configuration, error-handling, lang-agnostic, ci-hygiene, memory-isolation, project-conventions); distribute the cross-cutting `user-documentation` into its domains. 3 verified passes; 729 reqs held; target ~40 caps.
- [x] Pass 1 — 6 within-domain collapses (boot-block-injection→boot-block, cli-launch→cli-resolution, command-allowlist-seeding→command-classification, session-summary→session-state, supervisor-directives→supervisor-skill-discipline, worktree-embedded-placement→add-remove-branch). 46→40 dirs; 729 held; mdbook + anti-drift green
- [x] Pass 2 — distributed `user-documentation`'s 30 reqs into 11 domains + new `core-project-conventions` (8 reqs); reframed 39 `v0.5`/`v0.5.0` mentions in moved content to present-tense (nothing retired; 729 held; 40 dirs). 7 pre-existing `v0.5` mentions remain in broker-protocol/configuration/supervisor-launch (not moved content) — sweep in Pass 3
- [x] Pass 3 — renamed 21 caps to `<namespace>-<sub>` (14 namespaces: core/cli/git/tmux/session/boot/broker/supervisor/approval/spec/mcp/governance/skill/quality; `mcp` is the mcp- anchor). Fixed the 3 renamed `{{#include}}`s; rewrote the docs Specifications page by namespace (dropped 7 stale phantom links); remapped 9 `[[...]]` cross-refs; swept the last 7 `v0.5` mentions present-tense; no hardcoded src/tests refs to fix. **729 reqs / 40 caps**; mdbook + anti-drift + `cargo test --no-run` all green. Namespaced taxonomy COMPLETE

## 7. Verification (five gates, at every wave)
- [ ] Gate 3 (spec audit): traceability map run before AND after each wave — every WHEN/THEN still maps to ≥1 test; `openspec validate --strict` passes for the touched specs
- [ ] Gate 4 (doc audit): `mdbook build docs/` succeeds; Specifications page renders every capability; no dead `{{#include}}`/links; `git paw resume` appears nowhere as a real command
- [ ] Gate 1/2: full suite green vs merge-base (no code changed, but prose-pin/`*_skill_content` tests that assert spec content may need updating in lockstep)
- [ ] Gate 5 (security): no exported-asset or agnosticism regression (docs-only + spec-doc reorg)
- [ ] `openspec validate spec-consolidation --strict` passes
- [ ] Confirm no capability directory is orphaned or duplicated after the reorg (target ~46)
