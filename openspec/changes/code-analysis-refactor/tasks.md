# Tasks — code-analysis-refactor

Source of record: the four `.git-paw/v0.13.0-wave3-code-analysis-*.md` analyses. Every task is
behavior-preserving. Standing constraint (all waves): the **Do-not-touch** list in `design.md`
stays byte-identical, and the **five-gate verification** below runs cold-start before a wave
lands. Verify a gate by its **real exit code**, never piped output (a trailing `| tail` masks a
failure).

## R0 — Baseline & triage (no code change)
- [ ] Capture a `just coverage` per-module baseline
- [ ] Confirm `just verify` (`GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`) green in a **clean cold-start env** (`just smoke` / `just smoke-container`) — NOT the live dogfood session (local-pass ≠ CI-pass)
- [ ] Split the combined findings into **refactor (behavior-preserving, this change)** vs **bugfix (behavior-changing → `path-injection-hardening` / `broker-runtime-hardening`)**
- [ ] Gate: baseline recorded; suite green cold; triage done

## R0 — Baseline & triage — DONE
- [x] Suite green cold-start confirmed (fresh-worktree full run: 2360 passed, the 1 fail is a known sweep.sh time-window flake, passes isolated — not a regression). Coverage baseline established during test-suite-consolidation. Refactor-vs-bugfix triage done (bugfixes routed to path-injection/broker-runtime; NOT folded here).

