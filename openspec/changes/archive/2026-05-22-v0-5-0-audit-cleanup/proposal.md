## Why

A 10-agent post-merge audit of the 15 v0.5.0-cycle archived changes (2026-05-21) surfaced 4 real docs↔code mismatches, 4 minor docs gaps, and 9 acceptance-criteria-without-tests. The most consequential is a partial regression in `docs-v0.5.0-refresh`: it claimed to refresh `AGENTS.md` but the dependency table and commit-conventions scope list are still in v0.4 state. Other audited changes named `docs/src/user-guide/supervisor.md` in their Impact section but landed the content in sibling chapters without cross-linking.

Treating these as a Batch-4 cleanup pass before the v0.5.0 release tag closes the spec/code/docs drift the audit surfaced. Bundling into one change avoids 8 micro-PRs for what is essentially one editorial cleanup pass plus targeted test additions.

### Audit findings to close

**4 real docs gaps:**
1. `AGENTS.md` dependency table (~line 95) still lists `dirs` as approved AND missing 4 v0.5.0 deps (`schemars` 0.8, `serde_yaml` 0.9, `chrono` 0.4, `regex` 1). Per `docs-v0.5.0-refresh` task D6 the `dirs` row should note "intentionally absent — non-FOSS license" and the table should add the four new deps.
2. `AGENTS.md` commit-conventions scopes line (~line 52) missing v0.5.0 scopes: `user-guide`, `worktree`, `governance`, `learnings`, `pause`. Also missing the compound-scope `(<a>,<b>,...)` form documentation.
3. `docs/src/user-guide/supervisor.md` missing the governance-context spec-audit sub-step + cross-link to `docs/src/user-guide/governance.md`. The `governance-context` proposal Impact named this surface.
4. `docs/src/user-guide/supervisor.md` missing the "Common dev-command allowlist" subsection that `common-dev-allowlist-preset` proposal Impact required, AND missing the gate-command-templating note that `supervisor-gate-templating-v0-5-x` Impact required.

**4 minor docs gaps (low priority but worth closing):**
5. `forward-coordination`: `docs/src/user-guide/coordination.md` doesn't mirror the "Before you start editing" / "While you're editing" phased structure from the skill.
6. `conflict-detection`: `docs/src/user-guide/supervisor.md` doesn't mention the broker-side detector or `[conflict-detector]` tag.
7. `learnings-mode`: `docs/src/user-guide/supervisor.md` has no cross-link to `docs/src/user-guide/learnings.md`.
8. `supervisor-as-pane`: `docs/src/user-guide/supervisor.md` missing "When the user types in your pane" + "Merge orchestration" sections that the proposal Impact called out.

**2 new bugs surfaced during cleanup work (Bug C + Bug D):**

- **Bug C**: `git paw purge` interactive prompt does not honour `y`+Enter on the confirmation when unmerged-commits warning has been printed to stderr — the user reports answering yes but the purge doesn't run. Suspect interleaved stderr/stdin in `purge_with_prompt`.
- **Bug D**: `git paw purge --force` *appears* to freeze on worktrees with uncommitted changes or larger worktree state — the underlying cause is `git worktree remove` taking a long time silently. Manually running `git worktree remove --force <path>` first unblocks the purge. Two needed fixes: (a) propagate `--force` to `git worktree remove --force` when the user invokes `git paw purge --force`, and (b) emit per-worktree progress output (e.g. `Removing worktree <path>... done (12.3s)`) so the user can tell the command is working rather than stuck.

- **Bug E**: Supervisor-pane boot-block injection into `AGENTS.md` is not cleaned up on `git paw stop` or `git paw purge`. The injection (between `<!-- git-paw:start -->` and `<!-- git-paw:end -->` markers) accumulates across sessions: the v0.5.0 dogfood left ~700 lines of supervisor-skill prose appended to `AGENTS.md` even after the session was stopped + purged. Discovered 2026-05-21 when staging an unrelated AGENTS.md edit surfaced the injection as part of the staged diff. The fix is for `cmd_stop` and `cmd_purge` to delete the marked block on cleanup, restoring AGENTS.md to its pre-session content.

- **Bug F**: `git paw init` is not idempotent on existing configs and clobbers user content. Observed 2026-05-21: when the user's `.git-paw/config.toml` already has a `[supervisor]` block (e.g. the dogfood config), running `git paw init` (or the v0.5.x `supervisor-gate-templating-v0-5-x` change's commented-out-block writer) appends a second `[supervisor]` section, producing a TOML `duplicate key` parse error on subsequent loads. The correct behaviour: `init` SHALL add any newly-introduced config keys/sections that the user is missing, without touching keys/sections the user already has. Practically: parse the user's existing TOML, compute the diff against the bundled default schema, and append ONLY the missing keys (or skip entirely with a "config already has [supervisor]; no changes" message). Never blindly append a block that may already exist.

