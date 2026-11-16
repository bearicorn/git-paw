## Context

File:line references are as of `a450f46`. The archived changes this builds on are gitignored
(`.gitignore:66-70`), so their names are cited rather than their paths.

**Missing `Test:` pointers.** For each requirement that `2026-11-11-worker-usage-budget` and
`2026-11-11-worker-friction-fixes` added or modified, the scenarios they introduced were
compared against the main specs. These are the ones with no `Test:` line, and the test that
covers each:

| Spec : line | Scenario | Test |
|---|---|---|
| `supervisor-unattended-operation/spec.md:374` | The usage section separates roles and names unknowns | `src/supervisor/drive.rs::the_usage_section_separates_roles_and_names_unknowns` |
| `:882` | A removed, swapped or unknown worker keeps its spend | `drive.rs::the_total_never_goes_down_when_a_worker_leaves_swaps_or_turns_unknown` |
| `:898`, `:904` | Re-read parses appended / rewritten report re-read | `src/agent_cli/usage.rs::a_cached_re_read_parses_only_the_appended_lines`, `::a_report_rewritten_at_the_same_length_is_read_again_from_its_start` |
| `:917`, `:925`, `:932`, `:939` | Breach stops initiated spend / submits buffered text / starts no one-shot / no cap | `drive.rs::a_breach_under_escalate_escalates_once_and_sends_nothing` (asserts `spawns == 0` and a skipped `InvokeOrchestrator`, `drive.rs:11286-11301`), `::after_a_breach_buffered_input_is_submitted_but_no_branch_is_refreshed`, `::no_cap_leaves_actions_and_outcome_unchanged` |
| `:953` | A breach under abandon ends the run | `drive.rs::a_breach_under_abandon_exits_budget_exhausted_without_touching_a_pane`, `tests/unattended_drive_e2e.rs:2550` |
| `:969`, `:975`, `:982` | Unknown workers named / cost cap unenforceable / outcome unchanged | `drive.rs::the_start_report_names_a_worker_with_unknown_usage`, `::the_start_report_says_a_cost_cap_cannot_be_evaluated`, `::an_unevaluable_cost_cap_leaves_a_clean_wave_completed` |
| `cli-provider/spec.md:446-489` (6) | Known report, generic CLI, no id, malformed, no content in reason, relocated variant | `src/agent_cli/usage.rs:519-615` (the reason-content check is `a_malformed_report_is_unknown_not_a_partial_total`, which asserts `!reason.contains("SECRET")`) |
| `supervisor-config/spec.md:543-555` (3) | Absent caps, zero cap, round-trip | `src/config/supervisor.rs:1191`, `:1201`, `:1217` |
| `skill-agent/spec.md:1756`, `:1765` | Budget step present / vendor-agnostic | `tests/orchestrator_budget_skill_content.rs:67`, `:113` |
| `skill-agent/spec.md:148`, `:154`, `:160` | No broker curl / bare helper calls / activity and region examples | `tests/coordination_helper_only_skill_content.rs:28`, `:48`, `:67`, `:132` |
| `broker-agent-helper/spec.md:191`, `:198` | status `--activity` / intent `--region` | `tests/helper_publish_contract.rs:957`, `:1011` |
| `boot-block/spec.md:781-792` (3) | Bare-call instruction, standard / MCP / no CLI named | `tests/boot_block_bare_helper_calls.rs:28`, `:45`, `:56` |
| `git-add-remove-branch/spec.md:783` | add refills an emptied session | `tests/emptied_session_refill_e2e.rs:134` |
| `git-add-remove-branch/spec.md:790` | add on a stopped session is refused | **none.** The refusal is `src/commands/add.rs:98-103`, and no test reaches it |

Already covered, so not touched:
- `session-state` "Status JSON exposes attributed usage" (`:1123-1147`);
- `supervisor-config` "The budget exhaustion policy" and the cost-cap doctor check (`:574-602`);
- the drive-loop breach-detection scenarios (`supervisor-unattended-operation:848-873`);
- `tmux-orchestration` "start never silently ignores a launch selection" (`:682-716`);
- `core-init`'s sidecar and pointer-scope scenarios;
- `preflight-diagnostics`' new scenarios;
- the `session-state` liveness scenarios that friction-fixes added (`:929`, `:938`);
- `skill-agent` "Orchestrator skill confirms a merge landed" (`:1781-1803`);
- `broker-agent-helper`'s region-refusal and poll-cursor scenarios.