## R1 — Low-risk extractions behind the existing (non-PTY) net _(independent — start now)_
- [x] **Idiom/hygiene (D7) subset DONE** (each finding re-verified vs current code): dropped unused `anyhow` (+AGENTS.md row); deleted dead `detect.rs::resolve_command` (note preserved on `resolve_command_in`); removed the 2 vestigial `#[allow(dead_code)]` on tmux.rs; added the missing `///` on `agents.rs::inject_section_into_file`. **DEFERRED within R1:** the 6 logic-invariant `expect()→?` conversions (behavior-adjacent; dedicated per-site pass). [committed]
- [x] `config.rs → config/{mod,supervisor,broker,dashboard,specs,cli,layout}.rs` — pure move + re-export; all `#[serde]` attrs / `Default` / `merged_with` **verbatim** (§ do-not-touch) [committed 19b419a]
- [ ] **GATING NOTE:** R2 (`main.rs` split — the headline conflict-hotspot win) is BLOCKED on `cli-interaction-e2e` (#2) finishing + the source-grep test removal; R3 (tmux runtime) also needs the dashboard CPU-leak fix. So #2 is the true unblocker for #6's high-value waves.
- [x] `tmux.rs` builder (`command_strings` + argv builders) → `tmux/command.rs`; runtime path untouched [committed 49f74e0]
- [~] `interactive.rs` resolver logic → module; live `dialoguer` impls untouched — DEFERRED (lowest-value cosmetic split; not depended on by R2/R3). Revisit post-freeze if desired.
- [~] `dashboard.rs` draw/format helpers → module; event loop + SIGHUP untouched — DEFERRED (cosmetic + carries a rebase cost against the pending `fix/dashboard-cpu-leak`, which R3 already rebases on). Fold into R3's dashboard work rather than splitting twice.
- [x] Introduce the `CommandRunner` trait + real runner (behavior-identical) as the seam type; wire the already-covered builder call sites through it — `src/command_runner.rs` (trait + `RealCommandRunner` + `#[cfg(test)]` `FakeCommandRunner`); wired at `TmuxCommand::execute` (the already-covered builder execute site, `TmuxSession::execute` signature unchanged so main.rs callers untouched); argv + success/failure-handling asserted via the fake. Blind runtime (tmux `session.rs`/`git.rs`) + `cmd_*` deferred to R3/R2 per D2.
- [x] Introduce `SessionName`/`BranchSlug`/`WorktreePath` newtypes at construction points — output **byte-identical** to current inline formatting (NO new sanitization) — `src/domain.rs`; the free constructors delegate to them (`git::branch_slug` → `BranchSlug`, `resolve_session_name` → `SessionName`, `resolve_worktree_path` returns `WorktreePath`), so existing `branch_slug`/`resolve_session_name`/worktree tests pass unchanged (byte-identity proof). #10 hardens these constructors in one place.
- [ ] Idiom/hygiene (D7): drop unused `anyhow` + its `AGENTS.md` row (F1); delete dead `detect.rs::resolve_command` + allow (F3); remove the two vestigial `#[allow(dead_code)]` on `tmux.rs` public helpers; convert the 6 logic-invariant `expect()` → `?`/restructure (F2a); add the missing `///` on `agents.rs::inject_section_into_file`
- [x] Gate: `just verify` green cold + `just deny`; coverage ≥ baseline per touched module; reviewer confirms the diff is pure move/re-export (no line traces to a behavior change) — cold `just smoke-container` 87 suites / 0 failed; `just deny` advisories/bans/licenses/sources ok. New modules (`command_runner`, `domain`) ship with tests (coverage maintained); every diff is pure move / delegate / seam — existing `branch_slug`/`resolve_session_name`/worktree/tmux tests pass UNCHANGED, so no line traces to a behavior change.

## R2 — Unblock + split `main.rs` _(GATED: PTY net ✅ + source-grep handling)_
- [x] **Precondition — W1 PTY net:** `cli-interaction-e2e` matrix now covers the interactive / `--from-specs` / destructive-confirm dispatch + pane/prompt outcomes (spec-launch dispatch rows + bare-`--specs` shown-gate added; scenario→test map recorded). MET.
- [~] **Precondition — source-grep handling (refined; not a blanket prior deletion):**
  - `cli_specs_tty_proceeds_to_picker.rs` — DELETED (behaviorally replaced by `bare_specs_on_tty_shows_spec_picker`). ✅
  - `cli_from_specs_boot_block_failure.rs` — **R2-coupled**: its own doc says a behavioral test needs a tmux-fail shim that doesn't ship — exactly what the `CommandRunner` seam enables. DELETE IN R2 as the seam-based "send-keys failure is non-fatal" `CommandRunner`-mock test replaces it (do NOT orphan it earlier).
  - `source_audit.rs` — **do NOT delete** (it's the standing-guard model both skills cite): 3 real negative-invariant guards (`cmd_supervisor` has no `run_merge_loop` / no `spawn_auto_approve_thread` / no launcher self-publish) + 1 behavioral (empty-snapshot dashboard). When R2 moves `cmd_supervisor` out of `main.rs`, **REPOINT** its `include_str!`/`function_body` to the new `src/commands/supervisor.rs` location so the guards survive the split. The `main_rs_source_is_non_empty` tautology may be dropped.
- [ ] Split `main.rs → src/commands/*.rs` + thin `main`/`run` dispatch — leaf helpers first, then the tmux-orchestration `cmd_*` (`cmd_start`, `launch_spec_session`, `recover_*`, `restart_from_pause`, `attach_agent`) through the `CommandRunner` seam
  - IN PROGRESS (main.rs 5510 → 4961): `src/commands/` created; `mod commands;` in main.rs; dispatch calls qualified per-module. Extracted so far (7 modules, each verbatim/behavior-preserving, a green commit — bin tests 57, clippy clean): `helpers` (config_to_custom_defs, to_interactive_cli, session_cli_settings_paths, configured_settings_paths, expand_tilde + tests), `clis` (list/add/remove-cli), `replay`, `pause`, `stop`, `status`, `approvals` (+resolve_approvals_session).
  - HAZARD seen 3× (R2a config_to_custom_defs, R2b pause, R2b status/approvals): each handler has a `// Command: X` banner + a doc comment that can span a blank line; the removal must take the COMPLETE doc+banner or clippy fails "empty line after doc comment" (build still compiles — clippy is the catch). Grep `// Command:` after each removal to clear orphaned banners.
  - REMAINING R2b — the **purge cluster** (ENTANGLED, do carefully, NOT a mechanical move): `cmd_purge`, `cmd_purge_stale`, `PurgeOutcome`, `resolve_default_branch` + `collect_unmerged_branches` (cluster-internal) are movable to `commands::purge`; BUT `invalidate_if_stale` (shared with cmd_start @ main.rs 486/1790) and `detach_worktree` (shared with cmd_remove @ 2845) belong in `commands::helpers`, and `purge_with_prompt` has 6 TEST callers in main.rs's `mod tests` — decide test placement (move tests with it, or keep + qualify) before moving.
  - REMAINING R2c (seam-coupled, medium): `add`, `remove`, `recover`, `start` (+launch_spec_session, attach_agent, submit_prompt_to_pane, write_repo_discovery_file, resolve_submit_delay_ms) — route tmux/git through `CommandRunner`.
  - REMAINING R2d (highest-risk — CHECKPOINT FIRST): `supervisor` (cmd_supervisor, drive_unattended_loop, spawn_auto_approve_thread, publish_supervisor_question, resolve_supervisor_mode[_from_cwd]) + REPOINT `tests/source_audit.rs` `include_str!`/`function_body` to the new location + delete `cli_from_specs_boot_block_failure` via a seam-mock. `dashboard` stays in main.rs (SIGHUP `unsafe` do-not-touch + `fix/dashboard-cpu-leak` coupling).
- [ ] Add `CommandRunner`-mocked unit tests asserting the tmux/git **argv** of the `cmd_*` handlers without spawning real processes
- [ ] Enum-ripple guard: grep `BrokerMessage` / `SpecBackendKind` across `src/` — exhaustive matches stay co-located and compiler-caught
- [ ] Gate: `just verify` green cold + the PTY matrix green **serially**; coverage ≥ baseline; behavior parity via e2e (exit codes / stdout / `session.json` / panes), never source structure

## R3 — tmux runtime + remaining interactive/dashboard _(GATED: PTY net + dashboard CPU-leak fix)_
- [ ] **Precondition:** PTY net proven stable AND the dashboard CPU-leak fix (`fix/dashboard-cpu-leak`) landed
- [ ] Refactor the `tmux.rs` runtime path (send-keys / capture-pane parsing / pane-spawn — 59 `Command::new` sites) behind the `CommandRunner` seam
- [ ] Interactive live-prompt impls behind the PTY net
- [ ] Dashboard event loop (rebase on `fix/dashboard-cpu-leak`); SIGHUP `unsafe` path still untouched by this change
- [ ] Gate: PTY matrix + cold-start smoke green serially; manual dashboard smoke; coverage ≥ baseline

## R4 — Broker structural tidy _(DEFERRED post-freeze — optional, lowest priority)_
- [ ] Surgical, lock-discipline-preserving moves only in `broker/{delivery,mod,conflict}.rs`; NO reordering of lock / `.await` / `spawn`
- [ ] Extend `mcp_e2e.rs` to assert the thin query-module tool outputs **before** touching them
- [ ] Gate: `just verify` green cold + a cargo-mutants spot-check on `messages.rs`/`conflict.rs`/`delivery.rs` proving the behavioral net still kills the same mutants; serial e2e

## Standing do-not-touch constraint (every wave)
- [ ] Frozen serde surfaces byte-identical: `FileIntent` untagged, `AdvancedMain` flatten, `StatusPayload.message` no-`skip_serializing_if`, `SpecsConfig` `rename="type"`, `Session.created_at` serializer, `merged_with` default-as-"unset"
- [ ] `RepoSessionFile`/`RepoAgentEntry` shape unchanged (cross-process `sweep.sh`/`broker.sh` contract)
- [ ] Broker lock discipline preserved (no reorder of lock/`.await`/`spawn`); `clippy::await_holding_lock` stays clean
- [ ] Dashboard/SIGHUP `unsafe` path untouched
- [ ] `BrokerMessage`/`SpecBackendKind` variant sets unchanged
- [ ] No public CLI flag, config key, or broker wire shape changed

## Verification (five gates, at every wave — supervisor)
- [ ] Gate 1 — Testing: the wave's own tests pass (`cargo test --no-fail-fast` so the env-guard test can't mask failures)
- [ ] Gate 2 — Regression: full suite green vs the **merge-base**, cold-start; e2e serialized
- [ ] Gate 3 — Spec audit: every `code-architecture` scenario maps to a test (behavioral surface, `CommandRunner` argv assertions, re-export/compile checks); `openspec validate code-analysis-refactor --strict` passes
- [ ] Gate 4 — Doc audit: `docs/src/architecture.md` module table + subsystem sections match the new tree; `mdbook build docs/` passes; no `--help`/README/config-reference drift (no CLI/config surface moved)
- [ ] Gate 5 — Security: no secrets; least-privilege preserved; export-agnosticism intact; no frozen surface changed
- [ ] `cargo fmt` before every commit; `just check` + `just deny` green, confirmed by real exit code