**9 AC test gaps (low-impact but spec-stipulated):**
9. `prompt-submit-fix`: 3 missing tests on supervisor-skill prose (launch-time-sweep, escalation, "complements not replaces").
10. `supervisor-as-pane`: 5 scenarios — Tab-key-not-handled, no-input-buffer, vertical-layout-collapse, cmd_supervisor-doesn't-self-publish (negative source-audit test), aborted-launch-no-phantom-row.
11. `supervisor-as-pane-followups`: 2 dashboard-layout scenarios overlapping with #10.
12. `openspec-apply-boot-prompt`: 6 scenarios about backend-tag-on-scanned-entries lack `assert!(entry.backend == SpecBackendKind::OpenSpec)` tests in `src/specs/openspec.rs::tests` and `src/specs/markdown.rs::tests`.
13. `config-test-isolation`: 1 scenario ("None preserves platform-default") explicitly waived in code comment with rationale — leave as-is (documented exception, no work).
14. `spec-corrections-v0-5-0`: 2 scenarios — envelope-enumerates-seven, question-no-from-field.
15. `coordination-skill-followups` + `-2`: 2 minor — paste-buffer-cross-ref test, `git paw status` ordering warning substring test.

## What Changes

### 1. AGENTS.md catch-up (the regression)

- Add the 4 v0.5.0 dependencies to the dependency table with one-line "Purpose" cells (`schemars` → JSON Schema generation for governance config; `serde_yaml` → Spec Kit frontmatter parsing; `chrono` → ISO timestamp formatting; `regex` → broker agent_id validation + supervisor sweep filter).
- Update the `dirs` row: change "Platform XDG directories" to "(NOT approved — non-FOSS upstream license; replaced by homegrown `src/dirs.rs` for `config_dir()` resolution)". Move the row to a "Notable exclusions" sub-section beneath the approved table so a future contributor doesn't re-add it.
- Update the Scopes line: add `user-guide`, `worktree`, `governance`, `learnings`, `pause`; document compound-scope form `(scope1,scope2)` with one example.
- No CHANGELOG.md edits (autogenerated by git-cliff).

### 2. `docs/src/user-guide/supervisor.md` consolidation pass

Append four subsections (or amend existing structure):
- **Spec audit governance sub-step**: 4-6 lines pointing at `docs/src/user-guide/governance.md` and the five doc-checklist examples (DoD, ADR, security, test-strategy, constitution).
- **Common dev-command allowlist**: 6-8 lines describing the preset, opt-out via `enabled = false`, and `extra` field; cross-link to `docs/src/configuration/README.md`.
- **Repo-configurable gate commands**: 4-6 lines naming the six `[supervisor]` gate-command keys and the `(not configured)` graceful skip; cross-link to `docs/src/configuration/README.md`.
- **Broker-side conflict detector**: 4-6 lines naming the three failure shapes (forward, in-flight, ownership) and the `[conflict-detector]` tag; cross-link to `docs/src/user-guide/conflict-detection.md`.
- **Learnings aggregator**: 2-line cross-link to `docs/src/user-guide/learnings.md`.
- **"When the user types in your pane"** + **"Merge orchestration"**: mirror the prose from `assets/agent-skills/supervisor.md` (these are already in the bundled skill; copy the user-facing summary).

### 3. `docs/src/user-guide/coordination.md` mirror catch-up

Append a `## Workflow phases` section mirroring the skill's "Before you start editing" / "While you're editing" structure. ~15 lines.

### 4. Test gap closure

- **`prompt-submit-fix` (3 tests in `src/skills.rs::tests`)**: assert the launch-sweep section instructs the supervisor agent to act within the first-few-seconds window; assert the section instructs escalation via `agent.question` for unknown prompts; assert the "complements not replaces" cross-reference to the `[supervisor.auto_approve]` poll thread.
- **`supervisor-as-pane` + followups (5 tests)**:
  - `src/dashboard.rs::tests::tab_key_ignored_no_buffer` — assert pressing `KeyCode::Tab` does NOT alter any state.
  - `src/dashboard.rs::tests::printable_char_ignored_no_buffer` — assert pressing `KeyCode::Char('a')` and space leave no buffer state behind.
  - `src/dashboard.rs::tests::layout_collapses_without_message_log` — assert when `show_message_log = false`, draw_frame's layout chunks are exactly `[title, table, status]`.
  - `tests/source_audit.rs::cmd_supervisor_does_not_publish_supervisor_status` — grep `src/main.rs::cmd_supervisor`'s body for `publish_to_broker_http` AND `build_status_message("supervisor"` — assert zero matches.
  - `tests/source_audit.rs::dashboard_renders_no_supervisor_row_pre_bootstrap` — fixture-driven: render `Snapshot { agents: vec![] }` and assert the output contains no `supervisor` substring and no divider row.
