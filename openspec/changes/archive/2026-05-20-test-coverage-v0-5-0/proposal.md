# test-coverage-v0-5-0

## Why

A scenario-coverage audit run against the 13 archived v0.5.0 changes (Audit 6,
post-merge to `main`) measured **258 / 301 = 86%** scenario coverage. The
project's quality gate is 95%+ scenario coverage on shipped specs (≤ 15
uncovered scenarios across the whole v0.5.0 spec surface).

The 43 uncovered scenarios are not behavioural drift — the shipped code
already implements them; the audit recorded gaps where no `#[test]` asserts
the WHEN/THEN clause. Without an explicit test, future refactors can silently
violate the spec; the spec audit step the supervisor runs on every PR
(`agent-skills` capability) treats "no matching test" as a hard fail.

This change closes the coverage gap before the v0.6.0 MCP cycle starts, so the
v0.5.0 spec surface enters v0.6.0 with the property that every WHEN/THEN
scenario has at least one corresponding `#[test]`. **No code changes ship from
this change** — only `tests/` and `#[cfg(test)] mod tests {}` additions, plus
one optional `pub(crate)` visibility lift on `build_task_prompt` for direct
unit-test access (per design.md D1).

## What Changes

This change adds **38 test functions** across the 13 v0.5.0 capabilities (one
function per uncovered scenario, plus a small number of merged-scenario tests
where two scenarios share an obvious test shape). Five gaps are deferred per
design.md D5 (live-tmux / live-broker properties the unit suite cannot safely
assert; flagged in the dropped-tasks footnote in `tasks.md`).

**Per-archived-change test counts to add:**

| Archived change | Missing scenarios | Tests this change adds | Deferred |
|---|---|---|---|
| `2026-05-10-from-specs-launch-fixes` | 3 | 2 | 1 (TTY-attach observable) |
| `2026-05-11-boot-prompt-full-body` | 2 | 2 | 0 |
| `2026-05-11-prompt-submit-fix` | 3 | 3 | 0 |
| `2026-05-13-forward-coordination` | 3 | 3 | 0 |
| `2026-05-13-learnings-mode` | 4 | 4 | 0 |
| `2026-05-13-conflict-detection` | 2 | 1 | 1 (partial-fields, already covered by ConflictConfig defaults test) |
| `2026-05-13-cross-format-spec-selection` | 4 | 3 | 1 (unknown-spec E2E, covered by name-resolution unit test) |
| `2026-05-13-v040-hardening` | 2 | 1 | 1 (whitespace-only rejection, already covered by validation test) |
| `2026-05-13-governance-config` | 1 | 1 | 0 |
| `2026-05-13-governance-context` | 1 | 1 | 0 |
| `2026-05-13-spec-kit-format` | 8 | 6 | 2 (covered: tasks attach to phase, branch slug safe chars, tasks.md writeback) |
| `2026-05-13-supervisor-as-pane` | 9 | 8 | 1 (auto-approve-thread-runs-inside-subprocess; D2 deferred) |
| `2026-05-13-prompt-submit-fix` skill bullets | (included above) | — | — |
| **Total** | **42** | **35** | **6** |

Post-change scenario coverage: **(258 + 35) / 301 = 293 / 301 = 97.3%** — clear
of the 95% gate.

## Capabilities

### New Capabilities

- `test-coverage-v0-5-0`: tracks the per-archived-change scenario-to-test
  mapping for v0.5.0's spec surface. Contains one requirement per archived
  change covering its missing scenarios, each scenario mapped to the
  test-function name added by this change. This capability is **purely
  documentary** — it asserts that for each archived change, a test exists for
  each named scenario. It defines no new runtime behaviour and ships no code.

  The capability spec is archived alongside this change at completion; it
  becomes the v0.5.0 coverage-audit artefact for future drift checks.

### Modified Capabilities

*(none)* — main-spec requirements in `openspec/specs/` are unchanged. Every
gap is implementation-of-existing-spec; no SHALL/MUST is being added,
modified, or removed.

## Impact