**Renames and moves.**
- **README titles.** The titles are at `core-project-conventions/spec.md:12-44`. The v0.19 review
  changed the Purpose in place (D5 of `v019-release-review-fixes`) and left the titles.
- **specs.type.** The stale title is at `core-configuration/spec.md:635`. `superpowers` was
  added as a second scenario (`:642`) instead of a retitle.
- **Spec-format scenarios.** They sit at `code-architecture/spec.md:601-615`, inside "Source audits
  read every production line" (`:574`). `tests/source_audit.rs:447` already files their test
  under `spec-providers` "Per-format behaviour is reached only through the backend"
  (`spec-providers/spec.md:935`).
- **"…unaffected".** The scenario is at `git-operations/spec.md:413`, and the body sentence it
  mirrors is at `:384`.

`architecture-conformance-fixes` recorded the constraint. A MODIFIED block that renames a
scenario fails as an omitted scenario. A REMOVED and ADDED pair with the same requirement name is
rejected. Both were re-checked against openspec 1.16.1 while writing this change.

## Goals / Non-Goals

**Goals:** every scenario those two changes added names its test; no scenario title contradicts
its own body or its requirement; no scenario sits under a requirement it does not test.

**Non-Goals:**
- Back-filling `Test:` pointers across all specs (`core-ci-hygiene` alone has a dozen without one).
  That is a wider audit for its own change.
- Retitling the scenarios `architecture-conformance-fixes` deferred: `preflight-diagnostics`
  "An unknown CLI is reported, not warned" and `skill-agent` "Coordination skill retains blocked and
  artifact curl examples". They are outside the brief. The same REMOVED + ADDED technique applies,
  but the coordination-skill requirement is the largest in the repository, and renaming it ripples
  into every test that cites it.
- Any behaviour change.

## Decisions

### D1 — Pointers via MODIFIED, text byte-identical
Each pointer MODIFIED copies the requirement body and every scenario unchanged, adding only
`Test:` lines. This is the documented way to add a pointer. A scenario keeps the format its spec
already uses (`` Test: `path::fn` ``).

### D2 — New requirement names stay close to the old ones
Each REMOVED requirement returns under a name that differs by a word or two:
- "enumerates the current user-facing surface" → "links each capability family";
- "the specs type value" → "every specs type value";
- "read" → "check";
- "of a live worker branch" → "runs in the worktree holding the branch".

That keeps a search for the old name close to the new one. Each REMOVED block's `Migration` names
its replacement.

### D3 — specs.type scenarios merge rather than retitle
The requirement already demands all four values, and both scenarios point at one test
(`the_specs_example_lists_every_specs_type_value`). One scenario listing the four values replaces
both. This is the one place a scenario count goes down.

### D4 — The stopped-session scenario gets a new test, not an excuse
`add_on_a_stopped_session_is_refused_naming_start` builds a session, kills its tmux session, and
asserts that `git paw add` exits non-zero with `git paw start` in its message. It uses the existing
`emptied_session_refill_e2e` fixtures (`paw`, `kill`). It is the only code this change adds.

### D5 — git-operations body trimmed under the 500-character warning
The re-added requirement says the same in fewer words. "Separate from creation-time" becomes a
clause, and the "unaffected" sentence goes. The retitled scenario "create_worktree keeps the
creation-time rebase rules" keeps the cross-check and its four tests.

## Risks / Trade-offs

- [A parallel change MODIFIES one of these requirements by its old name] → its delta fails to
  apply on archive. None does today (checked against `openspec/changes/`), and `dx-error-cleanups`
  only cites one in prose. Archive this change early in the release.
- [Merging two scenarios loses a distinct check] → the single test already asserts all four values
  together. Nothing is lost.

## Migration Plan

1. Write `add_on_a_stopped_session_is_refused_naming_start`.
2. Update the code comments that cite renamed requirements.
3. Archive. Validate the main specs with `openspec validate --specs --strict` after the sync.

Rollback is a revert of the spec sync.

## Open Questions

None.