- **`openspec-apply-boot-prompt` (6 tests)**:
  - `src/specs/openspec.rs::tests::scan_returns_entries_with_openspec_backend_tag` — call `scan()` on a fixture with 2 changes; assert both returned entries have `backend == SpecBackendKind::OpenSpec`.
  - Repeat for `Markdown` in `src/specs/markdown.rs::tests::scan_returns_entries_with_markdown_backend_tag`.
  - 4 variants per backend covering single-entry, multi-entry, frontmatter-present, and filtered-out cases.
- **`spec-corrections-v0-5-0` (2 tests)**:
  - `src/broker/messages.rs::tests::envelope_serde_rename_covers_seven_variants` — for each of the 7 `BrokerMessage` variants, assert the JSON shape's `"type"` field matches the spec'd discriminator string.
  - `src/broker/messages.rs::tests::question_payload_omits_from_field` — serialize a `QuestionPayload`, assert the JSON does NOT contain a `"from"` key.
- **`coordination-skill-followups` + `-2` (2 tests)**:
  - `src/skills.rs::tests::supervisor_skill_paste_buffer_cross_ref` — assert the tmux-send-keys-alongside-feedback section cross-references paste-buffer recovery for long answers.
  - `src/skills.rs::tests::supervisor_skill_warns_against_git_paw_status_ordering` — assert the `pane_current_path` section contains the substring `git paw status` AND prose forbidding using its order as a mapping source.

### 5. `config-test-isolation` waived scenario annotation

Add a single doc-comment block in `tests/config_integration.rs` (or the relevant location) documenting that the "None preserves platform-default user-config resolution" scenario is exercised behaviourally by every existing call site (which all pass `None`) but has no dedicated test by design — a dedicated test would either pollute the dev machine's real config dir or require brittle env-var manipulation. This converts the AC gap from "missing" to "documented exception".

## Capabilities

### New Capabilities

*(none — all changes touch existing capabilities)*

### Modified Capabilities

- `user-documentation` — `AGENTS.md` and `docs/src/user-guide/{supervisor,coordination}.md` catch up with v0.5.0 surfaces. No new requirements; corrects landed-but-stale content per the audit.
- `agent-skills` — additional skill-content tests close drift-66 / drift-65 verification gaps.

## Impact

**Code:**

- `AGENTS.md` — dependency table refresh, scope-list refresh.
- `docs/src/user-guide/supervisor.md` — append five sub-sections (governance, allowlist, gate-config, conflict-detector, learnings) + add "When the user types in your pane" and "Merge orchestration" sections.
- `docs/src/user-guide/coordination.md` — append `## Workflow phases` mirror section.
- `src/dashboard.rs` — 3 new unit tests (no production code change).
- `src/specs/openspec.rs` + `src/specs/markdown.rs` — 6 new unit tests (no production code change).
- `src/broker/messages.rs` — 2 new unit tests (no production code change).
- `src/skills.rs` — 5 new unit tests (no production code change).
- `tests/source_audit.rs` — 2 new source-grep tests.
- `tests/config_integration.rs` — 1 doc-comment annotation.

**Tests:** see `### 4`. ~19 new tests total; all behavioural; no scenarios deferred.

**Docs:** see `### 1`–`### 3`. All deltas land in user-facing surfaces named in originating proposals' Impact sections.

**Backward compatibility:** doc + test edits only. Existing tests pass unchanged. No config changes, no schema changes, no API changes.

**Mismatches resolved:**

- 10-agent audit findings (15 audited changes, 4 real docs gaps + 4 minor + 9 AC gaps) — all 8 docs gaps and 8 of 9 AC gaps closed. The 9th (`config-test-isolation`'s "None preserves" scenario) is converted to a documented exception per `### 5`.
- The `docs-v0.5.0-refresh` partial regression on AGENTS.md is corrected without retroactive amend (the original archive stays; this change supersedes the regressed content).
