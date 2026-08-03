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
- [x] Split `main.rs → src/commands/*.rs` + thin `main`/`run` dispatch — DONE. `main.rs` 5510 → 2548 (−54%). `src/commands/` holds helpers, clis, replay, pause, stop, status, approvals, recover, start, add, remove, supervisor; `main`/`run` keep dispatch (qualified per-module). `cmd_dashboard` stays in main.rs (SIGHUP `unsafe` do-not-touch). Implemented via the code-analysis-refactor DOGFOOD (agent on the freshly-installed binary, human-supervised), five-gate-verified + ff-merged per wave. 5 commits: 365cccb (leaf handlers), 0c7c505 (recover), a3ffe9d (start), 23b09ea (add/remove), e87344b (supervisor).
  - HAZARD (future splits): each handler has a `// Command: X` banner + a doc that can span a blank line; removal must take the COMPLETE doc+banner or clippy fails "empty line after doc comment" (build still compiles — clippy is the catch). Dogfood learnings: file EDITS aren't auto-approved by the send-gate classifier (put the agent in accept-edits mode); the supervisor's OWN `simple_expansion`/loop commands trip the classifier with no auto-approver above the supervisor → deadlock.
- [x] Add `CommandRunner`-mocked unit tests asserting the tmux/git **argv** of the `cmd_*` handlers — added during R2c (suite rose to ~2377).
- [x] Enum-ripple guard: `BrokerMessage` / `SpecBackendKind` grepped during R2d — exhaustive matches stay co-located, compiler-caught.
- [x] Gate: five-gate verify green per wave — R2c: build/clippy/fmt clean + full suite 2377/0; R2d: build/clippy/fmt clean + full suite 2375/0 with `source_audit` passing after the `commands/supervisor.rs` repoint. (Verified allow-live in the worktree; the definitive cold `just smoke-container` re-check is the release-prep gate.)

## R3 — tmux runtime + remaining interactive/dashboard _(GATED: PTY net + dashboard CPU-leak fix)_
- [x] **Precondition SATISFIED:** PTY net stable (`cli-interaction-e2e` done) AND the dashboard CPU-leak fix is landed on `feat/v0.13.0-specs` — the `poll_tty(2)` root-cause fix is wired + tested in `dashboard.rs`, and the e2e-suite accumulation source was closed by `TmuxTestEnv`'s `kill-server`-on-drop (@ 241a9ac, 0 leaks validated). R3 is unblocked.
- [x] Refactor the `tmux.rs` runtime path (send-keys / capture-pane parsing / pane-spawn — the `Command::new` sites in `tmux/{session,readiness,layout}.rs`) behind the `CommandRunner` seam — DONE via the R3 dogfood (agent on the new binary, human-supervised), commit `87c7f0e`, byte-identical (full e2e suite 2400/0) + 25 new CommandRunner-mocked argv tests (120 tmux tests). ff-merged.
- [~] Interactive live-prompt impls behind the PTY net — DEFERRED (post-freeze). Behavior-sensitive live `dialoguer`/`crossterm` path, low ROI; the runtime seam is R3's high-value core.
- [~] Dashboard event loop — DEFERRED (post-freeze). The `fix/dashboard-cpu-leak` fix already landed (poll_tty root-cause + TmuxTestEnv reap); the remaining draw/format split touches the SIGHUP `unsafe` + just-fixed loop for low ROI.
- [x] Gate: five-gate verify green — build/clippy `-D warnings`/fmt clean + full suite 2400/0 (byte-identical runtime behavior via e2e). Cold `smoke-container` is the release-prep re-check.

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