**Code (test-only):**
- `src/main.rs` — lift visibility of `build_task_prompt` from `fn` (private)
  to `pub(crate) fn` so the test module in `src/main.rs` (already present) can
  call it directly. The function body is unchanged. (Per design.md D1.)
- `src/main.rs` `#[cfg(test)] mod tests {}` — ~3 new tests for
  `build_task_prompt` purity, spec-id substring, and AGENTS.md substring.
- `src/skills.rs` `#[cfg(test)] mod tests {}` — ~12 new tests for embedded
  skill content substrings (coordination, supervisor, governance, paste-buffer,
  spec-kit consolidated, merge-orchestration, watch-peer-intents ordering).
- `src/broker/conflict.rs` `#[cfg(test)] mod tests {}` — 1 new test asserting
  detector stops cleanly on broker stop (channel close → task exit observable).
- `src/broker/learnings.rs` (or wherever the aggregator lives) — 4 new tests
  for default flush interval, disabled-when-supervisor-off, multi-session H2
  append, and all-five-categories round-trip.
- `src/config.rs` — 1 new test asserting `GovernanceConfig` has no `gates`
  field (negative-assertion against TOML `[governance.gates]` deserialisation).
- `src/interactive.rs` `#[cfg(test)] mod tests {}` — 2 new tests using the
  existing `TrackingPrompter` stub for spec-picker Ctrl+C and zero-selection
  cancellation (per design.md D3).
- `src/specs/speckit/*.rs` — 3 new tests for unrecognised-line graceful
  handling, plan.md-absent boot-prompt section omission, checklists inclusion
  when present, single-`[P]` task body, and explicit-config-wins-over-auto-
  detect (the test for "explicit config wins" extends the existing dispatch
  test fixture; the other tests are siblings of the existing parser tests).
- `src/tmux.rs` or `tests/e2e_*.rs` — 4 new tests for pane-0 placement in
  bare-start vs pane-1 in supervisor mode, dashboard pane title,
  no-dashboard-pane-when-broker-disabled, and the 50/50 top-row split
  argv contract (assertion against `build_*_args` return value; live tmux
  not required — per design.md D4).
- `tests/cli_*.rs` — 3 new `assert_cmd` tests for non-TTY `--supervisor` skip
  path, bare `--specs` TTY-proceeds-to-picker, and boot-block-injection
  failure non-fatal exit code.
- `tests/e2e_supervisor_*.rs` — 2 new tests for `cmd_supervisor` returning
  immediately with attach hint (assertion against stdout + return-Ok-without-
  blocking) and `cmd_supervisor` not invoking `run_merge_loop` (negative
  assertion — `run_merge_loop` symbol no longer present per the REMOVED
  Requirement in `supervisor-cli`; this test asserts the function is gone or
  is not referenced from `cmd_supervisor`).

**Tests:** see `tasks.md` for the per-test function name + assertion + source
spec scenario.

**Specs:** no main-spec deltas. One new change-local capability spec at
`openspec/changes/test-coverage-v0-5-0/specs/test-coverage-v0-5-0/spec.md`
documenting the scenario-to-test mapping.

**Docs:** no user-facing docs change. No `--help` change. No mdBook chapter
change. The CONTRIBUTING.md test-strategy section already states the 95%
gate; this change closes against it.

**Backward compatibility:** no wire-format, no config, no CLI surface change.
The `build_task_prompt` visibility lift is `pub(crate)` so it does not appear
in the published API surface; rustdoc-public consumers see no change.

**Dependencies:** no new crates. `serial_test` (already in dev-deps) gates the
broker-aggregator multi-session H2-append test to avoid `.git-paw/session-
learnings.md` race conditions across parallel tests.

**Out of scope:**
- The 6 deferred gaps in design.md D5 (live-tmux/live-broker properties).
  These remain documented in `tasks.md` under a "Deferred" footnote so the
  next coverage audit explicitly skips them rather than re-flagging them as
  new gaps.
- Any spec-text correction (those are owned by `spec-corrections-v0-5-0`).
- Any new `--help` example, README example, or mdBook chapter (this change
  is test-only).
