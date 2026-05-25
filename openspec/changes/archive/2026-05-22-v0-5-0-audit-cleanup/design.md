## Context

A 10-agent docs↔code + AC↔tests audit ran 2026-05-21 against the 15 v0.5.0-cycle archived changes after Batch-3 archive. Findings split:

- **8 docs gaps** (4 real + 4 minor) — content named in proposal Impact sections that didn't land in the named surface.
- **9 AC gaps** — `#### Scenario:` blocks in archived specs without corresponding tests.

These are catch-up edits, not new behaviour. One change bundles them for v0.5.0 release prep.

## Decisions

### D1 — Single cleanup change, not per-change retro-amends

The audit findings span 8 archived changes. Two options:

- **D1a (chosen): one new cleanup change.** Bundles all 8 docs gaps + 8 AC gaps into a coherent editorial pass. Tracks the cycle's audit-driven correction as one OpenSpec change with one archive entry; future contributors can read the audit + this change to understand the v0.5.0 release-prep state.
- **D1b (rejected): retroactively edit the 8 archive directories and the corresponding `openspec/specs/` updates.** OpenSpec's archive model treats archives as immutable. Mutating archive content after the fact obscures history and may break tools that diff archives against current specs.

The cleanup change supersedes the regressed `docs-v0.5.0-refresh` AGENTS.md content; the archive of `docs-v0.5.0-refresh` stays intact (it documents what was *intended*).

### D2 — Annotate the waived AC, don't backfill it

`config-test-isolation`'s "None preserves platform-default user-config resolution" scenario has a documented rationale for not having a dedicated test (`src/config.rs:2924-2931`). Writing a test would either:

- Touch the dev machine's real config dir (test pollution).
- Manipulate env vars in process-global state (brittle, racy).

Both options have downsides that outweigh closing the AC gap. The change adds a clarifying doc comment converting the gap from "missing" to "documented exception" so future audits know it's intentional.

### D3 — Source-audit tests over runtime fixture for "cmd_supervisor doesn't self-publish"

Two ways to test "cmd_supervisor does not publish supervisor's own agent.status":

- **D3a (chosen): grep-based source-audit test** at `tests/source_audit.rs` greps `src/main.rs::cmd_supervisor`'s body for `publish_to_broker_http` + `build_status_message("supervisor"` substrings. Cheap, fast, deterministic. Matches the existing pattern in `tests/source_audit.rs` for `run_merge_loop` and `spawn_auto_approve_thread`.
- **D3b (rejected): runtime integration test** launching a supervisor session and watching the broker for the absence of a supervisor `agent.status` within a time window. Heavier; race-prone; tmux dependency.

The audit's failure-mode insight is that a future code change might re-add the launcher-side publish — D3a catches that at compile time of the test, not at runtime.

### D4 — Skill-content tests assert on rendered output, not template substrings

For the `prompt-submit-fix` skill-content gaps and the `coordination-skill-followups-2` `git paw status` warning test:

- Assert on the rendered skill content (`SkillTemplate::content` after `render()`), not on the raw `assets/agent-skills/supervisor.md` source.
- This makes the test stable across future template-substitution changes (e.g. drift 67's `{{...}}` placeholders).

### D5 — Dashboard input-handling tests stay unit-level

For the `supervisor-as-pane-followups` Tab/printable-key/layout-collapse tests:

- Test the `draw_frame` and key-handler functions directly with fixture state, not via `TestBackend` ratatui rendering (which is heavier).
- Layout-chunk assertions check the Vec<Constraint> produced by the layout-builder helper, not the rendered cells.

This is consistent with how `supervisor-as-pane`'s existing tests are structured (per `src/dashboard.rs::tests` patterns).

## Risks / Trade-offs

- **AGENTS.md regression visibility.** Once this change lands, `AGENTS.md` is correct. The originating `docs-v0.5.0-refresh` archive still describes the intended state (which IS what this change implements). The cleanup change's release-notes.md SHOULD explicitly call out "AGENTS.md catch-up from docs-v0.5.0-refresh partial regression — the intended state is now realized."

- **Test count inflation.** ~19 new tests. Mostly small (4-10 lines each), behavioural. Total impact on `cargo test` runtime: negligible (no new I/O, no new compilation surface).

- **Skill text churn vs. doc surface churn.** The `docs/src/user-guide/supervisor.md` consolidation pass touches a much-edited file. Merge conflict risk with any in-flight branch touching the same file is real — schedule this change to land before any new feature work on supervisor.md.

## Migration / Rollout

- Fully additive — no breaking changes.
- Existing tests pass unchanged.
- No config or schema changes.
- No version bump beyond v0.5.0; part of release prep.

After this change lands, v0.5.0 can tag.
